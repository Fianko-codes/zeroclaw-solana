# M5 — `solana-pay-confirm`: closing the payment loop

**Status: designed, not built.** This is the one remaining substantial engineering
item. Everything below is decided; the build is mechanical from here.

**Why this and not a third unrelated tool.** The submission currently covers
*request* → *build*. Adding confirmation makes it **request → build → confirm**:
the complete agent-safe Solana payment path, end to end, externally audited, with
on-chain proof. A judge comparing that against three unrelated utilities is
comparing a product to a toolbox. Adding a ninth `token-risk-check` would dilute
the audited positioning that is the entire differentiator.

## Shape

| Field | Value |
|---|---|
| Plugin | `plugins/solana-pay-confirm` |
| Tool | `solana_pay_confirm` |
| Track | A (payments) |
| Custody | **T0 — read-only.** No transaction is constructed, signed, or submitted. No bytes are returned that could ever be signed. |
| Permissions | `http_client`, `config_read` |
| Shared core | `nanosol` (same pin as the other two plugins) |

T0 is a genuine tier drop from the existing pair, which is worth saying out loud:
the submission then spans T0 and T1 with nothing above T1 anywhere.

## The core safety decision: the reference is derived, never accepted

The obvious API — "take a `reference` and tell me if it was paid" — is wrong.
A model that can choose the reference can point the tool at *any* payment on
chain and get back "paid: true", which is a confirmation-forgery primitive.

So the tool takes **exactly the inputs `solana-pay-request` takes** —
`recipient`, `amount`, `mint`, `invoice_id` — and **re-derives** the reference
itself:

```
reference = SHA-256( "zeroclaw-solana-pay-v1" ‖ recipient ‖ asset-discriminator
                     ‖ canonical-amount ‖ invoice-id )
```

`recipient` is constrained to the operator's `allowed_recipients`, exactly as in
the request plugin. Therefore the model cannot redirect the confirmation, cannot
substitute a reference, and cannot confirm a payment that was not requested with
these exact terms. A wrong amount produces a different reference, which finds
nothing — the check is bound to the invoice by construction rather than by a
later comparison.

`nanosol::reference::derive_payment_reference` **already exists** and is already
what `solana-pay-request` uses, so request and confirm cannot drift. Add a
cross-plugin golden vector asserting the `reference` in a request URL equals the
reference the confirm tool derives from the same four inputs.

## Verification, from raw bytes

The RPC endpoint is a trust boundary. `jsonParsed` output is the endpoint's
*interpretation*, so verification must not rest on it.

1. `getSignaturesForAddress(reference, { limit, commitment })` — Solana Pay
   attaches the reference as a read-only non-signer on the transfer instruction,
   which is exactly what makes a payment findable.
2. For each candidate, newest first, bounded by `max_signatures_scanned`:
   `getTransaction(sig, { encoding: "base64", maxSupportedTransactionVersion: 0 })`.
3. Decode the **message bytes** with `nanosol` and require, fail-closed:
   - `meta.err == null`;
   - exactly one SPL Token / Token-2022 transfer instruction;
   - `destination == ATA(recipient, mint)`, re-derived locally;
   - `mint ==` the resolved mint;
   - instruction amount `==` the expected raw amount, and for `TransferChecked`
     the decimals match the mint's real on-chain decimals;
   - the derived reference is present **in that instruction's account list** as a
     read-only non-signer — not merely somewhere in the transaction;
   - confirmation status meets `min_commitment` (**default `finalized`**).
4. **Cross-check the balance delta.** From `meta.preTokenBalances` /
   `postTokenBalances`, the recipient ATA's increase must equal the expected raw
   amount.

Step 4 is the one that earns the tool its keep. A Token-2022 transfer fee, or a
permanent delegate, can make the amount *received* differ from the amount
*requested* — the exact deception the transfer builder's audit flagged as its one
residual Medium. A confirmer that only reads the instruction amount inherits that
hole; one that reconciles against the balance delta closes it. **It confirms what
arrived, not what was asked for.**

### Optional: two-endpoint agreement

Because this is a pure read, a second endpoint is a cheap and unusually strong
mitigation: with `rpc_url_secondary` set, both endpoints must return the same
transaction bytes for the same signature, or the tool refuses. A single lying RPC
stops being sufficient to forge a confirmation. Worth shipping — no other read-only
tool in the registry does this.

## Statelessness, and why this sidesteps the blocker that killed `payment-watch`

`PLAN.md` gated `payment-watch` on proving that a cursor survives between SOP
runs — unproven, and the reason it was cut. **This design has no cursor.** The
reference is a pure function of the invoice, so:

- every call re-derives and re-checks from scratch;
- the verdict is idempotent — once paid, always paid, same answer forever;
- there is no "newest signature" to carry across runs, and nothing to persist.

