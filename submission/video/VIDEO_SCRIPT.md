# 90-second video script - ZeroClaw Solana Payment Desk

Goal: one continuous desktop recording showing a real Telegram channel, a real
ZeroClaw agent/tool flow, and the phone wallet mirrored with `scrcpy`. No
slides. Do not show seed phrases, private keys, Telegram bot tokens, credentialed
RPC URLs, or personal contact names.

Use `LAST_MINUTE_RECORDING_RUNBOOK.md` for the exact current addresses and setup
commands.

## Fast recording layout

Use a 16:9 desktop recording.

- Left 60%: Telegram chat with the ZeroClaw agent.
- Right 40%: `scrcpy` phone mirror, opened to the devnet Solana wallet.
- Bottom or second terminal tab: only show non-secret checks: QR rendering,
  public signature/explorer, and a short `paid:true`/`paid:false` result if it is
  easier to read there.

Start phone mirroring:

```bash
scrcpy --stay-awake --window-title "Demo wallet"
```

If the phone screen is too tall, resize the scrcpy window manually before
recording. Keep Telegram and the phone visible together.

## 1:30 script

| Time | What to show exactly | Say exactly |
| --- | --- | --- |
| 0:00-0:08 | Telegram chat open beside the mirrored phone wallet. Terminal may show `zeroclaw plugin list` with `solana-pay-request` and `solana-pay-confirm`. | "This is a real ZeroClaw agent running through Telegram. The use case is a non-custodial Solana payment desk: request a payment, approve it in a wallet, then verify it from chain evidence." |
| 0:08-0:25 | In Telegram, send: `Create devnet invoice DEMO-<short-id> for 1.25 <TOKEN> to our configured merchant wallet.` Show the agent response with the Solana Pay URL/reference. | "The agent calls `solana_pay_request`. It creates a Solana Pay URL and deterministic reference from the invoice terms. It cannot sign, submit, or read a private key." |
| 0:25-0:43 | Terminal: run `./demo/qr.sh '<SOLANA_PAY_URL>'` or faster: `adb shell am start -a android.intent.action.VIEW -d '<SOLANA_PAY_URL>'`. On the mirrored phone, open the request in Phantom. Show wallet review with amount and recipient. | "The QR or deep link is outside the model context. The wallet is where approval happens, so ZeroClaw is not custody." |
| 0:43-0:58 | On the phone, approve the transaction. Show success/signature or public explorer/confirmation in terminal. | "This transaction was signed and submitted outside ZeroClaw. The agent only created the request." |
| 0:58-1:15 | Back in Telegram, send: `Confirm invoice DEMO-<short-id>: 1.25 <TOKEN> to our merchant wallet.` Show `paid: true`, `received_ui: "1.25"`, and the signature. | "`solana_pay_confirm` re-derives the same reference, decodes the transaction bytes locally, and only returns paid when the recipient token balance increased by the exact requested amount." |
| 1:15-1:25 | Send the same confirm prompt with `1.24 <TOKEN>` or a changed recipient. Show `paid: false`. | "A changed amount or recipient is not close enough. It becomes a different reference, so the model cannot force a paid result." |
| 1:25-1:30 | Show the final Telegram `paid:true` result beside phone success. Optional terminal line: `request -> external wallet approval -> byte-verified confirmation`. | "That is the submission: a real channel, real wallet approval, and byte-verified confirmation without giving the agent custody." |

If the recording runs long, cut the negative check before cutting the positive
request-pay-confirm flow.

## 25-minute execution plan

1. Spend 3 minutes on the screen layout.
   Open Telegram, terminal, and `scrcpy`. Make sure the phone wallet is on
   devnet and has the disposable demo account selected.

2. Spend 5 minutes on a dry run without recording.
   Use a fresh invoice ID like `DEMO-0807-A`. Create the request, render the QR,
   approve from the phone, and confirm `paid:true`. Also test the `1.24` negative
   confirmation. If this fails, fix it before recording.

3. Spend 2 minutes cleaning the screen.
   Close unrelated browser tabs, clear terminal scrollback containing secrets,
   hide Telegram contact names if needed, and keep only public demo data visible.

4. Spend 3 minutes recording one take.
   Start OBS or SimpleScreenRecorder, speak the script, and do not pause the
   screen during wallet approval. If you stumble but the technical flow is
   visible, keep going.

5. Spend 5 minutes trimming/compressing.
   Use the recorder's trim UI, or run:

   ```bash
   ffmpeg -i raw-demo.mp4 -t 00:01:45 -c:v libx264 -preset veryfast -crf 24 -c:a aac zeroclaw-demo.mp4
   ```

6. Spend 2 minutes checking the final file.
   Play it once. Confirm it shows Telegram, scrcpy wallet approval, `paid:true`,
   and no secrets.

7. Spend the remaining time uploading.
   Upload `zeroclaw-demo.mp4`, then submit the video link plus
   `submission/pdf/ZeroClaw_Submission_Packet.pdf` and the pushed GitHub repos.

## Telegram plugin setup checklist

Use this only if the Telegram channel is not already wired.

1. Create a Telegram bot with BotFather and save the token outside the recording.
2. Install or enable the ZeroClaw Telegram channel plugin:

   ```bash
   zeroclaw plugin install telegram \
     --registry https://raw.githubusercontent.com/JordanTheJet/zeroclaw-plugins/main/registry.json
   ```

3. Configure the Telegram channel with:
   - `bot_token`: secret token from BotFather.
   - `allowed_users`: your Telegram user ID or username, not `"*"`.
   - `parse_mode`: leave empty/plain text for the demo.

4. Configure the agent to use the three Solana tools:
   - `solana-pay-request`
   - `solana-pay-confirm`
   - optional but not necessary for the 90-second video: `spl-transfer-build`

5. Paste the operating rules from `../supporting/AGENT_SKILL.md` into the
   agent/system instruction so it refuses keys, caller-supplied references, and
   arbitrary transaction requests.

6. In Telegram, send `/start` or a simple test message. Confirm the bot replies
   before starting the real invoice flow.

7. Do not expose the bot token, ZeroClaw config file, seed phrase, private key,
   or credentialed RPC URL while recording.
