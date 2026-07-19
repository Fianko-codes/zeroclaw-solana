# M4 Phase-A Spike — Durable Nonce Protocol & Simulation

Isolated experiment. Frozen M3.5 (`zeroclaw-plugins@95e10dc`) and the immutable
`nanosol@989cd0d` are **not** modified by this spike. This crate lives only on the
disposable branch `agent/m4-durable-nonce-experiment`.

## Starting state (recorded before any Phase-A work)

| Repo | Branch (disposable) | Base commit |
|---|---|---|
| zeroclaw-solana (main repo, nanosol) | `agent/m4-durable-nonce-experiment` | `989cd0d3bd25ce6a2d796f72c0dc6a4ae56d989f` |
| zeroclaw-plugins (submodule) | `agent/m4-durable-nonce-experiment` | `95e10dc1b8ec4c796b22d50ffc63136e462eaf0a` |

Frozen tags (unchanged): `m35-security-freeze-95e10dc` → `95e10dc…`,
`m3-known-good-3f7f8e9` → `3f7f8e9a5db1a7d7c626d1f22ace166cd0d02b17`.
PR #54 head = `95e10dc…` (draft, frozen M3.5) — untouched.

Toolchain: `cargo +1.96.1` (cargo 1.96.1, 356927216 2026-06-26). Devnet RPC
`https://api.devnet.solana.com` (solana-core `4.2.0-beta.1`), Agave CLI
`solana-cli 3.1.13`.

## Official sources / oracle crates (host dev-dependencies only)

| Crate | Version | Used for |
|---|---|---|
| `solana-nonce` (features: serde) | `=3.2.0` | `Versions`/`State`/`Data`/`DurableNonce` byte oracle |
| `solana-system-interface` (features: bincode) | `=3.2.0` | `advance_nonce_account`, `SystemInstruction::AdvanceNonceAccount` (idx 4) |
| `solana-sdk-ids` | `=3.1.0` | system program id, recent-blockhashes sysvar id |
| `solana-pubkey`, `solana-hash`, `solana-instruction`, `solana-message`, `solana-transaction` | pinned (match nanosol) | pubkey/hash/instruction/message oracles |
| `bincode` | `=1.3.3` | serialize official `Versions` to compare bytes |

Primary docs: Agave "Durable Transaction Nonces" implemented proposal
(`docs.anza.xyz/implemented-proposals/durable-tx-nonces`), Solana core docs
(`solana.com/docs/core/transactions/durable-nonces`), `simulateTransaction` RPC
(`solana.com/docs/rpc/http/simulatetransaction`). Deprecation notice: durable
nonces "may be deprecated in a future release" (SIMD discussion #415 — a
discussion, not an activated SIMD; feature is fully live today). Recent-blockhashes
sysvar deprecated for on-chain reads but still a **required account** in
AdvanceNonceAccount.

## Nonce-account format — CONFIRMED byte-for-byte

An initialized, current-version nonce account (`Versions::Current(State::Initialized(Data))`)
serializes to **exactly 80 bytes** via bincode (default: enum discriminants are
u32 little-endian, integers fixed-width LE):

| Offset | Len | Field | Initialized value |
|---|---|---|---|
| 0 | 4 | Versions discriminant (u32 LE) | `01 00 00 00` = Current (1) |
| 4 | 4 | State discriminant (u32 LE) | `01 00 00 00` = Initialized (1) |
| 8 | 32 | authority (Pubkey) | — |
| 40 | 32 | durable_nonce (Hash) | — |
| 72 | 8 | lamports_per_signature (u64 LE) | — |

Reproduced from official crates (`examples/print_layout.rs`, and asserted in
`tests/nonce_oracle.rs`):

```
len=80
bytes=01000000 01000000 <authority×32> <durable_nonce×32> 8813000000000000
                                                           ^ lamports_per_signature = 0x1388 = 5000
Uninitialized state discriminant  = 00 00 00 00
Legacy version wrapper            = 00 00 00 00 (variant 0)
```

Corrected/important assumptions:
- **bincode tolerates trailing bytes** on `deserialize::<Versions>` (verified:
  81-byte input → `is_err=false`). The nanosol parser is therefore a **strict
  fixed-80-byte reader**, not a bincode call: it requires `len == 80`, version
  `Current`, state `Initialized`, and refuses Legacy, Uninitialized, unknown
  version/state discriminants, and any other length. Modern accounts created by
  `solana create-nonce-account` are always `Current`.
