#!/usr/bin/env bash
# Render a Solana Pay request as a scannable QR code in the terminal.
#
# The `solana_pay_request` tool deliberately returns the URL and never QR art:
# a rendered QR is 1–3 KB of non-semantic characters charged against every
# operator's context window on every call, and the URL *is* the QR content. So
# rendering happens out here, outside the model's context.
#
# Usage:
#   demo/qr.sh 'solana:FnHy…?amount=1.5&spl-token=EPjF…&reference=3FrM…'
#   zeroclaw agent -a m2 -m 'charge invoice 412 …' | demo/qr.sh --stdin
#
# --stdin reads the tool's JSON output and extracts qr_payload.
set -euo pipefail

if ! command -v qrencode >/dev/null; then
  echo "qrencode is required: pacman -S qrencode | apt install qrencode" >&2
  exit 2
fi

if [[ "${1:-}" == "--stdin" ]]; then
  payload=$(python3 -c '
import json, re, sys
raw = sys.stdin.read()
for match in re.finditer(r"\{.*?\}", raw, re.S):
    try:
        value = json.loads(match.group())
    except json.JSONDecodeError:
        continue
    for key in ("qr_payload", "url"):
        if isinstance(value.get(key), str) and value[key].startswith("solana:"):
            print(value[key])
            sys.exit(0)
sys.exit("no solana: payload found on stdin")
')
else
  payload="${1:-}"
fi

if [[ -z "$payload" || "$payload" != solana:* ]]; then
  echo "expected a solana: URL (got ${payload:-nothing})" >&2
  exit 2
fi

# Show what is being encoded, then the code itself. Anyone scanning this should
# be able to read the terms first.
python3 - "$payload" <<'PY'
import sys
from urllib.parse import unquote_plus
url = sys.argv[1]
recipient, _, query = url[len("solana:"):].partition("?")
print(f"\033[1mrecipient\033[0m {recipient}")
for field in query.split("&"):
    key, _, value = field.partition("=")
    print(f"\033[1m{key:>10}\033[0m {unquote_plus(value)}")
PY
printf '\n'
qrencode -t ansiutf8 -- "$payload"
printf '\nScan with any Solana Pay wallet. Confirm it later with:\n'
printf '  solana_pay_confirm { recipient, amount, mint, invoice_id }\n'
printf 'which re-derives this exact reference from those four fields.\n'
