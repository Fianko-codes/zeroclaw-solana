# M3 Security Audit — ZeroClaw Solana submission

Scope audited (read-only):

- upstream draft PR `zeroclaw-labs/zeroclaw-plugins#54`
- fork head `3f7f8e9a5db1a7d7c626d1f22ace166cd0d02b17`
- immutable core `nanosol` rev `989cd0d3bd25ce6a2d796f72c0dc6a4ae56d989f`
- `plugins/solana-pay-request`, `plugins/spl-transfer-build`, `nanosol`

Method: full source read of all three crates; constant/behaviour verification
against the pinned official crates (`solana-message 4.3.0`,
`solana-transaction 4.1.5`, `spl-token-interface 3.0.0`,
`spl-token-2022-interface 3.1.1`, `spl-associated-token-account-interface 2.0.0`,
`solana-short-vec 3.2.2`) and against the `spl-token-2022-interface` source for
the TLV layout; host `inject_config` review in `zeroclaw`; execution of all three
test suites offline (0 failures); and an independent adversarial harness of 11
byte-/structure-level mutations against the production `verify_final_bytes`
entry point (0 bypasses). No production code was modified.

---

## 1. Verdict

**Merge-ready as a draft, and competitive for first prize.** The headline
"summary is derived from and checked against the final serialized bytes" claim is
real and holds under adversarial testing: the verifier decodes the exact bytes it
returns, semantically re-derives every field, re-derives both ATAs, and requires
a byte-equivalent canonical recompile of the whole message. Every hostile
mutation I constructed — including cases the committed tests do not cover — was
rejected fail-closed. Transaction construction matches official Solana byte
output exactly (oracle-compared). RPC, config, and prompt-injection boundaries
are strict and enforced in Rust. Token-2022 policy matches official source
including the padding invariant.

There are **no critical or high findings.** I reached that conclusion only after
building concrete attacks (Section 3 lists them). The residual issues are one
constrained, already-disclosed RPC-trust amount-divergence vector (Medium) and a
small number of test-coverage gaps and packaging items (Medium/Low) that do not
undermine the security model but should be closed before the PR leaves draft.

## 2. First-prize readiness score

**9.0 / 10.**

Strengths that earn it: a genuinely novel, provably-robust verifier; official
byte-for-byte oracles rather than self-consistency; minimal manifest permissions;
canonical upstream layout; honest documentation with preserved public devnet
evidence (finalized signature, balances, component hash). Deductions: the
`nanosol` packaging boundary is an unresolved git dependency (acceptable for a
draft, not for final merge), and two security-relevant paths lack automated
adversarial coverage (officially-packed Token-2022 mint; the WASM transport
shim).

## 3. Critical findings

**None.** Attacks attempted against `verify_final_bytes`
(`plugins/spl-transfer-build/src/transfer.rs`) via an independent harness linking
the same pinned `nanosol`; every one was rejected:

| Attack | Result |
|---|---|
| unreferenced extra static account key appended | rejected — non-canonical recompile |
| non-canonical account-key order with indexes remapped | rejected — non-canonical recompile |
| 6-account TransferChecked (double reference) | rejected — invalid account count |
| reference present in bytes, policy expects none | rejected — fields differ from policy |
| reference absent in bytes, policy expects some | rejected — fields differ from policy |
| source/destination ATA swap (drain recipient) | rejected — ATA derivation mismatch |
| authority a separate readonly signer (not fee payer) | rejected — signer set differs |
| memo content altered | rejected — fields differ from policy |
| duplicated transfer instruction | rejected — instruction count |
| trailing garbage bytes | rejected — wire decode |
| nonzero signature slot | rejected — wire decode |

Config-spoofing via a caller-supplied `__config` is blocked host-side
(`zeroclaw/crates/zeroclaw-plugins/src/runtime.rs::inject_config` removes the
caller key before injecting the trusted section) and the plugin uses
`#[serde(deny_unknown_fields)]`.

## 4. High findings

**None.**

## 5. Medium findings

