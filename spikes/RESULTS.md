# Milestone 0 results

## Verdict

**M0 PASS.** Both isolated lockfiles are preserved, the actual checked-out
ZeroClaw `tool-plugin` WIT builds as a `wasm32-wasip2` component with the full
proposed dependency set, all required host and WASM Clippy checks pass, and the
independent PDA/ATA/compact-u16 implementation matches the selected official
Solana oracles. No production plugin code or later-milestone code was changed.

M1 may begin under the acceptance conditions stated in `PLAN.md`.

## Date and environment

- Recorded: `2026-07-18T12:04:59+05:45` (Asia/Kathmandu)
- Host: `Linux 7.1.3-arch1-3 x86_64 GNU/Linux`
- Default compiler: `rustc 1.97.0 (2d8144b78 2026-07-07)`
- Default Cargo: `cargo 1.97.0 (c980f4866 2026-06-30)`
- CI compiler installed and validated: `rustc 1.96.1 (31fca3adb 2026-06-26)`
- CI Cargo: `cargo 1.96.1 (356927216 2026-06-26)`
- Default installed targets at start: `wasm32-unknown-unknown`,
  `wasm32-wasip2`, `x86_64-unknown-linux-gnu`
- `wasm32-wasip2` was already installed for the default toolchain, so the
  preparation step did not reinstall it.
- The exact CI toolchain was not initially installed. It was installed with
  Rustfmt, Clippy, and `wasm32-wasip2`, then used for the parity run.

Environment commands (all passed):

```text
rustc -V
cargo -V
rustup show active-toolchain
rustup target list --installed
rustup toolchain install 1.96.1 --profile minimal --component clippy,rustfmt --target wasm32-wasip2
rustc +1.96.1 -V
cargo +1.96.1 -V
rustup target list --installed --toolchain 1.96.1
```

## Repository baseline

The outer workspace Git repository had **no commits** before M0. Consequently,
it has no current commit hash: `git rev-parse HEAD` exited 128 with
`fatal: ambiguous argument 'HEAD'`. Its pre-existing status was:

```text
## No commits yet on master
?? PLAN.md
?? zeroclaw-plugins/
?? zeroclaw/
```

The worktree was therefore already dirty before M0. The two checked-out nested
repositories were clean:

- `zeroclaw-plugins`: `23a5dcb953f697cae08d8e2802b39894ac9ddda1`
- `zeroclaw`: `e592a555d69c6a701c0fa0fa3f94a4bbcffbb2c2`
- `zeroclaw-plugins/wit/UPSTREAM_REF`:
  `e112ce6b5ccdac9e1cb166bab217e730dd7e24c2`

Preparation inspected `PLAN.md` in full, every file in
`zeroclaw-plugins/wit/v0`, both reference plugin directories
`plugins/redact-text` and `plugins/telegram`, and
`.github/workflows/validate.yml` plus its invoked
`tools/ci/validate_components.sh`. The nested repositories remained clean
after M0.

## Checked-out WIT and component shape

The spike binds directly to
`../../zeroclaw-plugins/wit/v0`; it does not use a copied or reconstructed WIT.
The checked-out contract is package `zeroclaw:plugin@0.1.0`, world
`tool-plugin`, with imported `logging` and exported `plugin-info` and `tool`.
The generated component implements both plugin-information functions and all
four tool functions (`name`, `description`, `parameters-schema`, `execute`).

The shim uses `logging.log-record` with `plugin-event`; it has no stdout/stderr
logging calls. Valid inputs return a successful `tool-result`; invalid user
input returns `success: false` with a model-visible error. `waki::Client` is
constructed to prove the target-only API compiles, but it is never sent and no
network call occurs.

## Resolved dependencies

Both locks were created with `cargo generate-lockfile` and used with `--locked`
for the acceptance tests/builds.

### `spikes/wasm-build/Cargo.lock`

- Lock SHA-256:
  `e1fd4a8a1f872f727ba9c5b2b58dbad8b8dd01c9cf7a21426c814a7c1e2ec4f5`
- `base64 0.22.1`
- `bs58 0.5.1`
- `curve25519-dalek 4.1.3` (`default-features = false`, `alloc`)
- `serde 1.0.228`
- `serde_json 1.0.150`
- `sha2 0.10.9`
- direct `wit-bindgen 0.46.0`
- WASM-only `waki 0.5.1`
- transitive `wit-bindgen 0.34.0` from `waki 0.5.1`

The duplicate WIT binding versions are real Cargo resolution, also present in
the checked-out Telegram reference plugin. The spike does not override or
hide it.

### `spikes/pda-oracle/Cargo.lock`