Duplicate-notification suppression and termination remain the SOP's business, and
the README will say so plainly rather than implying in-plugin state. The tool
exposes `match_count` so an SOP can detect a **double payment** — two settled
transfers against one invoice — which is a real merchant condition and something
a cursor-based watcher would silently skip.

## Interface

Input: `recipient`, `amount`, `mint`, `invoice_id`. Unknown fields denied
(`deny_unknown_fields`). No `reference` field exists in the schema, and no
`__config`.

Output, bounded to ≈200 tokens and length-asserted in tests:

```json
{
  "paid": true,
  "signature": "…",
  "slot": 0,
  "confirmation_status": "finalized",
  "mint": "…",
  "recipient": "…",
  "reference": "…",
  "expected_raw": "1500000",
  "received_raw": "1500000",
  "received_ui": "1.5",
  "match_count": 1,
  "summary": "CONFIRMED 1.5 USDC received by …, finalized, signature …"
}
```

Anything that does not verify returns `paid: false` with a specific reason.
There is no third state and no hedged "probably". A refusal uses
`success: false` per the WIT contract; a verified-not-paid answer is a successful
call with `paid: false`.

## Config

| Key | Required | Notes |
|---|---|---|
| `rpc_url` | yes | HTTPS only, same URL grammar as the transfer builder |
| `rpc_url_secondary` | no | when set, both endpoints must agree on the transaction bytes |
| `allowed_recipients` | yes | non-empty, unique; the model picks from this set only |
| `mint_allowlist` | yes | canonical pubkeys |
| `mint_aliases` | no | `NAME=mint`, uppercase-normalized, must target allowed mints |
| `min_commitment` | no | `confirmed` \| `finalized`; default `finalized` |
| `max_signatures_scanned` | no | default 10, hard ceiling |
| `allow_token_2022` | no | default `false`; extension policy identical to the transfer builder |

Decimals are read from the **on-chain mint account**, not from config — the RPC
call is already being made, so there is no reason to trust an operator-typed
number for a money path.

## `nanosol` delta

Bounded, and the reason a new immutable revision is needed:

- `get_signatures_for_address_request` / parser;
- `get_transaction_request` / parser (base64 message + the `meta` fields needed:
  `err`, `slot`, `confirmationStatus`, pre/post token balances);
- a decoder for a **signed** v0 transaction — `decode_unsigned_v0_transaction`
  requires all-zero signature slots, which a settled transaction obviously does
  not have;
- legacy-message support: a real wallet may submit a legacy (non-v0) message, so
  the decoder must accept both rather than refusing real payments.

Then re-pin all three plugins to the new revision and re-run the full matrix.
Nothing about the existing verify path changes.

## Test plan

Host-run, mocked RPC, no network. Adversarial fixtures are the point:

- golden: request URL reference == confirm-derived reference (cross-plugin);
- wrong mint; wrong recipient ATA; amount off by one raw unit; decimals mismatch;
- reference present in the transaction but **not** in the transfer instruction;
- `meta.err != null` (failed transfer) → not paid;
- `confirmed` when `finalized` is required → not paid;
- **Token-2022 fee shortfall**: instruction amount correct, balance delta short →
  not paid;
- two settled matches → `match_count: 2`, still verified, flagged;
- legacy vs v0 message shapes;
- secondary-endpoint disagreement → refuse;
- injection: a poisoned message tries to supply `reference`, spoof `__config`,
  swap the recipient off-allowlist, or coerce `paid: true` — all fail closed, with
  the transcript generated by an asserted test;
- output budget assertion.

## Evidence plan

1. Devnet, real host: `solana-pay-request` produces a URL → pay it from a
   disposable wallet **with the reference attached** → `solana_pay_confirm`
   returns `paid: true` with the finalized signature and a matching balance delta.
   Then re-run and show the verdict is unchanged (idempotence).
2. Negative on devnet: same invoice, wrong amount → different reference → not
   found → `paid: false`.
3. Mainnet read-only smoke: the tool's RPC and decode paths against mainnet, as
   already done for the transfer builder.
4. Read-only security audit of the new plugin before the PR body claims it, to
   keep "externally audited" true of the whole submission rather than two thirds
   of it.

## Sequence

1. `nanosol`: RPC additions + signed/legacy decoder + tests → new immutable rev.
2. Re-pin, re-run the full 1.96.1 matrix on all three plugins.
3. `solana-pay-confirm`: pure core, then the thin wasm shim, then the test suite.
4. README: custody tier, threat model incl. the RPC boundary and the two-endpoint
   mitigation, config, worked example, injection transcript.
5. Devnet + mainnet evidence; security audit.
6. Promote onto the PR branch as its own revertable merge commit; update the PR
   component table, proof section, and title to say **request → build → confirm**.

Estimated: a few days, dominated by tests and evidence rather than by logic —
which is the correct ratio for this submission.
