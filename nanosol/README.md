# nanosol

`nanosol` is the isolated M1 core for the ZeroClaw × Solana submission. It is
small enough to audit and compiles for `wasm32-wasip2`; it contains no signer,
private-key type, RPC client, async runtime, or generic Solana SDK dependency.

Implemented in M1:

- strict base58 32-byte public keys, on-curve checks, PDA bump search, and ATA
  derivation for both SPL Token programs;
- exact UI amount ↔ raw `u64` conversion with no floating point;
- `TransferChecked`, ATA `CreateIdempotent`, memo, and compute-budget builders;
- canonical compact-u16;
- canonical account compilation, legacy messages, v0 static-key messages
  (explicitly no ALTs), unsigned transaction serialization, total decoding,
  and base64;
- typed errors and deterministic single-line shaping helpers.

Official Solana crates are development-only oracles. Runtime dependencies are
limited to the dependency set proven by M0: `base64`, `bs58`, `sha2`, and
`curve25519-dalek`.

## Validation

Run from this directory:

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --target wasm32-wasip2 --release
```

The WASM command proves target compatibility of the standalone core; M1 does
not produce or load a ZeroClaw component. Component shims begin in later
milestones.

## Deliberate non-goals

M1 does not implement RPC, mint-account parsing, Token-2022 extension policy,
durable nonce accounts, signing, production plugin shims, or network I/O.