- Lock SHA-256:
  `d1f55405732123cc5bd95b8af323029c08242b0f922b9b9d6b72090f002a7a68`
- Runtime: `bs58 0.5.1`, `curve25519-dalek 4.1.3`, `sha2 0.10.9`
- Development-only oracle: `bincode 1.3.3`, `solana-pubkey 3.0.0`,
  `solana-short-vec 3.2.2`,
  `spl-associated-token-account-interface 2.0.0`

`solana-pubkey` is pinned to `3.0.0` for the direct PDA oracle because the
current ATA interface `2.0.0` exposes that major's `Pubkey` type. The resolved
dev graph also contains `solana-pubkey 4.2.0` transitively through current
official interface dependencies; it is not used as the direct comparison
type.

Dependency evidence commands (passed):

```text
(cd spikes/wasm-build && cargo generate-lockfile)
(cd spikes/pda-oracle && cargo generate-lockfile)
(cd spikes/wasm-build && cargo tree --locked --target wasm32-wasip2 --depth 1)
(cd spikes/wasm-build && cargo tree --locked --target wasm32-wasip2 -i wit-bindgen@0.34.0)
(cd spikes/wasm-build && cargo tree --locked --target wasm32-wasip2 -i wit-bindgen@0.46.0)
(cd spikes/pda-oracle && cargo tree --locked --depth 1)
```

## Commands and outcomes

Commands below are shown with their working directories. No required
validation command failed.

### Default Rust 1.97.0 run

From `spikes/wasm-build`:

| Exact command | Result | Output summary |
|---|---|---|
| `cargo fmt --check` | PASS | No diff. |
| `cargo test --locked` | PASS | 3 passed, 0 failed; doc tests passed. |
| `cargo test` | PASS | 3 passed, 0 failed; doc tests passed. |
| `cargo clippy --all-targets -- -D warnings` | PASS | No warnings. |
| `cargo clippy --target wasm32-wasip2 -- -D warnings` | PASS | Component shim and WASM-only `waki` graph checked; no warnings. |
| `cargo build --locked --target wasm32-wasip2 --release` | PASS | Optimized component produced. |

From `spikes/pda-oracle`:

| Exact command | Result | Output summary |
|---|---|---|
| `cargo fmt --check` | PASS | No diff. |
| `cargo test --locked` | PASS | 9 oracle tests passed, 0 failed; doc tests passed. |
| `cargo clippy --all-targets -- -D warnings` | PASS | No warnings. |
| `cargo test --locked --test oracle -- --nocapture` | PASS | 9 passed and preserved the vector values below. |

### Exact CI Rust 1.96.1 parity run

From `spikes/wasm-build`:

| Exact command | Result | Output summary |
|---|---|---|
| `cargo +1.96.1 fmt --check` | PASS | No diff. |
| `cargo +1.96.1 test` | PASS | 3 passed, 0 failed; doc tests passed. |
| `cargo +1.96.1 clippy --all-targets -- -D warnings` | PASS | No warnings. |
| `cargo +1.96.1 clippy --target wasm32-wasip2 -- -D warnings` | PASS | Component and target-only HTTP graph checked; no warnings. |
| `cargo +1.96.1 build --locked --target wasm32-wasip2 --release` | PASS | Final optimized component produced. |

From `spikes/pda-oracle`:

| Exact command | Result | Output summary |
|---|---|---|
| `cargo +1.96.1 fmt --check` | PASS | No diff. |
| `cargo +1.96.1 test --locked` | PASS | 9 oracle tests passed, 0 failed; doc tests passed. |
| `cargo +1.96.1 clippy --all-targets -- -D warnings` | PASS | No warnings. |

The only nonzero commands relevant to the recorded environment were
informational, not acceptance checks:

- Outer-workspace `git rev-parse HEAD`: exit 128 because no initial commit
  exists.
- `git -C zeroclaw cat-file -t e112ce6b5ccdac9e1cb166bab217e730dd7e24c2`:
  exit 128 because this local clone does not contain the older pinned object.
- `command -v wasm-tools`: exit 1 because `wasm-tools` is not installed.
  The successful Rust component build and `file` identification below were
  used instead; no extra validation tool was installed merely for inspection.

## WASM artifact

- Path:
  `spikes/wasm-build/target/wasm32-wasip2/release/zeroclaw_wasm_build_spike.wasm`
- Final producer: Rust/Cargo 1.96.1 parity build
- Byte size: **178,738 bytes**
- SHA-256:
  **`3d7798d6cf6fb115c773780b8997c54b0e83aaabf88e18070b54941a9012652b`**
- `file` result: `WebAssembly (wasm) binary version 0x1000d (component)`