### M-1 Hidden Token-2022 transfer fee lets a malicious RPC diverge displayed vs. received amount

- **Severity:** Medium (constrained; requires malicious RPC + operator opt-in).
- **File/function:** `nanosol/src/mint.rs::parse_mint_account` /
  `plugins/spl-transfer-build/src/transfer.rs::enforce_token_policy` and
  `approval_summary`.
- **Scenario:** Operator sets `allow_token_2022=true` and allowlists a mint that
  is actually a fee-bearing Token-2022 mint. A dishonest/compromised RPC returns
  an 82-byte (base, extension-free) `getAccountInfo` payload for that mint.
  `parse_mint_account` reports zero extensions, `enforce_token_policy` accepts it,
  and a `TransferChecked` for `amount` is built and verified. `TransferChecked`
  executes successfully on a fee mint on-chain, but the recipient receives
  `amount − fee` while the approval summary states "SEND `amount`". The decimals
  guard in `TransferChecked` prevents amount *inflation*; this is specifically a
  *net-received* divergence caused by an undisclosed fee.
- **Why tests don't catch it:** All mint fixtures are constructed locally and are
  internally honest; there is no fixture modelling an RPC that lies about a mint's
  extension set. The token-policy suite proves fee mints are refused *when the RPC
  reports the extension*, which is the honest-RPC case.
- **Minimum safe correction:** This cannot be fully closed without a trusted
  mint-state source; it is inherent to trusting the RPC for mint layout. Practical
  hardening: (a) keep the current default (`allow_token_2022=false`) prominent;
  (b) in the approval summary, when `token_program == Token2022`, append an
  explicit "net amount depends on mint extensions as reported by the configured
  RPC" qualifier so the human approver is not told an unqualified net figure. The
  README already discloses the RPC trust boundary; mirroring it into the
  model-visible summary is the concrete change.
- **Regression test to add:** `plugins/spl-transfer-build/tests/token_policy.rs`,
  `fn token_2022_summary_is_qualified_or_fee_mint_lie_is_flagged` — feed a
  Token-2022 legacy-length (82-byte) mint payload under `allow_token_2022=true`
  and assert the emitted summary carries the Token-2022 net-amount qualifier.

## 6. Low findings

### L-1 Plugin has no independent defense if the host stops stripping `__config`

- **File/function:** `plugins/*/src/*.rs` `ComponentArgs` (`#[serde(rename =
  "__config")]`).
- **Scenario:** Security depends on `inject_config` deleting the caller key.
  That host behaviour is present and tested today, but the plugin cannot observe
  "before injection", so a future host regression would let a model inject its own
  RPC/sender/allowlist. Defense-in-depth only; not currently exploitable.
- **Tests:** `caller_config_spoof_*` proves the plugin is safe *given* host
  stripping; it cannot prove the plugin is safe if stripping regresses.
- **Correction:** Document the host contract as a hard dependency in the plugin
  README (already partially done) and, if ZeroClaw ever supports it, prefer an
  out-of-band config channel over an in-args reserved key. No code change is
  strictly required.
- **Regression test:** `component_and_injection.rs`,
  `fn raw_caller_config_without_host_strip_is_ignored_or_refused` — call
  `execute_component_input` with a raw `__config` and NO trusted section injected,
  asserting the resolved config is empty and the call refuses (fails closed).

### L-2 Returned Solana Pay URL carries model-visible, percent-encoded untrusted text

- **File/function:** `plugins/solana-pay-request/src/pay_request.rs::build_url` /
  `form_urlencode`; `RequestOutput.url`/`qr_payload`.
- **Scenario:** `label`/`message`/`memo` are attacker-influenced and appear
  percent-encoded in the returned `url`/`qr_payload`, which is model-visible.
  Percent-encoding neutralises structure-breaking characters (newline → `%0A`,
  `&` → `%26`), and the approval-critical `summary` deliberately excludes these
  fields, so summary injection is prevented; but the decoded semantic text remains
  recoverable by the model from the URL.
