# nanosol

`nanosol` is the isolated deterministic core for the ZeroClaw × Solana
submission. This revision extends the immutable M1 core only with reusable M3
transfer-building primitives. It is small enough to audit and compiles for
`wasm32-wasip2`; it contains no signer, private-key type, HTTP client, async
runtime, or generic Solana SDK dependency.

The M1 primitives remain unchanged:

- strict base58 32-byte public keys, on-curve checks, PDA bump search, and ATA
  derivation for both SPL Token programs;
- exact UI amount ↔ raw `u64` conversion with no floating point;
- `TransferChecked`, ATA `CreateIdempotent`, memo, and compute-budget builders;
- canonical compact-u16;
- canonical account compilation, legacy messages, v0 static-key messages
  (explicitly no ALTs), unsigned transaction serialization, total decoding,
  and base64;
- typed errors and deterministic single-line shaping helpers.

Added for M3:

- strict JSON-RPC request builders and response parsers for
  `getLatestBlockhash`, `getAccountInfo`, and `simulateTransaction`;
- strict SPL Mint decoding, token-program owner identification, on-chain mint
  decimals, and minimal Token-2022 TLV inspection;
- semantic decoders for ATA `CreateIdempotent`, `TransferChecked`, and memo
  instructions inside an unsigned v0 transaction;
- the M2 domain-separated payment-reference derivation, preserving its golden
  vector exactly.

RPC support here is codec-only. The plugin owns transport, response-size
enforcement, endpoint validation, and policy. Token-2022 extension acceptance
is likewise a plugin policy decision; `nanosol` reports parsed extension types
without silently accepting them.

Official Solana crates are development-only oracles. Runtime dependencies are
limited to the M0 set (`base64`, `bs58`, `sha2`, and `curve25519-dalek`) plus
`serde_json` for the three bounded RPC codecs.

## Validation

Run from this directory:

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --target wasm32-wasip2 --release
```

The WASM command proves target compatibility of the standalone core. This
crate does not itself produce or load a ZeroClaw component.

## Deliberate non-goals

This crate does not implement HTTP transport, operator configuration,
Token-2022 acceptance policy, durable nonce accounts, signing, production
plugin shims, or network I/O. It deliberately exposes only the three RPC
methods needed by M3 rather than a generic RPC surface.
