# ZeroClaw WASM build spike

This isolated Milestone 0 crate binds directly to
`../../zeroclaw-plugins/wit/v0` and exports that checkout's `tool-plugin`
world. The component reports plugin information, exposes one meaningful tool,
emits structured host log records, and returns WIT `tool-result` values. It
does not write logs to stdout and never performs a live HTTP request.

`src/core.rs` is ordinary host-testable Rust. It proves 32-byte public-key
base58 decoding, SHA-256, Edwards-curve decompression/off-curve detection,
JSON parsing/serialization, and base64 encoding. `src/lib.rs` is the thin WASM
shim. `waki` is target-gated to WASM and its client is constructed (but never
sent) to make the architecture check explicit.

Run from this directory:

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo clippy --locked --target wasm32-wasip2 -- -D warnings
cargo build --locked --target wasm32-wasip2 --release
```

The release component is
`target/wasm32-wasip2/release/zeroclaw_wasm_build_spike.wasm`.