- **Tests:** `poisoned_display_text_is_encoded_and_never_becomes_the_summary`
  covers the summary exclusion and URL encoding. No test asserts a policy on the
  raw text being model-reachable via the URL (by design it must be, to render a
  QR).
- **Correction:** None required; behaviour is correct. Optionally note in the
  README that the returned URL is a payload for out-of-band QR rendering and that
  its query values are untrusted merchant input.
- **Regression test:** none needed beyond the existing encoding assertions.

### L-3 Solana Pay URL is checked against a hardcoded golden, not an executable `@solana/pay` oracle

- **File/function:** `plugins/solana-pay-request/tests/request.rs::token_request_matches_official_url_encoding_and_golden_reference`.
- **Scenario:** The expected URL string is hand-annotated as matching
  `@solana/pay 1.0.22` `encodeURL`. Field order and `form_urlencode` are correct
  by inspection (amount, spl-token, reference, label, message, memo; unreserved
  set `A–Za–z0–9*-._`, space→`+`), but CI has no executable cross-check, so a
  future encoder drift would only be caught if someone re-derives the golden.
- **Correction:** Keep the golden; add a comment pointing at the exact
  `@solana/pay` commit (already present) and, if feasible, a one-off committed
  vector file generated from the JS encoder.
- **Regression test:** `request.rs`, extend with 2–3 additional golden URLs
  covering empty-optional and SOL-vs-mint permutations.

### L-4 WASM `WakiTransport` adversarial paths are asserted only in comments

- **File/function:** `plugins/spl-transfer-build/src/rpc.rs::WakiTransport::post`
  (`#[cfg(target_family = "wasm")]`).
- **Scenario:** "Accept HTTP 200 only, follow no redirect, cap body before
  parse" is security-relevant but only exercised by the real-host devnet run
  (happy path). The size cap is redundantly enforced in
  `nanosol::rpc::parse_result` (64 KiB), and any redirect surfaces as a non-200
  status (wasi-http does not transparently follow redirects), so the practical
  exposure is small — but non-200/oversize/redirect handling has no automated
  adversarial test.
- **Tests:** Host tests use `MockTransport`; the real transport is `cfg`-gated
  out of host builds.
- **Correction:** Extract the size-accumulation loop into a transport-agnostic
  helper that takes an iterator of chunks + a status code, so it can be unit
  tested off-WASM; assert non-200 → `HttpStatus`, oversize chunk → `ResponseTooLarge`.
- **Regression test:** new `plugins/spl-transfer-build/tests/transport_budget.rs`
  driving that helper with 200/302/oversize fixtures.

## 7. Confirmed strong properties

- **Byte-exact transaction construction.** `nanosol` legacy and v0 messages and
  unsigned transactions serialize byte-for-byte identically to `solana-message` /
  `solana-transaction` via `bincode` (`tests/message_oracle.rs`). Account
  ordering, header signer/writable counts, and privilege merging match the
  official compiler.
- **Instruction correctness.** `TransferChecked` (disc 12, `amount` LE u64,
  `decimals`), ATA `CreateIdempotent` (data `[1]`, 6 accounts, System + token
  program in slots 4/5), memo v3, and compute-budget builders match the official
  SPL/Solana interface crates exactly (`tests/instruction_oracle.rs`).
- **PDA / ATA / program IDs.** All program IDs equal the official crates' `id()`;
  PDA and ATA derivations match `find_program_address` /
  `get_associated_token_address_with_program_id` (`tests/pubkey_oracle.rs`).
- **compact-u16 canonicality.** Matches `solana-short-vec`, including rejection of
  alias encodings (`[0x80,0x00]`), 3-byte overflow, and `> u16::MAX`.
- **Message sanitize.** Rejects duplicate keys, out-of-bounds program/account
  indexes, ALT lookups (v0 lookup count must be 0), trailing bytes, oversize wire
  (`> 1232`), legacy signer counts `≥ 128`, and signature/required mismatch.
