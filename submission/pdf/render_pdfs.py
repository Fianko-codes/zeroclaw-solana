#!/usr/bin/env python3
"""Render submission-ready ZeroClaw bounty PDFs."""

from __future__ import annotations

import html
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import markdown


ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "submission" / "pdf"
HTML_DIR = OUT_DIR / "_html"

ONE_PAGER = (
    "ZeroClaw_One_Pager",
    "ZeroClaw Solana Payment Desk",
    "One-page Summary",
    ROOT / "submission" / "ONE_PAGER.md",
)

DOCS = [
    (
        "ZeroClaw_Submission_Writeup",
        "ZeroClaw Solana Payment Desk",
        "Submission Write-up",
        ROOT / "submission" / "WRITEUP.md",
    ),
    (
        "ZeroClaw_Evidence_Index",
        "Quality and Reliability Evidence",
        "Supporting Material",
        ROOT / "submission" / "supporting" / "EVIDENCE_INDEX.md",
    ),
    (
        "ZeroClaw_Operator_SOP",
        "Operator Reproduction SOP",
        "Supporting Material",
        ROOT / "submission" / "supporting" / "OPERATOR_SOP.md",
    ),
    (
        "ZeroClaw_Agent_Skill",
        "Bounded Agent Operating Skill",
        "Supporting Material",
        ROOT / "submission" / "supporting" / "AGENT_SKILL.md",
    ),
    (
        "ZeroClaw_Redacted_Config",
        "Redacted Configuration Template",
        "Supporting Material",
        ROOT / "submission" / "supporting" / "redacted-config.toml",
    ),
]

STALE_OUTPUTS = [
    "00_submission_readme.pdf",
    "01_writeup.pdf",
    "02_evidence_index.pdf",
    "03_operator_sop.pdf",
    "04_agent_skill.pdf",
    "05_redacted_config.pdf",
    "ZeroClaw_OnePager.pdf",
    "zeroclaw_submission_packet.pdf",
    "ZeroClaw_Submission_Packet.unoptimized.pdf",
]

ANY_HREF_RE = re.compile(r'<a href="([^"]*)">((?:(?!</a>).)*)</a>', re.S)


