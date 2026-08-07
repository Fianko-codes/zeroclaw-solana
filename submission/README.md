# ZeroClaw × Solana submission bundle

This folder is the hand-off package for the Superteam ZeroClaw bounty
submission. It is written to be pasted into the submission form and used to
record a short, real demonstration.

## Contents

- [ONE_PAGER.md](./ONE_PAGER.md) — one-page submission summary.
- [WRITEUP.md](./WRITEUP.md) — detailed project write-up for supporting
  material.
- [video/VIDEO_SCRIPT.md](./video/VIDEO_SCRIPT.md) — a 90-second, shot-by-shot
  Telegram + scrcpy phone recording script; no slides required.
- [supporting/OPERATOR_SOP.md](./supporting/OPERATOR_SOP.md) — reproducible,
  safe setup and recording procedure.
- [supporting/AGENT_SKILL.md](./supporting/AGENT_SKILL.md) — the bounded
  payment-desk behavior to give the channel agent.
- [supporting/redacted-config.toml](./supporting/redacted-config.toml) —
  redacted public configuration template.
- [supporting/EVIDENCE_INDEX.md](./supporting/EVIDENCE_INDEX.md) — validation,
  audit, real-host, real-network, and reproducibility evidence.

## Submission checklist

- [ ] Push the two source repositories and make the URLs in `WRITEUP.md`
  reachable.
- [ ] Follow the SOP with a disposable devnet wallet and mint.
- [ ] Record the exact flow in `video/VIDEO_SCRIPT.md`; replace every
  `<PLACEHOLDER>` only in the recording, never in source control.
- [ ] Upload the resulting video and paste the links listed in `WRITEUP.md`.
- [ ] Confirm that no seed phrase, private key, RPC credentials, session token,
  or personal customer data appears in the terminal, phone, or video metadata.

The source and evidence make no claim that a private key is ever supplied to
ZeroClaw or to one of these components.
