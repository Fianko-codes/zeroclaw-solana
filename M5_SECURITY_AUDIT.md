# M5 Security Audit — `solana-pay-confirm` and the `nanosol` read delta

Scope audited (read-only, then remediated):

- `plugins/solana-pay-confirm` at fork head `c26da355c1243bc75cf6b99f2353e007b3742651`
- the `nanosol` delta introduced by rev `2093879cd1cc28c6182386706d70b8a56b3b07be`
  (`signature`, the `getSignaturesForAddress` / `getTransaction` surface, signed
  legacy/v0 decoding, token-transfer decoding)
- the re-pinned `solana-pay-request` and `spl-transfer-build` (dependency change
  only)

Method: full source read of the new component and the core delta; construction of
an adversary model for each trust boundary (model arguments, operator config, and
the RPC endpoint) followed by concrete attack attempts against
`verify_record` and `execute_component_input`; verification of the reference
derivation and the ATA derivation against **independent Python implementations**
(framed SHA-256, and PDA derivation including Ed25519 point decompression);
execution of the full suite on the pinned 1.96.1 toolchain; and one real
mainnet-beta transaction run through the production verification path.

Companion documents: `M3_SECURITY_AUDIT.md` (the earlier two components) and
`plugins/solana-pay-confirm/RESULTS.md` (evidence).

---

## 1. Verdict

**No critical or high findings.** The central claim — that this tool can only
confirm a payment which was requested with these exact terms — holds. The
reference is derived from the four invoice fields and there is no argument, config
key, or code path by which a caller can supply one; `deny_unknown_fields` turns an
attempt into a refusal before any network read. Recipients are constrained to the
operator's allowlist. The binding is checked against the transfer instruction's
own account list rather than the transaction as a whole, and the amount is
checked twice — once as instructed, once as *received* — which closes the
Token-2022 net-amount divergence that the M3 audit recorded as its one residual
Medium against the transfer builder.

Three findings were raised during this audit and **all three are fixed in the
audited tree**: one documentation accuracy issue (Medium), one public-API
footgun (Low), and one unbounded-work issue (Low). Each has a regression test.

## 2. Adversary model

| Adversary | Controls | Best outcome against this component |
|---|---|---|
| Prompt injection via the model | the four schema fields only | A refusal, or a verdict about an invoice the operator already allows. Cannot supply a reference, endpoint, commitment, or window; cannot reach a non-allowlisted recipient or mint. |
| A caller spoofing `__config` | nothing after host stripping | Nothing. Verified both with host stripping reproduced and with a raw caller section and no host injection (fails closed). |
| An on-chain third party | can attach any reference key to their own transactions | Denial of service only: spam can push a real payment out of the scan window. Cannot forge a match — a spam transaction fails from the destination check onward. |
| **A dishonest RPC endpoint** | every byte the verdict is computed from | **Can forge a positive confirmation** when only one endpoint is configured. This is inherent to a read-only tool and is now stated as such (F-1). |
| A partly dishonest endpoint | lies about one field, relays the rest | Nothing: local decoding, local ATA derivation, the reference check, and the balance reconciliation must all agree. |

## 3. Attacks attempted against the verifier

Each was constructed as a fixture and run against the production entry point.
All were rejected, and rejected with the specific reason claimed:

| Attack | Result |
|---|---|
| Supply `reference` directly in arguments | refusal `invalid_arguments`, 0 RPC calls |
| Supply `paid`, `signature`, `match_count` in arguments | refusal `invalid_arguments`, 0 RPC calls |
| Spoof `__config` (endpoint, allowlist, commitment) | operator section wins; every read went to the operator's endpoint at the operator's commitment |
| Swap the recipient off the allowlist | refusal `recipient_not_allowed`, 0 RPC calls |
| Reference present in the transaction but on another instruction | `paid:false`, reason names the instruction scope |
| Reference attached to the transfer as **writable** | `paid:false`, same reason |
| Transfer of a different mint into the expected ATA | `paid:false`, wrong mint |
| Transfer to a different recipient's ATA | `paid:false`, wrong destination |
| Amount off by ±1 base unit | `paid:false`, instruction amount |
| `TransferChecked` asserting decimals the mint does not have | `paid:false`, wrong decimals |
| Correct instruction amount, short balance delta (fee mint) | `paid:false`, amount received differs |
| Balance delta larger than the invoice | `paid:false`, same |
| Post balance present, no increase | `paid:false`, balance did not increase |
| Balance record for a different account index | `paid:false`, missing balance record |
| Balance record with a foreign owner or mint | `paid:false`, wrong destination / wrong mint |
| Two transfer instructions in one transaction | `paid:false`, multiple transfers |
| Failed transaction (list-level and metadata-level) | `paid:false`, failed on chain; list-level costs no transaction read |
| Commitment below the operator minimum, and absent commitment | `paid:false`, commitment too weak; no transaction read |
| Slot disagreement between list and transaction | `paid:false`, inconsistent slots |
| Undecodable bytes / address-table lookups | `paid:false`, unsupported message subset |
| Second endpoint disagreeing, or never having seen the signature | refusal `endpoint_disagreement` |
| Transport faults (unavailable, 503, oversize, bad UTF-8) | refusals, never verdicts |
| Endpoint error prose in a JSON-RPC error | refusal; the prose does not appear in output |
| Reference spam ahead of the real payment in the window | `paid:true` on the real payment, `match_count 1` |

A wrong-amount query derives a different reference and scans a different address
entirely, which is the property that makes the binding structural rather than a
comparison that could be skipped.

## 4. Findings and remediation

### F-1 The RPC trust boundary was understated (Medium, documentation)

- **File:** `plugins/solana-pay-confirm/README.md`, "RPC trust and
  prompt-injection model".
- **Issue:** the section said a dishonest endpoint could *hide* a payment or lie
  about mint state. That is incomplete in the direction that matters: the
  transaction bytes, the reported commitment, and the balance metadata all come
  from the endpoint, so a single endpoint willing to fabricate all three
  consistently can make an unpaid invoice read as `paid: true`. An operator
  reading the old text could reasonably have concluded that a false positive was
  impossible.
- **Why it is a real finding:** the value of this component is that its verdict is
  trustworthy. A trust claim that overstates its own guarantee is the one
  documentation defect that can cause financial loss.
- **Fixed:** the section now states the forgeable-positive case explicitly, in a
  pull quote, and explains why in-plugin Ed25519 verification would *not* close it
  (forged bytes need not be real, so they can be signed with an attacker's own
  key, and the balance delta remains the endpoint's word regardless — so the
  check would add a dependency and buy nothing against this adversary). It then
  ranks what does help: two-endpoint agreement; the fact that every paid verdict
  names a signature an operator can check independently; and the partial-lie
  detection that local decoding and reconciliation provide. The limitations list
  and this audit's adversary table say the same thing.

### F-2 `verify_record` accepted a commitment level as an argument (Low, API)

- **File:** `plugins/solana-pay-confirm/src/confirm.rs`.
- **Issue:** the public verifier took `confirmation_status: CommitmentLevel`
  alongside the candidate it belonged to. Internally the scan loop always passed
  the value it had just filtered, so the shipped behaviour was correct — but the
  signature allowed a caller to pass a level the endpoint never reported, and the
  commitment gate then lived in the *caller* rather than in the verifier. That is
  a bypass waiting for a future refactor.
- **Fixed:** the parameter is gone. `verify_record` reads
  `candidate.confirmation_status`, requires it to satisfy
  `expected.min_commitment`, and treats an absent status as unknown. Both the
  candidate-level and record-level failure flags are now also checked inside the
  verifier. The pre-check in the scan loop is retained purely to avoid a needless
  network read and is documented as an optimisation, not the gate.
- **Regression test:**
  `adversarial.rs::a_verdict_cannot_be_reached_with_a_commitment_the_endpoint_did_not_report`
  drives the public verifier directly and asserts both the refusal at `finalized`
  and the pass once the operator accepts `confirmed`.

