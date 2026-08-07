# ZeroClaw Solana Payment Desk

**Summary:** A real Telegram-connected ZeroClaw agent that creates Solana Pay requests, keeps signing in the user's wallet, and confirms payment from decoded on-chain evidence.

## What it does

ZeroClaw Solana Payment Desk gives merchants a narrow non-custodial payment workflow.

- `solana_pay_request` turns invoice terms into a Solana Pay URL, QR payload, and deterministic reference.
- The customer approves in Phantom or another wallet. ZeroClaw never receives a private key.
- `solana_pay_confirm` re-derives the reference, decodes matching transaction bytes locally, and returns `paid: true` only when the recipient token balance increased by the exact requested amount.

The demo shows this in a real Telegram channel with a phone wallet mirrored by `scrcpy`: request -> wallet approval -> byte-verified confirmation.

## Who it is for

Small merchants, payment operators, and support teams that want an agent to collect and verify SPL-token payments without turning the agent into a wallet or autonomous settlement system.

## ZeroClaw features used

- WASM/WIT plugins: isolated `wasm32-wasip2` components.
- Minimal permissions: request is config-only; confirm/build use HTTP only for bounded Solana RPC reads/simulation.
- Operator config: recipient/mint allowlists and RPC policy are injected by ZeroClaw, not chosen by the model.
- Real channel operation: Telegram is the live interface.
- Supervised custody boundary: signing stays external.

## What was built

- `nanosol`: shared Rust Solana core for exact amounts, public keys, ATAs/PDAs, deterministic references, transaction inspection, and narrow JSON-RPC parsing.
- `solana-pay-request`: deterministic Solana Pay transfer-request URLs without network access.
- `solana-pay-confirm`: read-only exact-settlement verifier.
- `spl-transfer-build`: guarded unsigned SPL-transfer proposal builder for refunds/payouts.
- Reproduction scripts, redacted config, operator SOP, agent skill, golden vectors, adversarial tests, and real-host evidence.

## Custody tier and threat model

`solana-pay-request` is T1 request-only: no network, signer, or key access. `solana_pay_confirm` is T0 read-only: it cannot construct or submit transactions. `spl_transfer_build` is T1 build-only: it returns unsigned proposals and cannot sign or submit.

Threat model: prompts, invoices, labels, memos, tool args, and model text are untrusted. The plugins reject unknown fields, derive references from exact invoice terms, use strict decimal parsing, enforce allowlists, and bound output size. Wallet approval is external; RPC remains a verification dependency; delivery/accounting/duplicate handling belong in the SOP.

## Reproduce

- Plugins: `https://github.com/Fianko-codes/zeroclaw-plugins/tree/2c3592afa241603bd34311ba392e7214e9ef0a41`
- Shared Solana core: `https://github.com/Fianko-codes/zeroclaw-solana/tree/09d73652be97a8f348938feac4b022cb049b0c35`

Run from the workspace root:

```bash
./demo/reproduce.sh
```

Supporting packet includes the full write-up, evidence index, SOP, agent skill, redacted config, and video runbook. Secrets are redacted.