CSS = """
@page {
  margin: 18mm 16mm;
}

:root {
  color: #17212b;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont,
    "Segoe UI", sans-serif;
  font-size: 13.5px;
  line-height: 1.48;
}

body {
  margin: 0;
}

.doc-header {
  border-bottom: 2px solid #101820;
  margin-bottom: 24px;
  padding-bottom: 12px;
}

.doc-kicker {
  color: #506070;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.doc-title {
  color: #111820;
  font-size: 27px;
  font-weight: 750;
  line-height: 1.15;
  margin-top: 6px;
}

.doc-context {
  color: #586676;
  font-size: 12px;
  margin-top: 8px;
}

h1 {
  color: #111820;
  font-size: 23px;
  line-height: 1.18;
  margin: 22px 0 12px;
}

h2 {
  border-bottom: 1px solid #d7dee7;
  color: #111820;
  font-size: 17px;
  line-height: 1.25;
  margin: 21px 0 9px;
  padding-bottom: 4px;
}

h3 {
  color: #263241;
  font-size: 14px;
  margin: 18px 0 7px;
}

p,
ul,
ol,
pre,
table {
  margin: 0 0 10px;
}

ul,
ol {
  padding-left: 20px;
}

li {
  margin: 3px 0;
}

a {
  color: #0f5f9c;
  text-decoration: none;
}

.local-ref {
  color: #263241;
  font-weight: 600;
}

code {
  background: #f2f5f8;
  border: 1px solid #e0e6ed;
  border-radius: 4px;
  color: #111820;
  font-family: "SFMono-Regular", Consolas, "Liberation Mono", monospace;
  font-size: 0.92em;
  padding: 1px 4px;
}

pre {
  background: #f7f9fb;
  border: 1px solid #d8e0e8;
  border-radius: 6px;
  overflow-wrap: anywhere;
  padding: 10px 12px;
  white-space: pre-wrap;
}

pre code {
  background: transparent;
  border: 0;
  border-radius: 0;
  padding: 0;
}

table {
  border-collapse: collapse;
  page-break-inside: auto;
  width: 100%;
  word-break: normal;
}

tr {
  page-break-inside: avoid;
}

th,
td {
  border: 1px solid #d8e0e8;
  padding: 6px 8px;
  text-align: left;
  vertical-align: top;
}

th {
  background: #eef3f7;
  color: #111820;
  font-weight: 700;
}

blockquote {
  border-left: 3px solid #aab7c4;
  color: #3d4b59;
  margin: 0 0 10px;
  padding: 3px 0 3px 12px;
}

.packet-cover {
  align-items: flex-start;
  display: flex;
  flex-direction: column;
  min-height: 700px;
}

.packet-cover h1 {
  font-size: 34px;
  letter-spacing: 0;
  margin: 64px 0 14px;
}

.packet-cover .subtitle {
  color: #3f4d5c;
  font-size: 16px;
  margin-bottom: 34px;
  max-width: 620px;
}

.packet-cover .meta {
  border-top: 1px solid #d7dee7;
  color: #3f4d5c;
  margin-top: auto;
  padding-top: 16px;
  width: 100%;
}

.one-pager {
  font-size: 10.8px;
  line-height: 1.24;
}

.one-pager .doc-header {
  margin-bottom: 8px;
  padding-bottom: 6px;
}

.one-pager .doc-title {
  font-size: 20px;
}

.one-pager h1 {
  font-size: 17px;
  margin: 0 0 5px;
}

.one-pager h2 {
  font-size: 12px;
  margin: 7px 0 3px;
  padding-bottom: 2px;
}

.one-pager p,
.one-pager ul,
.one-pager ol,
.one-pager pre {
  margin-bottom: 4px;
}

.one-pager li {
  margin: 0;
}

.one-pager ul,
.one-pager ol {
  padding-left: 17px;
}

.one-pager pre {
  padding: 6px 8px;
}
"""


def run(args: list[str]) -> None:
    completed = subprocess.run(args, cwd=ROOT, text=True, capture_output=True)
    if completed.returncode != 0:
        sys.stderr.write(completed.stdout)
        sys.stderr.write(completed.stderr)
        raise SystemExit(completed.returncode)


def strip_local_links(body: str) -> str:
    """Keep external links clickable; render local workspace links as text.

    Relative Markdown links otherwise become file:// PDF annotations when
    headless Chrome prints the temporary HTML. That leaks local paths and gives
    judges dead links. The supporting packet is the local material; only public
    web URLs should remain clickable.
    """

    def replace(match: re.Match[str]) -> str:
        href = html.unescape(match.group(1))
        label = match.group(2)
        if href.startswith(("http:", "https:", "mailto:")):
            return match.group(0)
        return f'<span class="local-ref">{label}</span>'

    return ANY_HREF_RE.sub(replace, body)


def markdown_to_html(source: Path) -> str:
    text = source.read_text(encoding="utf-8")
    if source.suffix == ".toml":
        return f"<pre><code>{html.escape(text)}</code></pre>"
    rendered = markdown.markdown(
        text,
        extensions=[
            "extra",
            "fenced_code",
            "sane_lists",
            "tables",
            "toc",
        ],
        output_format="html5",
    )
    return strip_local_links(rendered)


def wrap_html(title: str, section: str, body: str, body_class: str = "") -> str:
    body_class_attr = f' class="{html.escape(body_class)}"' if body_class else ""
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{html.escape(title)}</title>
  <style>{CSS}</style>
</head>
<body{body_class_attr}>
  <header class="doc-header">
    <div class="doc-kicker">Superteam ZeroClaw bounty</div>
    <div class="doc-title">{html.escape(title)}</div>
    <div class="doc-context">{html.escape(section)}</div>
  </header>
  <main>{body}</main>
