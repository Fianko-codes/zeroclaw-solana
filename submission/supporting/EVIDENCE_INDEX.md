# Supporting material: quality and reliability evidence

## What can be checked immediately

| Claim | Evidence |
| --- | --- |
| Components compile as standalone WASM tools | [`demo/reproduce.sh`](../../demo/reproduce.sh) runs locked tests, host/wasm Clippy, and WASM release builds on the pinned Rust `1.96.1` toolchain. |
| Payment request validation and QR boundary | [`solana-pay-request` README](../../zeroclaw-plugins/plugins/solana-pay-request/README.md) and [evidence](../../zeroclaw-plugins/plugins/solana-pay-request/EVIDENCE.md). |
| Final-byte-derived unsigned-transfer summary | [`spl-transfer-build` README](../../zeroclaw-plugins/plugins/spl-transfer-build/README.md), especially its final-byte verification and external-signing workflow; [evidence](../../zeroclaw-plugins/plugins/spl-transfer-build/EVIDENCE.md). |
| Invoice-bound, balance-delta confirmation | [`solana-pay-confirm` README](../../zeroclaw-plugins/plugins/solana-pay-confirm/README.md) and [evidence](../../zeroclaw-plugins/plugins/solana-pay-confirm/EVIDENCE.md). |
| Independent adversarial review | [`M5_SECURITY_AUDIT.md`](../../M5_SECURITY_AUDIT.md): threat model, 24 attacks, findings, fixes, and post-remediation validation. |
| Plugin ABI / registry compatibility | [`zeroclaw-plugins` README](../../zeroclaw-plugins/README.md) and vendored [`wit/v0`](../../zeroclaw-plugins/wit/v0/README.md). |
| Exact reusable low-level primitives | [`nanosol` README](../../nanosol/README.md) and its Rust test suite. |

## Recorded external validation

- **Real ZeroClaw host:** all three components have host execution records using
  ZeroClaw `0.8.3` and WASM components, rather than only unit tests.
- **Real devnet builder acceptance:** `spl-transfer-build` evidence records an
  externally signed, public devnet transaction, independent byte decode,
  simulation, and final balance change. The component had only the sender's
  public key.
- **Real mainnet read-only checks:** builder evidence documents a mainnet
  simulation and a Token-2022 refusal. Confirmation evidence documents a real
  host call against mainnet and offline verification over captured raw
  transaction bytes.
- **Cross-plugin binding:** request and confirmation assert the same frozen,
  independently derived reference vector, while the builder attaches the same
  reference convention to transfer instructions.
- **Mutation resistance:** the builder evidence documents independently applied
  mutations of amounts, accounts, programs, flags, instructions, signatures,
  lookup data, and trailing bytes; each is refused rather than paired with the
  old approval summary.

## Limits disclosed rather than papered over

- A first-party request→paid→confirm `paid:true` recording is not already
  preserved. The existing devnet fixture was pruned and a new faucet attempt was
  rate-limited. The video SOP requires a newly funded disposable devnet payment
  before making that claim on camera.
- Confirmation depends on its operator-selected RPC. A second independently
  operated endpoint reduces a single-provider forged-positive risk but does not
  eliminate all external trust.
- The components are stateless; a merchant system must own delivery policy,
  duplicate-notification suppression, persistence, retries, and daily limits.
- The design is intentionally narrow: no native SOL confirmation, arbitrary
  instructions, swaps, generic signing, submission, or key custody.

## Source navigation

Public repository locations (also linked in the write-up):

- [Plugin repository (pinned source)](https://github.com/Fianko-codes/zeroclaw-plugins/tree/2c3592afa241603bd34311ba392e7214e9ef0a41)
- [Shared core (pinned source)](https://github.com/Fianko-codes/zeroclaw-solana/tree/09d73652be97a8f348938feac4b022cb049b0c35)
- [Draft upstream PR](https://github.com/zeroclaw-labs/zeroclaw-plugins/pull/54)
- [Solana Pay transfer-request specification](https://github.com/solana-foundation/solana-pay/blob/master/SPEC.md#specification-transfer-request)