- A freshly allocated all-zero 80-byte account decodes as version `0` (Legacy) →
  refused as `UnsupportedVersion(0)`.

## AdvanceNonceAccount instruction — CONFIRMED byte-for-byte

Matched to `solana_system_interface::instruction::advance_nonce_account` in
`tests/nonce_oracle.rs` and reproduced in `examples/print_layout.rs`:

- program id = System Program = 32 × `0x00` (== `nanosol::pubkey::SYSTEM_PROGRAM_ID`).
- instruction data = `04 00 00 00` (`SystemInstruction::AdvanceNonceAccount`,
  discriminant 4 as u32 LE, no payload).
- accounts (exact order):
  - `[0]` nonce account — **writable, non-signer**
  - `[1]` `SysvarRecentB1ockHashes11111111111111111111` (raw
    `06a7d517192c568ee08a845f73d29788cf035c3145b21ab344d8062ea9400000`) —
    readonly, non-signer
  - `[2]` nonce authority — **signer** (readonly at the instruction level)
- must be **instruction index 0** for the runtime to treat the transaction as a
  durable-nonce transaction (`NONCED_TX_MARKER_IX_INDEX = 0`).
- the message `recent_blockhash` field must equal the stored durable nonce.

Corrected assumption (build-path subtlety): when `nonce_authority == sender ==
fee payer` (the only M4 arrangement), `Message::compile` merges the authority key
with the fee payer, so the compiled message marks that key **writable+signer** at
index 0 — not the readonly-signer of the raw instruction. The runtime only
requires the authority to be a signer, so the decoder requires exactly `is_signer`
and the verifier separately checks `authority == sender == fee payer`. Proven in
`tests/durable_build.rs`.

## Runtime failure semantics (to be devnet-confirmed; documented for summary/README)

- Stale/mismatched nonce, wrong authority, non-writable nonce, malformed/uninitialized
  nonce, or missing AdvanceNonceAccount-at-index-0 → **validation fails, transaction
  dropped, nonce not consumed, no fee**.
- Once nonce validation succeeds, a **later instruction failure still advances the
  nonce and charges the fee** (anti-replay/anti-fee-theft). This MUST appear in the
  approval summary and README.

## Simulation configuration (devnet-pending)

Durable simulation must use `sigVerify=false`, `replaceRecentBlockhash=false`,
`encoding="base64"`. `sigVerify` and `replaceRecentBlockhash` are mutually
exclusive; `replaceRecentBlockhash=true` would swap in a fresh cluster blockhash
and **bypass** the durable-nonce validation, so it must not be used in durable mode.

## Phase-A gate status

| # | Condition | Status |
|---|---|---|
| 1 | Official nonce-account bytes parsed correctly | **PASS** (`tests/nonce_oracle.rs`) |
| 2 | Official `AdvanceNonceAccount` bytes match exactly | **PASS** (`tests/nonce_oracle.rs`) |
| 3 | Runtime account order, signer & writable requirements proven | **PASS** (oracle + `tests/durable_build.rs`) |
| 4 | Simulation works without blockhash replacement | PENDING — needs a funded devnet nonce account |
| 5 | Unsigned durable tx survives ≥ 5 min | PENDING — needs devnet SOL |
| 6 | External signing & submission finalize | PENDING — needs devnet SOL |
| 7 | Nonce before/after state preserved | PENDING — needs devnet SOL |
| 8 | No production plugin behavior modified yet | **PASS** (only `spikes/durable-nonce/` added) |

Spike tests: `cargo +1.96.1 test` in `spikes/durable-nonce/` → 6 passed
(`nonce_oracle` 4, `durable_build` 2). Devnet driver `durable_devnet`
(`parse`/`build`/`sign`) self-tested offline: parses official bytes, builds a
291-byte single-signature-slot unsigned durable SOL transfer, and the external
signer produces a valid signature leaving the message bytes unchanged.

### Blocker on conditions 4–7

`https://api.devnet.solana.com` airdrop returns HTTP 429 "reached your airdrop
limit today"; the disposable sender could not be funded automatically. Conditions
4–7 require a funded disposable devnet keypair. A background retry loop continues;
otherwise the disposable sender address must be funded out-of-band (web faucet or a
transfer from an existing devnet wallet) to complete Phase A. Private keys never
enter `nanosol`, the plugin, or committed evidence; the disposable sender key lives
only in the session scratchpad and is destroyed at the end.

_This file is updated in place once the devnet conditions run._