### F-3 A full scan window could multiply the per-read cap (Low, availability)

- **File:** `plugins/solana-pay-confirm/src/confirm.rs`.
- **Issue:** each response was capped (64 KiB, or 256 KiB for a transaction), but
  a 25-signature window against two endpoints permits fifty such reads — up to
  ~12.5 MiB of JSON parsing in one call, against a host default of 1e9 fuel and no
  wall-clock timeout. A hostile endpoint that answered every read at maximum size
  could plausibly have driven the component into a fuel trap, which surfaces as a
  plugin *fault* rather than a clean refusal.
- **Fixed:** a single `ReadBudget` (1 MiB, `MAX_TOTAL_RESPONSE_BYTES`) now covers
  every read in a call across both endpoints. The remaining allowance is what is
  handed to the transport, so an oversize body is refused before it is buffered,
  and exhaustion is a clean refusal with its own category
  (`read_budget_exhausted`) rather than a trap.
- **Regression test:**
  `adversarial.rs::an_endpoint_that_answers_every_read_at_maximum_size_exhausts_a_bounded_budget`
  asserts the refusal category and that the scan stopped part-way rather than
  reading the whole window.

## 5. Verified properties

- **Reference derivation** matches an independent Python implementation of the
  framed hash, and matches the reference embedded in the real, finalized M3
  devnet payment. Both plugins assert the same frozen vector, so the pair cannot
  drift apart silently.
- **ATA derivation** matches an independent Python PDA implementation (including
  the off-curve check) against a real on-chain ATA credited by a real mainnet
  payment.
- **No write path exists.** A test asserts the component source contains no
  `sendTransaction`, `simulateTransaction`, `requestAirdrop`, message
  compilation, transfer construction, signing, or key-handling symbol. Manifest
  permissions are exactly `http_client` and `config_read`.
- **Output is bounded and endpoint-free.** Happy-path output is asserted under
  1 200 bytes against a 4 000-byte ceiling; every refusal reason and verdict
  reason comes from a closed taxonomy of the component's own sentences; untrusted
  invoice text is quoted, de-controlled, and truncated.
- **Empty config confirms nothing**, unknown config keys are refusals, endpoint
  URLs must be credential-free HTTPS, and a "second" endpoint equal to the first
  is refused.

## 6. Residual risks, accepted and documented

1. **A single dishonest endpoint can forge a positive verdict** (F-1). Mitigated
   by `rpc_url_secondary` and by the independently checkable signature; not
   eliminated. This is the honest ceiling of a read-only confirmer.
2. **The signature list is taken from the primary endpoint only.** A primary that
   omits a payment causes a false negative. Cross-checking lists was considered
   and rejected: two healthy endpoints legitimately lag each other, so list
   comparison would convert benign lag into refusals. Documented.
3. **Reference spam can push a real payment out of the window.** Bounded by
   design (default 10, maximum 25); the reason string reports how many candidates
   were scanned so the condition is visible rather than silent.
4. **CPI transfers are not decoded.** Covered in net effect by the balance
   reconciliation; a payment made entirely through a program CPI reads as
   `paid: false`. Documented as a limitation.
5. **Address-table lookups are refused** rather than resolved, because a token
   balance's `accountIndex` would otherwise index a list the component cannot
   reconstruct. Fail-closed, documented.
6. **Per-day limits remain impossible** in a stateless plugin, as for the other
   two components.

## 7. Post-remediation validation

Re-run on the pinned toolchain after all three fixes:

```text
cargo +1.96.1 test --locked                                → 62 passed / 0 failed
cargo +1.96.1 clippy --locked --all-targets -- -D warnings  → rc 0
cargo +1.96.1 clippy --locked --target wasm32-wasip2 -- -D warnings → rc 0
cargo +1.96.1 build --locked --target wasm32-wasip2 --release → rc 0
```

No production behaviour changed for an honest endpoint: the same fixtures produce
the same verdicts, and the two new tests cover paths that previously had none.
