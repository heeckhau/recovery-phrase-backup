#!/usr/bin/env bash
# Builds slides.html: one self-contained file (images inlined, native emoji)
# that opens offline in any browser.
set -euo pipefail
cd "$(dirname "$0")"

marp="npx --yes @marp-team/marp-cli@4.1.2"
# --no-stdin: otherwise marp waits for input whenever stdin isn't a terminal.
$marp slides.md --no-stdin --config marp.config.js --html -o slides.html

python3 - <<'EOF'
import base64, re
html = open("slides.html").read()
for path, mime in [("../.github/screenshot.png", "image/png"), ("share-card.png", "image/png"), ("qr.svg", "image/svg+xml")]:
    data = base64.b64encode(open(path, "rb").read()).decode()
    html = html.replace(path, f"data:{mime};base64,{data}")
left = re.findall(r'(?:src="|url\(&quot;)(?!data:)[^"&]+', html)
assert not left, f"external references left: {left}"
open("slides.html", "w").write(html)
EOF
echo "built slides.html"
