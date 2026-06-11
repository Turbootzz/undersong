#!/usr/bin/env bash
# film.sh — unattended capture wrapper around the game's dev rigs
# (macOS has no `timeout`; the rigs film but never exit the app).
#
# Usage:
#   scripts/film.sh shot   <out.png>      [env KEY=VAL ...]  # boot → auto-shot → kill
#   scripts/film.sh replay <replay.ron>   [max_secs] [env...] # film a run → kill
#
# Builds first so the run starts instantly; polls for the artifact
# (shot file / frame series going quiet) and kills the app when done.
set -euo pipefail
cd "$(dirname "$0")/.."

mode="${1:?shot|replay}"
shift

cargo build -p game --quiet

run_game() {
  env "$@" ./target/debug/game >/tmp/undersong-film.log 2>&1 &
  GAME_PID=$!
}

case "$mode" in
  shot)
    out="${1:?output png}"
    shift
    rm -f "$out"
    run_game UNDERSONG_SHOT="$out" "$@"
    for _ in $(seq 1 30); do
      [ -f "$out" ] && break
      sleep 1
    done
    sleep 2 # let the encoder finish writing
    kill "$GAME_PID" 2>/dev/null || true
    wait "$GAME_PID" 2>/dev/null || true
    [ -f "$out" ] && echo "shot: $out" || { echo "no shot produced; log tail:"; tail -5 /tmp/undersong-film.log; exit 1; }
    ;;
  replay)
    file="${1:?replay ron}"
    max="${2:-90}"
    shift; shift 2>/dev/null || true
    stem="$(basename "$file" .ron)"
    dir="docs/playtests/$stem"
    rm -rf "$dir"
    run_game UNDERSONG_VISUAL_REPLAY="$file" "$@"
    quiet=0
    for _ in $(seq 1 "$max"); do
      sleep 1
      count="$(ls "$dir" 2>/dev/null | wc -l | tr -d ' ')"
      last="${last:-0}"
      if [ "$count" -gt 0 ] && [ "$count" = "$last" ]; then
        quiet=$((quiet + 1))
        [ "$quiet" -ge 6 ] && break # series went quiet: run ended
      else
        quiet=0
      fi
      last="$count"
    done
    kill "$GAME_PID" 2>/dev/null || true
    wait "$GAME_PID" 2>/dev/null || true
    echo "filmed $(ls "$dir" 2>/dev/null | wc -l | tr -d ' ') frames → $dir"
    ;;
  *)
    echo "unknown mode: $mode" >&2
    exit 1
    ;;
esac
