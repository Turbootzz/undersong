# Undersong — Difficulty audit (P7)

Method: the gate-replay driver's *measured* requirements (the level at
which its T3-policy party first beats each wall reliably) against each
maestro's ace level. The driver plays worse than a human (no manual
switching reads, no item timing) — treat its numbers as the curve's
upper bound.

| Wall | Ace | Driver needed | Margin | Verdict |
|---|---|---|---|---|
| Dario (1) | 10 | 13–14 | +3 | comfortable tutorial |
| Mirelle (2) | 17 | 17–19 | +1 | first real check ✓ |
| Stelt (3, doubles) | 22 | 26–29 + trained partner | +5 | doubles spike — intended, but watch playtests |
| Rhea (4) | 31 | 31–34 | +1 | fair |
| Maren (5) | 36 | 38 | +2 | fair after the damper ruling |
| Orsk (6) | 40 | 38–40 | 0 | generous (stall test, not a damage race) |
| Ilva (7) | 46 | 47 | +1 | hardest pre-league ✓ by design |
| Calder (8) | 50 | 47–50 | 0 | fair |
| Quartet | 52–58 | 50–55 + potions | ~0 | endurance, not spike |
| Vault guards | 56–58 | ~55 | 0 | epilogue-grade |

Observations:
1. The Stelt spike is the curve's one anomaly (+5): doubles punish a
   2-mote party hard. The campaign teaches the third catch right
   before it (Route 4's fields) — keep that signposting loud.
2. Whiteout money-halving is brutal for struggling players (the bot's
   economy collapsed repeatedly). QoL candidate: cap the loss.
3. Grinding shoulders: 11→13 (pre-Dario), 23→29 (pre-Stelt), 31→34,
   →38, →47. The →47 stretch before Ilva is the longest; Route 6's
   field exp partially covers it.

## QoL audit list (parked for post-ship)

- [ ] Whiteout loss cap (half, but max ~2000?) — needs a doc 02 ruling.
- [ ] Repels (no anti-encounter item exists; long backtracks grate).
- [ ] PP restoratives (rests only; no field ethers).
- [ ] A run-toggle / faster walk.
- [ ] Doubles target picker in the windowed presenter (still slot-0).
- [ ] Box: multi-deposit, search.
- [ ] An in-game Score% readout beyond Reed's line (the gate is opaque).
