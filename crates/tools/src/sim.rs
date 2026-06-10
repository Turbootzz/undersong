//! Balance Monte Carlo (`tools simulate`) and shared battle-driving
//! helpers (docs/06-ROADMAP.md P1, doc 04 §4 batch gate).
//!
//! All randomness flows through `BattleRng` from one master seed, so a
//! report is exactly reproducible from its command line.

use std::collections::BTreeMap;

use battle::ai::{AiTier, choose};
use battle::{BattleKind, BattleMote, BattleState, MoteBuilder, Outcome, TurnActions, step};
use data::content::{MoveSet, SpeciesPool};
use undersong_core::chart::TypeChart;
use undersong_core::rng::BattleRng;
use undersong_core::species::{SpeciesSpec, StatSpread};
use undersong_core::types::Type;

pub struct SimConfig {
    pub battles: u32,
    pub level: u8,
    pub team_size: usize,
    pub seed: u64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            battles: 1000,
            level: 30,
            team_size: 3,
            seed: 0x00C0_FFEE,
        }
    }
}

/// Builds one Mote: random IVs (0–31), random nature, the last ≤4
/// learnset moves available at `level`.
fn random_mote(spec: &SpeciesSpec, level: u8, moves: &MoveSet, rng: &mut BattleRng) -> BattleMote {
    let ivs = StatSpread {
        hp: u16::try_from(rng.below(32)).expect("0..32"),
        atk: u16::try_from(rng.below(32)).expect("0..32"),
        def: u16::try_from(rng.below(32)).expect("0..32"),
        spa: u16::try_from(rng.below(32)).expect("0..32"),
        spd: u16::try_from(rng.below(32)).expect("0..32"),
        spe: u16::try_from(rng.below(32)).expect("0..32"),
    };
    let nature = u8::try_from(rng.below(25)).expect("0..25");
    let known: Vec<_> = spec
        .learnset
        .iter()
        .filter(|(l, _)| *l <= level)
        .filter_map(|(_, id)| moves.get(id).cloned())
        .collect();
    let picked = known.iter().rev().take(4).rev().cloned().collect();
    MoteBuilder::new(spec, level)
        .ivs(ivs)
        .nature(nature)
        .moves(picked)
        .build()
}

/// A team of `size` distinct random species from the pool.
fn random_team(
    pool: &SpeciesPool,
    size: usize,
    level: u8,
    moves: &MoveSet,
    rng: &mut BattleRng,
) -> Vec<BattleMote> {
    let mut indices: Vec<usize> = Vec::new();
    while indices.len() < size.min(pool.species.len()) {
        let pick =
            usize::try_from(rng.below(u32::try_from(pool.species.len()).expect("pool fits u32")))
                .expect("index");
        if !indices.contains(&pick) {
            indices.push(pick);
        }
    }
    indices
        .into_iter()
        .map(|i| random_mote(&pool.species[i], level, moves, rng))
        .collect()
}

/// Drives one battle to completion with per-side AI tiers. Returns the
/// outcome and the number of turns played.
pub fn play_out(mut state: BattleState, tiers: [AiTier; 2], rng: &mut BattleRng) -> (Outcome, u16) {
    loop {
        if let Some(outcome) = state.outcome {
            return (outcome, state.turn);
        }
        let action0 = choose(tiers[0], &state, 0, rng);
        let action1 = choose(tiers[1], &state, 1, rng);
        let (next, _) = step(&state, &TurnActions::new(action0, action1), rng);
        state = next;
    }
}

pub struct SimReport {
    pub battles: u32,
    pub draws: u32,
    pub total_turns: u64,
    pub species_results: BTreeMap<String, (u32, u32)>, // (wins, battles)
    pub type_results: BTreeMap<Type, (u32, u32)>,
    pub t2_vs_t0_wins: u32,
    pub t3_vs_t1_wins: u32,
    pub t3_vs_t0_wins: u32,
    pub t2_vs_t0_battles: u32,
}