- **Verifier (headline).** `verify_final_bytes` pins signer set, fee payer,
  blockhash, instruction count, per-instruction program + account roles + data,
  both ATA derivations (owner recovered, not merely the destination ATA), the
  reference, the memo, and a byte-equivalent canonical recompile of the entire
  message. Survived all 11 adversarial mutations.
- **Token-2022 layout matches official source.** Base mint 82; padding
  `[82,165)` must be all-zero; account-type byte at 165 must be `Mint (1)`; TLV at
  166. This exactly matches `spl-token-2022-interface 3.1.1`
  `type_and_tlv_indices` (including the padding-zero check, which is an official
  invariant, not over-strictness). Discriminants 1–28 match `ExtensionType`;
  duplicates rejected; unknown extensions parse but fail policy closed;
  every extension (fees, hook, permanent delegate, confidential, pausable,
  non-transferable, default-state) is refused.
- **Mint parse.** COption tags at `[0,4)`/`[46,50)`, decimals `[44]`,
  is_initialized `[45]` match the SPL `Mint` layout; executable, wrong-owner,
  wrong-length, uninitialized all fail closed; `space` cross-checked against data
  length when present.
- **RPC boundary.** 64 KiB response cap before parse; strict JSON-RPC 2.0
  envelope; numeric id must match the per-call id; result XOR error; remote error
  surfaces only a numeric code; attacker diagnostic strings are reduced to a
  single bounded category token (simulation error → the single object key, e.g.
  `"InstructionError"`), never the detail; `space`/owner/executable/base64
  validated; HTTPS-only URL with no userinfo/fragment/whitespace/backslash.
- **Guardrails in Rust.** mint allowlist, exact per-mint cap (mint set must equal
  the allowlist), recipient allowlist, on-curve sender, off-curve-recipient
  opt-in, Token-2022 opt-in, self-transfer refusal, and exact decimal arithmetic
  (no floats; rejects `0`, excess precision, exponents, signs, whitespace,
  overflow) are all enforced in code, not just documented.
- **Reference preimage.** Domain-tagged, fixed-width recipient/mint, u32-BE
  length-framed variable fields; SOL vs token disambiguated by a tag byte even for
  an all-zero mint — collision-resistant; framing tested with the
  `(1,23)` vs `(12,3)` case.
- **Output hygiene.** Both tools hard-cap output `< 4000` bytes, emit no stdout
  (`println!`/`eprintln!`/`wasi:logging` absent — asserted by test), and structured
  logs carry only bounded phase/category labels — no args, memo, URL, RPC body,
  account bytes, or simulation logs.
- **Determinism.** Repeated identical invocations produce identical output;
  operator-config map insertion order does not change output.

## 8. Missing or weak tests

Ranked by value. None of these indicate a live defect (the harness confirms the
paths are safe); they close self-consistency and coverage gaps.

1. **Officially-packed Token-2022 mint round-trip.** Current extended-mint
   fixtures are built to `nanosol`'s own offset assumptions. Discriminants and
   the 82/165/166 offsets are independently verified, but no test packs a mint
   with the official `spl-token-2022-interface` extension machinery and feeds
   those exact bytes to `parse_mint_account`.
   Target: `nanosol/tests/mint.rs::official_packed_extended_mint_parses_identically`.
2. **Verifier cases proven only by this audit's harness.** Add: unreferenced
   extra static key; 6-account TransferChecked; reference-direction mismatch
   (both ways); non-canonical account-key order with remapped indexes; authority
   as a separate readonly signer.
   Target: `plugins/spl-transfer-build/tests/transaction_and_mutations.rs`.
3. **Token-2022 net-amount qualifier / fee-mint-lie** (see M-1).
   Target: `plugins/spl-transfer-build/tests/token_policy.rs`.
4. **Host-strip regression** (see L-1).
   Target: `plugins/*/tests/*injection*.rs`.
5. **Transport budget/status unit test** off-WASM (see L-4).
   Target: new `plugins/spl-transfer-build/tests/transport_budget.rs`.

No false-positive tests were found: spot-checks confirm the security-relevant
assertions fail if the control is removed (e.g. the mutation loop asserts
`is_err()`, the injection tests assert empty output and zero RPC calls).

