# Solana PDA, ATA, and compact-u16 oracle spike

This isolated Milestone 0 crate implements the minimum algorithms independently:

- strict base58 parsing of 32-byte Solana public keys;
- `create_program_address` and descending-bump `find_program_address`;
- associated-token-account derivation for an explicitly selected token program;
- canonical compact-u16 encoding and decoding.

The runtime dependency surface is only `bs58`, `sha2`, and
`curve25519-dalek`. Official Solana crates are development-only test oracles:
`solana-pubkey` checks PDA behavior,
`spl-associated-token-account-interface` checks ATA addresses, and
`solana-short-vec` checks compact-u16. `bincode` is used only to invoke the
official `ShortU16` serializer. The implementation was written independently;
official source is not copied into it.

Run from this directory:

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

To display the preserved ATA addresses and compact-u16 boundary vectors while
still executing the comparisons:

```bash
cargo test --locked --test oracle -- --nocapture
```

