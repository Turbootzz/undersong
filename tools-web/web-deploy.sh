#!/usr/bin/env bash
# Undersong web build (doc 06 P7): wasm32 + bindgen + itch.io shell.
# Usage: tools-web/web-deploy.sh [out-dir]   (default: dist/web)
set -euo pipefail
OUT="${1:-dist/web}"
command -v wasm-bindgen >/dev/null || {
  echo "wasm-bindgen-cli missing: cargo install wasm-bindgen-cli" >&2
  exit 1
}
cargo build -p game --release --target wasm32-unknown-unknown
mkdir -p "$OUT"
wasm-bindgen --target web --no-typescript \
  --out-dir "$OUT" \
  target/wasm32-unknown-unknown/release/game.wasm
cp -R assets "$OUT/assets"
cat > "$OUT/index.html" <<'HTML'
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Undersong</title>
  <style>
    html, body { margin: 0; height: 100%; background: #16131f; }
    body { display: grid; place-items: center; }
    canvas { image-rendering: pixelated; outline: none; }
    #boot { color: #e8dcc3; font: 14px monospace; }
  </style>
</head>
<body>
  <div id="boot">tuning…&nbsp;(press any key once it loads — the first
  keypress is also the audio unlock)</div>
  <script type="module">
    import init from './game.js';
    init().then(() => document.getElementById('boot').remove());
  </script>
</body>
</html>
HTML
( cd "$OUT" && zip -qr ../undersong-web.zip . )
echo "web build → $OUT (zip: dist/undersong-web.zip)"