For comparison, the preceding default-1.97.0 release build also passed but
produced 181,074 bytes with SHA-256
`f92ea777153e81778f93d1c8f50e116161bba227fcbf03aa209cb434ab1f370a`.
The final evidence uses the artifact built by the repository's pinned CI
toolchain.

Build output lives under ignored `target/` directories and is not intended for
commit. Source, READMEs, and both locks are the reproducible artifacts.

## PDA and ATA oracle results

Every hand-written result matched official `solana-pubkey 3.0.0` or
`spl-associated-token-account-interface 2.0.0` bytes and bump behavior:

| Vector | Address | Bump |
|---|---|---:|
| PDA seeds `"vault"`, wallet bytes | `AX98mRGeNSHXNjVEzoXbqYnqro3RAuqY8jY5kwypJaps` | 253 |
| PDA seeds `"metadata"`, mint bytes, `"v1"` | `A58xJoDHBWdR5nbLcYBb2Fkykxx2sPfHmti2na1FfjN5` | 254 |
| PDA seeds empty, `"deterministic"`, `[0,1,2,3]` | `ADSYsXEau7knP8J9FLYJsyurfXdSMmH9xfx13giicpPR` | 254 |
| Bump-search seed `[0x00,0x00]` | `5R38h2RixqJ8g5kcSEtS8dR3YLdAWyeqdgAXagREzjHR` | 254 |
| Legacy SPL Token ATA | `C4PRXFV6Gf5mytVZb6RoeLsG8CjcFWzR2EJ3dvwPTUJH` | 255 |
| Token-2022 ATA | `8SxXYnUGVsRBsFWUoqczVJyoNKfeGbo9nsSdHyFTirZc` | 254 |

The bump-search fixture proves bump 255 is rejected as on-curve before bump
254 succeeds. Seed `[0x01,0x00]` provides a deterministic direct on-curve
rejection vector. A 33-byte seed, 17 explicit seeds, and 16 pre-bump search
seeds were rejected consistently with the official oracle. Public-key parsing
matched official bytes for all fixtures and rejected malformed/wrong-length
inputs. PDA, ATA, and compact results were repeated 64 times without drift.

## Compact-u16 oracle results

All boundary encodings matched official `solana-short-vec 3.2.2`:

| Value | Bytes |
|---:|---|
| 0 | `00` |
| 1 | `01` |
| 127 | `7f` |
| 128 | `80 01` |
| 255 | `ff 01` |
| 16,383 | `ff 7f` |
| 16,384 | `80 80 01` |
| 65,535 | `ff ff 03` |

Both implementations rejected empty/truncated encodings, alias encodings,
third-byte overflow, and continuation beyond the third byte. Both returned
the same consumed length when trailing bytes followed a valid value.

## Warnings, deviations, and assumptions

### Compiler/build warnings

None. Tests and both Clippy targets emitted no Rust warnings. Cargo did report
newer incompatible major versions as available during lock generation; the
resolved versions above were retained because they are the versions compatible
with the checked-out reference architecture and oracle type graph.

### Deviations or disproven assumptions

1. The `PLAN.md` observation that the release artifact would be “on the order
   of tens of KB” did **not** reproduce. The final CI-toolchain component is
   178,738 bytes (about 174.5 KiB), and the default-toolchain component was
   181,074 bytes. This is a size-estimate discrepancy, not a build or
   architecture failure.
2. The current latest `solana-pubkey` is 4.2.0, but the current official ATA
   interface 2.0.0 uses the 3.x public-key type. The direct oracle therefore
   pins `solana-pubkey = 3.0.0`; blindly selecting latest would create
   incompatible Rust types or require byte conversion between oracle majors.
3. The outer workspace cannot supply the requested single commit hash because
   it has not had an initial commit. The clean nested repository hashes are
   recorded instead.

No WIT signature differed from the checked-out reference, and every proposed
functional dependency compiled successfully for `wasm32-wasip2`.

### Unresolved assumptions

- The component was compiled and identified as a Component Model binary, but
  was not instantiated in a live ZeroClaw host during this source/build spike.
- PDA curve work was not fuel-profiled in a live host. The explicit M0
  acceptance commands and the session's completion standard do not require a
  host-load/fuel measurement; it remains a follow-up integration measurement.
- No live HTTP request was made, by design. M0 proves only that the correctly
  target-gated `waki` API and dependency graph compile.
- The local `zeroclaw` clone lacks the historical `UPSTREAM_REF` Git object, so
  this session did not independently refetch and byte-diff that historical
  tree. The spike nevertheless consumed the exact current vendored WIT bytes,
  and the plugin repository's CI contains the authoritative drift check.

## Failure architecture changes

Not applicable: M0 passes. No architecture change is required before M1.