impl SimReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        let avg_turns = if self.battles > 0 {
            self.total_turns / u64::from(self.battles)
        } else {
            0
        };
        out.push_str(&format!(
            "simulate: {} battles, avg {} turns, {} draws\n",
            self.battles, avg_turns, self.draws
        ));
        out.push_str("\nper-species win rates (random T2 mirror pools):\n");
        let mut rows: Vec<_> = self.species_results.iter().collect();
        rows.sort_by(|a, b| {
            let rate_a = a.1.0 * 1000 / a.1.1.max(1);
            let rate_b = b.1.0 * 1000 / b.1.1.max(1);
            rate_b.cmp(&rate_a)
        });
        for (species, (wins, total)) in rows {
            out.push_str(&format!(
                "  {species:<20} {:>5.1}%  ({wins}/{total})\n",
                f64::from(*wins) * 100.0 / f64::from((*total).max(1))
            ));
        }
        out.push_str("\nper-type aggregate win rates:\n");
        for (ty, (wins, total)) in &self.type_results {
            let name = format!("{ty:?}");
            out.push_str(&format!(
                "  {name:<10} {:>5.1}%  ({wins}/{total})\n",
                f64::from(*wins) * 100.0 / f64::from((*total).max(1))
            ));
        }
        out.push_str(&format!(
            "\nT2 vs T0 (equal teams): {:.1}% ({}/{})  [gate: ≥ 90%]\nT3 vs T0 (equal teams): {:.1}% ({}/{})  [gate: ≥ 90%]\nT3 vs T1 (equal teams): {:.1}% ({}/{})  [gate: ≥ 45%, §14 v1.7 mirror plateau]\n",
            f64::from(self.t2_vs_t0_wins) * 100.0 / f64::from(self.t2_vs_t0_battles.max(1)),
            self.t2_vs_t0_wins,
            self.t2_vs_t0_battles,
            f64::from(self.t3_vs_t0_wins) * 100.0 / f64::from(self.t2_vs_t0_battles.max(1)),
            self.t3_vs_t0_wins,
            self.t2_vs_t0_battles,
            f64::from(self.t3_vs_t1_wins) * 100.0 / f64::from(self.t2_vs_t0_battles.max(1)),
            self.t3_vs_t1_wins,
            self.t2_vs_t0_battles
        ));
        out
    }

    pub fn t2_rate_percent(&self) -> f64 {
        f64::from(self.t2_vs_t0_wins) * 100.0 / f64::from(self.t2_vs_t0_battles.max(1))
    }

    pub fn t3_rate_percent(&self) -> f64 {
        f64::from(self.t3_vs_t1_wins) * 100.0 / f64::from(self.t2_vs_t0_battles.max(1))
    }

    pub fn t3_vs_t0_rate_percent(&self) -> f64 {
        f64::from(self.t3_vs_t0_wins) * 100.0 / f64::from(self.t2_vs_t0_battles.max(1))
    }
}