</body>
</html>
"""


def cover_html() -> str:
    items = "\n".join(
        f"<li>{html.escape(title)} <span>({html.escape(section)})</span></li>"
        for _, title, section, _ in DOCS
    )
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>ZeroClaw Solana Payment Desk Submission Packet</title>
  <style>{CSS}</style>
</head>
<body>
  <main class="packet-cover">
    <div class="doc-kicker">Superteam ZeroClaw bounty</div>
    <h1>ZeroClaw Solana Payment Desk</h1>
    <p class="subtitle">A direct submission packet for a chat-native, non-custodial Solana payment workflow: request, externally sign, and byte-verified confirmation.</p>
    <h2>Included documents</h2>
    <ul>{items}</ul>
    <div class="meta">Secrets redacted. Code repositories are linked from the write-up.</div>
  </main>
</body>
</html>
"""


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)

    if HTML_DIR.exists():
        shutil.rmtree(HTML_DIR)

    for stale in STALE_OUTPUTS:
        path = OUT_DIR / stale
        if path.exists():
            path.unlink()

    with tempfile.TemporaryDirectory(prefix="zeroclaw-pdf-") as html_tmp:
        html_dir = Path(html_tmp)
        one_slug, one_title, one_section, one_source = ONE_PAGER
        one_html_path = html_dir / f"{one_slug}.html"
        one_pdf = OUT_DIR / f"{one_slug}.pdf"
        one_body = markdown_to_html(one_source)
        one_html_path.write_text(
            wrap_html(one_title, one_section, one_body, "one-pager"),
            encoding="utf-8",
        )
        run(
            [
                "google-chrome-stable",
                "--headless=new",
                "--disable-gpu",
                "--no-sandbox",
                "--no-pdf-header-footer",
                "--print-to-pdf-no-header",
                "--run-all-compositor-stages-before-draw",
                "--virtual-time-budget=10000",
                f"--print-to-pdf={one_pdf}",
                one_html_path.as_uri(),
            ]
        )

        cover_path = html_dir / "00_packet_cover.html"
        cover_pdf = OUT_DIR / "ZeroClaw_Submission_Cover.pdf"
        cover_path.write_text(cover_html(), encoding="utf-8")
        run(
            [
                "google-chrome-stable",
                "--headless=new",
                "--disable-gpu",
                "--no-sandbox",
                "--no-pdf-header-footer",
                "--print-to-pdf-no-header",
                "--run-all-compositor-stages-before-draw",
                "--virtual-time-budget=10000",
                f"--print-to-pdf={cover_pdf}",
                cover_path.as_uri(),
            ]
        )

        pdfs: list[Path] = []
        for slug, title, section, source in DOCS:
            body = markdown_to_html(source)
            html_path = html_dir / f"{slug}.html"
            pdf_path = OUT_DIR / f"{slug}.pdf"
            html_path.write_text(wrap_html(title, section, body), encoding="utf-8")
            run(
                [
                    "google-chrome-stable",
                    "--headless=new",
                    "--disable-gpu",
                    "--no-sandbox",
                    "--no-pdf-header-footer",
                    "--print-to-pdf-no-header",
                    "--run-all-compositor-stages-before-draw",
                    "--virtual-time-budget=10000",
                    f"--print-to-pdf={pdf_path}",
                    html_path.as_uri(),
                ]
            )
            pdfs.append(pdf_path)

        packet = OUT_DIR / "ZeroClaw_Submission_Packet.pdf"
        packet_raw = OUT_DIR / "ZeroClaw_Submission_Packet.unoptimized.pdf"
        if packet_raw.exists():
            packet_raw.unlink()
        if packet.exists():
            packet.unlink()

        run(["pdfunite", str(cover_pdf), *(str(pdf) for pdf in pdfs), str(packet_raw)])
        run(["qpdf", "--warning-exit-0", "--linearize", str(packet_raw), str(packet)])
        packet_raw.unlink()

        print("Generated PDFs:")
        for pdf in [one_pdf, cover_pdf, *pdfs, packet]:
            print(pdf.relative_to(ROOT))


if __name__ == "__main__":
    main()
