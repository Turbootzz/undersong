# Undersong — Companion Score (wiki)

A static companion site generated from the game's own content files.

```bash
cargo run -p tools -- wiki   # refresh data + sprites from content/
cd wiki
bun install
bun run dev                  # local browsing
bun run build                # static dist/ — host it anywhere
```

Spoilers (legendaries, late-game locations) hide behind the header
toggle; the choice persists in localStorage.