## 9. Documentation and PR issues

- The READMEs and `RESULTS.md` are accurate, honest, and appropriately scoped;
  the RPC trust boundary, statelessness (no per-day cap), recent-blockhash
  expiry, and Token-2022 limits are disclosed. Test counts in `RESULTS.md`
  (nanosol 36; M3 component 25) reconcile with the actual runs.
- The devnet evidence (finalized signature, recipient/sender balances,
  component sha256, 475-byte capture) is concrete and verifiable — strong for the
  demo/documentation and merge-readiness criteria.
- Minor: the READMEs are dense and implementation-heavy for a judge skimming for
  utility; a 3–4 line "what a merchant/operator gets" opener on each would help
  the 30% utility and 10% demo axes without diluting the safety content.
- Minor: `RESULTS.md` references the source-path sensitivity of the Cargo hash;
  this is correct and worth keeping, but state up front that the WASM artifact is
  not committed (only its hash), so a reviewer knows to rebuild.
- Add M-1's net-amount qualifier note to the `spl-transfer-build` README's
  Token-2022 section so the disclosure is co-located with the opt-in.

## 10. Packaging recommendation

Acceptable for a **draft**: both plugins pin `nanosol` at the immutable rev
`989cd0d…`, which is clean-clone reproducible, and the crate correctly lives
outside `plugins/` (shared crates may not live there). The layout, manifest
fields, `crate-type`, and WIT path (`../../wit/v0`) match upstream conventions;
`registry.json` is generated and correctly does not yet list the two plugins.

For **merge**, resolve the boundary with the maintainer before leaving draft.
Most upstream-friendly, in order of preference:

1. **Vendor `nanosol` as a small in-repo path crate** under a non-`plugins/`
   workspace location (e.g. `crates/nanosol`) so there is no external git
   dependency — simplest for a maintainer to reason about and build offline.
2. **Publish `nanosol` to crates.io** and depend on a pinned version — clean, but
   adds a release surface the maintainers must own.
3. Keep the pinned git rev only if the maintainer explicitly prefers it.

The git dependency is the single most likely maintainer-concern item; surface the
decision proactively in the PR description.

## 11. M4 go / no-go recommendation

**No-go for now — hold M4.** The decision standard is met on the blocking
criteria (no unresolved critical/high; M3 preserved as the immutable
`989cd0d…` fallback), but M4 is optional and the guidance prefers a polished M3
over an unreliable durable-nonce feature. Before starting M4, close the
merge-blocking non-security items on M3: the `nanosol` packaging decision
(Section 10) and the Section 8 regression tests, then security-freeze M3. Only
after M3 is merge-locked should durable nonce begin on an isolated branch that
does not weaken `verify_final_bytes` and can be dropped cleanly if devnet
evidence fails. Starting M4 now risks delaying a submission that is already
first-prize competitive.

## 12. Exact remediation order

1. **(Merge-blocking, non-code)** Resolve the `nanosol` packaging boundary with
   the maintainer (Section 10, prefer option 1). Update both plugin `Cargo.toml`
   and `RESULTS.md`.
2. **(Medium, small code)** M-1: add the Token-2022 net-amount qualifier to
   `approval_summary` and its README section; add the fixture test.
3. **(Test debt)** Add the officially-packed Token-2022 mint round-trip test
   (Section 8.1) — highest-value coverage gap.
4. **(Test debt)** Port the five harness mutations into
   `transaction_and_mutations.rs` (Section 8.2).
5. **(Low)** Add the host-strip regression test (L-1) and, if adopted, the
   transport-budget unit test (L-4).
6. **(Docs)** Utility-first README openers and the "artifact not committed" note
   (Section 9).
7. **Security-freeze M3, keep `989cd0d…` as the tagged fallback, then decide on
   M4** per Section 11.

---

*Production code was not modified during this audit. Findings were validated by
running all three test suites (0 failures) and an independent 11-case adversarial
harness against the production `verify_final_bytes` (0 bypasses).*
