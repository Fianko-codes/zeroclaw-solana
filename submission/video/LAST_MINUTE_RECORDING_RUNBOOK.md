# Last-minute recording runbook

This is the exact fast path for the current demo wallet.

## Known addresses

- Phantom payer wallet: `FUjdzAArSQrPydoyowKgstGHPed2fLoqr2i5yDnw4f2`
- Disposable merchant recipient: `68QYsqw1QdqZk4mXyRNV8JyxP1FkhGhU1F75oKQ5gRZr`
- Temporary mint authority public key: `4SSaxFfpJYJVN4itJ1Vjdu2SgcjZTuuB5fa2gAfvsUx1`
- Temporary local keypair directory: `/tmp/zeroclaw-video-demo.pveMys`

Do not show `/tmp/zeroclaw-video-demo.pveMys/*.json` or any key-generation
terminal scrollback in the video.

## A. Prepare Phantom

1. Open Phantom mobile.
2. Switch Phantom to Solana Devnet.
   Usually: Settings/Profile -> Developer Settings -> Testnet Mode -> Solana
   Devnet.
3. Confirm the selected account is:
   `FUjdzAArSQrPydoyowKgstGHPed2fLoqr2i5yDnw4f2`.
4. Send `0.05` devnet SOL from Phantom to:
   `4SSaxFfpJYJVN4itJ1Vjdu2SgcjZTuuB5fa2gAfvsUx1`.

That funds the temporary local mint authority so the CLI can create a disposable
demo token.

## B. Create the demo token after funding

Check the temporary mint authority balance:

```bash
solana balance 4SSaxFfpJYJVN4itJ1Vjdu2SgcjZTuuB5fa2gAfvsUx1 --url devnet
```

Create a 6-decimal demo SPL token:

```bash
spl-token create-token --decimals 6 \
  --fee-payer /tmp/zeroclaw-video-demo.pveMys/mint-authority.json \
  --mint-authority 4SSaxFfpJYJVN4itJ1Vjdu2SgcjZTuuB5fa2gAfvsUx1 \
  --url devnet
```

Copy the mint address printed after `Creating token`. Call it `<MINT>`.

Create token accounts for the payer and merchant:

```bash
spl-token create-account <MINT> \
  --owner FUjdzAArSQrPydoyowKgstGHPed2fLoqr2i5yDnw4f2 \
  --fee-payer /tmp/zeroclaw-video-demo.pveMys/mint-authority.json \
  --url devnet
```

```bash
spl-token create-account <MINT> \
  --owner 68QYsqw1QdqZk4mXyRNV8JyxP1FkhGhU1F75oKQ5gRZr \
  --fee-payer /tmp/zeroclaw-video-demo.pveMys/mint-authority.json \
  --url devnet
```

Mint 10 demo tokens to Phantom:

```bash
spl-token mint <MINT> 10 \
  --recipient-owner FUjdzAArSQrPydoyowKgstGHPed2fLoqr2i5yDnw4f2 \
  --mint-authority /tmp/zeroclaw-video-demo.pveMys/mint-authority.json \
  --fee-payer /tmp/zeroclaw-video-demo.pveMys/mint-authority.json \
  --url devnet
```

Verify Phantom has the demo token:

```bash
spl-token accounts --owner FUjdzAArSQrPydoyowKgstGHPed2fLoqr2i5yDnw4f2 --url devnet
```

## C. Configure the Solana tools

Use the real `<MINT>` from step B. The alias should be `DEMO`.

```toml
[[plugins.entries]]
name = "solana-pay-request"

[plugins.entries.config]
mint_aliases = "DEMO=<MINT>"
mint_decimals = "DEMO=6"
default_label = "ZeroClaw demo merchant"
allowed_recipients = "68QYsqw1QdqZk4mXyRNV8JyxP1FkhGhU1F75oKQ5gRZr"

[[plugins.entries]]
name = "solana-pay-confirm"

[plugins.entries.config]
rpc_url = "https://api.devnet.solana.com"
allowed_recipients = "68QYsqw1QdqZk4mXyRNV8JyxP1FkhGhU1F75oKQ5gRZr"
mint_allowlist = "<MINT>"
mint_aliases = "DEMO=<MINT>"
min_commitment = "confirmed"
max_signatures_scanned = "10"
allow_token_2022 = "false"
```

For the demo video, `confirmed` is acceptable because speed matters. Do not say
"finalized" in the video unless your tool output actually says finalized.

## D. Open the phone wallet on the desktop

```bash
scrcpy --stay-awake --window-title "Demo wallet"
```

Put Telegram left, scrcpy right, terminal bottom.

## E. Telegram prompts for the recording

Use a fresh invoice ID:

```text
Create devnet invoice DEMO-0807-A for 1.25 DEMO to merchant wallet 68QYsqw1QdqZk4mXyRNV8JyxP1FkhGhU1F75oKQ5gRZr.
```

Copy the returned `solana:` URL.

Open it on the mirrored phone without scanning:

```bash
adb shell am start -a android.intent.action.VIEW -d '<SOLANA_PAY_URL>'
```

Approve in Phantom.

Then confirm in Telegram:

```text
Confirm invoice DEMO-0807-A: 1.25 DEMO to merchant wallet 68QYsqw1QdqZk4mXyRNV8JyxP1FkhGhU1F75oKQ5gRZr.
```

Expected: `paid: true`, `received_ui: "1.25"`, and a signature.

Optional negative check:

```text
Confirm invoice DEMO-0807-A: 1.24 DEMO to merchant wallet 68QYsqw1QdqZk4mXyRNV8JyxP1FkhGhU1F75oKQ5gRZr.
```

Expected: `paid: false`.

## F. What to record

Record only the clean terminal, Telegram, and scrcpy windows. Do not record:

- keypair generation output;
- config files containing Telegram tokens;
- private key JSON files;
- Phantom recovery phrase;
- personal Telegram chats.