/// Runs the full Monte Carlo: random-team T2 mirrors for balance stats,
/// plus the T2-vs-T0 equal-team regression (doc 02 §14 gate).
pub fn simulate(
    pool: &SpeciesPool,
    moves: &MoveSet,
    chart: &TypeChart,
    config: &SimConfig,
) -> SimReport {
    let mut master = BattleRng::from_seed(config.seed);
    let mut report = SimReport {
        battles: config.battles,
        draws: 0,
        total_turns: 0,
        species_results: BTreeMap::new(),
        type_results: BTreeMap::new(),
        t2_vs_t0_wins: 0,
        t3_vs_t1_wins: 0,
        t3_vs_t0_wins: 0,
        t2_vs_t0_battles: config.battles,
    };

    // Balance pass: random teams, T2 both sides.
    for _ in 0..config.battles {
        let seed = (u64::from(master.next_u32()) << 32) | u64::from(master.next_u32());
        let mut rng = BattleRng::from_seed(seed);
        let team0 = random_team(pool, config.team_size, config.level, moves, &mut rng);
        let team1 = random_team(pool, config.team_size, config.level, moves, &mut rng);
        let roster0: Vec<String> = team0.iter().map(|m| m.species.to_string()).collect();
        let roster1: Vec<String> = team1.iter().map(|m| m.species.to_string()).collect();
        let types0: Vec<Type> = team0.iter().flat_map(|m| m.types.clone()).collect();
        let types1: Vec<Type> = team1.iter().flat_map(|m| m.types.clone()).collect();

        let state = BattleState::new(BattleKind::Trainer, team0, team1, chart.clone());
        let (outcome, turns) = play_out(state, [AiTier::T2, AiTier::T2], &mut rng);
        report.total_turns += u64::from(turns);

        let winner = match outcome {
            Outcome::Won { winner } => Some(winner),
            _ => {
                report.draws += 1;
                None
            }
        };
        for (side, roster, types) in [(0u8, roster0, types0), (1u8, roster1, types1)] {
            let won = winner == Some(side);
            for species in roster {
                let entry = report.species_results.entry(species).or_insert((0, 0));
                entry.1 += 1;
                if won {
                    entry.0 += 1;
                }
            }
            for ty in types {
                let entry = report.type_results.entry(ty).or_insert((0, 0));
                entry.1 += 1;
                if won {
                    entry.0 += 1;
                }
            }
        }
    }

    // Tier regression: identical teams, T2 (side 0) vs T0 (side 1).
    for _ in 0..config.battles {
        let seed = (u64::from(master.next_u32()) << 32) | u64::from(master.next_u32());
        let mut rng = BattleRng::from_seed(seed);
        let team = random_team(pool, config.team_size, config.level, moves, &mut rng);
        let state = BattleState::new(BattleKind::Trainer, team.clone(), team, chart.clone());
        let (outcome, _) = play_out(state, [AiTier::T2, AiTier::T0], &mut rng);
        if matches!(outcome, Outcome::Won { winner: 0 }) {
            report.t2_vs_t0_wins += 1;
        }
    }

    // Tier gate (roadmap P4): identical teams, T3 (side 0) vs T1.
    for _ in 0..config.battles {
        let seed = (u64::from(master.next_u32()) << 32) | u64::from(master.next_u32());
        let mut rng = BattleRng::from_seed(seed);
        let team = random_team(pool, config.team_size, config.level, moves, &mut rng);
        let state = BattleState::new(BattleKind::Trainer, team.clone(), team, chart.clone());
        let (outcome, _) = play_out(state, [AiTier::T3, AiTier::T1], &mut rng);
        if matches!(outcome, Outcome::Won { winner: 0 }) {
            report.t3_vs_t1_wins += 1;
        }
    }
    for _ in 0..config.battles {
        let seed = (u64::from(master.next_u32()) << 32) | u64::from(master.next_u32());
        let mut rng = BattleRng::from_seed(seed);
        let team = random_team(pool, config.team_size, config.level, moves, &mut rng);
        let state = BattleState::new(BattleKind::Trainer, team.clone(), team, chart.clone());
        let (outcome, _) = play_out(state, [AiTier::T3, AiTier::T0], &mut rng);
        if matches!(outcome, Outcome::Won { winner: 0 }) {
            report.t3_vs_t0_wins += 1;
        }
    }

    report
}

/// One battle for the CLI runner: two random teams, both T2.
pub fn one_battle(
    pool: &SpeciesPool,
    moves: &MoveSet,
    chart: &TypeChart,
    level: u8,
    team_size: usize,
    seed: u64,
) -> (BattleState, Vec<battle::BattleEvent>, BattleRng) {
    let mut rng = BattleRng::from_seed(seed);
    let team0 = random_team(pool, team_size, level, moves, &mut rng);
    let team1 = random_team(pool, team_size, level, moves, &mut rng);
    let state = BattleState::new(BattleKind::Trainer, team0, team1, chart.clone());
    (state, Vec::new(), rng)
}

/// Convenience: drives a battle while collecting the full event stream.
pub fn play_out_with_events(
    mut state: BattleState,
    tiers: [AiTier; 2],
    rng: &mut BattleRng,
) -> (Outcome, Vec<battle::BattleEvent>) {
    let mut all_events = Vec::new();
    loop {
        if let Some(outcome) = state.outcome {
            return (outcome, all_events);
        }
        let action0 = choose(tiers[0], &state, 0, rng);
        let action1 = choose(tiers[1], &state, 1, rng);
        let (next, events) = step(&state, &TurnActions::new(action0, action1), rng);
        all_events.extend(events);
        state = next;
    }
}
