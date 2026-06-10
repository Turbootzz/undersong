//! The damage pipeline (doc 02 §4) — exact order, floor at every step,
//! integer-only. Multipliers are rationals applied one at a time.

use undersong_core::chart::TypeChart;
use undersong_core::moves::{MoveCategory, MoveSpec, WeatherKind};
use undersong_core::types::{Eff, Type};

use crate::abilities::Ability;
use crate::mote::{BattleMote, MajorStatus};
use crate::stats::{StageStat, Stages, stage_multiplied};

/// Everything the pipeline needs about one strike.
pub struct DamageContext<'a> {
    pub attacker: &'a BattleMote,
    pub defender: &'a BattleMote,
    pub attacker_stages: &'a Stages,
    pub defender_stages: &'a Stages,
    pub chart: &'a TypeChart,
    pub weather: Option<WeatherKind>,
    pub crit: bool,
    /// Uniform 85..=100 (doc 02 §4), rolled by the caller.
    pub rand: u8,
    /// In doubles, a spread move hitting ≥2 targets takes ×0.75; launch
    /// moves are single-target, so this stays false (doc 02 v1.5 #2).
    pub spread: bool,
    /// Format flag for soloist/chorister (doc 02 §10).
    pub doubles: bool,
}

pub struct DamageOutcome {
    pub amount: u32,
    /// Collapsed classification for the event stream: the exact product
    /// can be 4× or ¼× on dual types; events only need the class.
    pub effectiveness: Eff,
    pub type_product: (u32, u32),
}

/// One `floor(value · num / den)` pipeline step, in u64.
fn mul(value: u64, num: u64, den: u64) -> u64 {
    value * num / den
}

/// Computes one hit. Returns `None` for status moves and `0×` immunity
/// (the caller emits the no-effect message for the latter).
pub fn compute_damage(spec: &MoveSpec, ctx: &DamageContext<'_>) -> Option<DamageOutcome> {
    if spec.power == 0 || matches!(spec.category, MoveCategory::Status) {
        return None;
    }

    // tuning_fork (doc 02 §10): feral moves strike as resonant, ×1.2
    // (applied at the ability step below). Type shift affects the chart
    // product and STAB.
    let move_type = if ctx.attacker.ability == Ability::TuningFork && spec.r#type == Type::Feral {
        Type::Resonant
    } else {
        spec.r#type
    };

    let type_product = ctx.chart.product(move_type, &ctx.defender.types);
    let effectiveness = classify(type_product);

    // A/D: physical → atk/def, special → spa/spd; stage-modified, with
    // crit ignoring the attacker's negative offensive stages and the
    // defender's positive defensive stages (doc 02 §4).
    let (atk_stat, def_stat, atk_stage_stat, def_stage_stat) = match spec.category {
        MoveCategory::Physical => (
            ctx.attacker.stats.atk,
            ctx.defender.stats.def,
            StageStat::Atk,
            StageStat::Def,
        ),
        MoveCategory::Special => (
            ctx.attacker.stats.spa,
            ctx.defender.stats.spd,
            StageStat::Spa,
            StageStat::Spd,
        ),
        MoveCategory::Status => unreachable!("status handled above"),
    };
    let mut atk_stage = ctx.attacker_stages.get(atk_stage_stat);
    let mut def_stage = ctx.defender_stages.get(def_stage_stat);
    if ctx.crit {
        atk_stage = atk_stage.max(0);
        def_stage = def_stage.min(0);
    }
    let a = u64::from(stage_multiplied(u32::from(atk_stat), atk_stage));
    let mut d = u64::from(stage_multiplied(u32::from(def_stat), def_stage));
    // Dustchord: stone-type defenders take special hits at spd ×3/2
    // (doc 02 §7, pipeline position pinned by v1.2 #9).
    if matches!(spec.category, MoveCategory::Special)
        && matches!(ctx.weather, Some(WeatherKind::Dustchord))
        && ctx.defender.types.contains(&Type::Stone)
    {
        d = d * 3 / 2;
    }
    let d = d.max(1);

    // base = floor(floor(floor(2·Level/5 + 2) · Power · A/D) / 50) + 2
    let level_term = 2 * u64::from(ctx.attacker.level) / 5 + 2;
    let mut damage = level_term * u64::from(spec.power) * a / d / 50 + 2;

    // × spread (doubles only)
    if ctx.spread {
        damage = mul(damage, 3, 4);
    }
    // × weather (doc 02 §7: heatwave/downpour boost or hinder ember/tide)
    if let Some(weather) = ctx.weather {
        let factor = weather_factor(weather, move_type);
        damage = mul(damage, factor.0, factor.1);
    }
    // × crit 2.0
    if ctx.crit {
        damage = mul(damage, 2, 1);
    }
    // × rand 85..=100 / 100
    debug_assert!((85..=100).contains(&ctx.rand));
    damage = mul(damage, u64::from(ctx.rand), 100);
    // × stab 1.5
    if ctx.attacker.types.contains(&move_type) {
        damage = mul(damage, 3, 2);
    }
    // × type1 × type2
    damage = mul(damage, u64::from(type_product.0), u64::from(type_product.1));
    // × burn (physical only)
    if matches!(ctx.attacker.status, Some(MajorStatus::Burn))
        && matches!(spec.category, MoveCategory::Physical)
    {
        damage = mul(damage, 1, 2);
    }
    // × other: ability modifiers (doc 02 §10), one rational at a time.
    match ctx.attacker.ability {
        Ability::Amplify if spec.flags.sound => damage = mul(damage, 13, 10),
        Ability::Soloist => {
            damage = if ctx.doubles {
                mul(damage, 9, 10)
            } else {
                mul(damage, 13, 10)
            };
        }
        Ability::Chorister if ctx.doubles => damage = mul(damage, 6, 5),
        Ability::TuningFork if spec.r#type == Type::Feral => damage = mul(damage, 6, 5),
        _ => {}
    }
    let crescendo_type = match ctx.attacker.ability {
        Ability::CrescendoEmber => Some(Type::Ember),
        Ability::CrescendoTide => Some(Type::Tide),
        Ability::CrescendoBloom => Some(Type::Bloom),
        _ => None,
    };
    if let Some(element) = crescendo_type
        && move_type == element
        && u32::from(ctx.attacker.hp) * 3 <= u32::from(ctx.attacker.max_hp())
    {
        damage = mul(damage, 3, 2);
    }

    // minimum 1 if the type product is > 0
    if type_product.0 > 0 {
        damage = damage.max(1);
    } else {
        damage = 0;
    }

    Some(DamageOutcome {
        amount: u32::try_from(damage).expect("damage fits u32"),
        effectiveness,
        type_product,
    })
}

/// Weather damage factor for a move type (doc 02 §7).
fn weather_factor(weather: WeatherKind, move_type: Type) -> (u64, u64) {
    match (weather, move_type) {
        (WeatherKind::Heatwave, Type::Ember) | (WeatherKind::Downpour, Type::Tide) => (3, 2),
        (WeatherKind::Heatwave, Type::Tide) | (WeatherKind::Downpour, Type::Ember) => (1, 2),
        _ => (1, 1),
    }
}

/// Collapses an exact product to the event-stream classification.
fn classify(product: (u32, u32)) -> Eff {
    let (num, den) = product;
    if num == 0 {
        Eff::Zero
    } else if num > den {
        Eff::Double
    } else if num < den {
        Eff::Half
    } else {
        Eff::Neutral
    }
}

/// Crit chance by stage (doc 02 §4): 1/16, 1/8, 1/4, 1/3, 1/2; stage caps
/// at 4 (doc 02 v1.1 #10).
pub fn crit_chance(stage: u8) -> (u32, u32) {
    match stage.min(4) {
        0 => (1, 16),
        1 => (1, 8),
        2 => (1, 4),
        3 => (1, 3),
        _ => (1, 2),
    }
}

#[cfg(test)]
mod tests {
    use undersong_core::ids::SpeciesId;
    use undersong_core::moves::{MoveFlags, MoveTarget};
    use undersong_core::species::{GrowthCurve, StatSpread};

    use super::*;
    use crate::mote::{BattleMote, ZERO_SPREAD};

    fn chart() -> TypeChart {
        ron::from_str(
            "TypeChart(entries: {
                Ember: { Bloom: Double, Tide: Half, Feral: Neutral, Frost: Double, Alloy: Double, Ember: Half },
                Feral: { Feral: Neutral, Phantom: Zero, Ember: Neutral, Bloom: Neutral },
                Tide:  { Ember: Double, Bloom: Half, Feral: Neutral },
            })",
        )
        .expect("chart")
    }

    fn dummy(types: &[Type], atk: u16, def: u16, spa: u16, spd: u16, level: u8) -> BattleMote {
        let stats = StatSpread {
            hp: 100,
            atk,
            def,
            spa,
            spd,
            spe: 50,
        };
        BattleMote {
            species: SpeciesId::new("dummy"),
            name_key: "motif.dummy".into(),
            types: types.to_vec(),
            level,
            exp: 0,
            growth: GrowthCurve::MediumFast,
            nature: 0,
            base_stats: stats,
            ivs: ZERO_SPREAD,
            evs: ZERO_SPREAD,
            stats,
            hp: 100,
            status: None,
            moves: vec![],
            catch_rate: 100,
            base_exp_yield: 100,
            ev_yield: vec![],
            learnset: vec![],
            ability: crate::abilities::Ability::None,
            held: crate::abilities::HeldItem::None,
            entry_boosted: false,
        }
    }

    fn ember_note() -> MoveSpec {
        MoveSpec {
            id: "ember_note".into(),
            name_key: "move.ember_note".into(),
            r#type: Type::Ember,
            category: MoveCategory::Special,
            power: 40,
            accuracy: 100,
            pp: 25,
            priority: 0,
            target: MoveTarget::Foe,
            flags: MoveFlags::default(),
            effects: vec![],
        }
    }

    fn ctx<'a>(
        attacker: &'a BattleMote,
        defender: &'a BattleMote,
        stages: &'a (Stages, Stages),
        chart: &'a TypeChart,
        crit: bool,
        rand: u8,
    ) -> DamageContext<'a> {
        DamageContext {
            attacker,
            defender,
            attacker_stages: &stages.0,
            defender_stages: &stages.1,
            chart,
            weather: None,
            crit,
            rand,
            spread: false,
            doubles: false,
        }
    }

    /// Hand-computed pipeline vector, worked floor-by-floor:
    /// L50 ember_note (40 power, special) from spa 120 vs spd 80, STAB,
    /// 2× vs Bloom, max roll:
    ///   level_term = floor(100/5)+2 = 22
    ///   base = floor(22·40·120/80 / 50)+2 = floor(1320/50)+2 = 26+2 = 28
    ///   rand 100 → 28 ; STAB → floor(28·3/2)=42 ; type 2× → 84
    #[test]
    fn pipeline_hand_vector_stab_super_effective() {
        let attacker = dummy(&[Type::Ember], 50, 50, 120, 50, 50);
        let defender = dummy(&[Type::Bloom], 50, 50, 50, 80, 50);
        let chart = chart();
        let stages = (Stages::default(), Stages::default());
        let out = compute_damage(
            &ember_note(),
            &ctx(&attacker, &defender, &stages, &chart, false, 100),
        )
        .expect("damaging");
        assert_eq!(out.amount, 84);
        assert_eq!(out.effectiveness, Eff::Double);
    }

    /// Same strike at minimum roll: rand 85 → floor(28·85/100)=23;
    /// STAB → floor(23·3/2)=34 ; 2× → 68.
    #[test]
    fn pipeline_hand_vector_min_roll() {
        let attacker = dummy(&[Type::Ember], 50, 50, 120, 50, 50);
        let defender = dummy(&[Type::Bloom], 50, 50, 50, 80, 50);
        let chart = chart();
        let stages = (Stages::default(), Stages::default());
        let out = compute_damage(
            &ember_note(),
            &ctx(&attacker, &defender, &stages, &chart, false, 85),
        )
        .expect("damaging");
        assert_eq!(out.amount, 68);
    }

    /// Crit doubles before rand (doc 02 §4 order): 28 → 56 (crit) →
    /// floor(56·85/100)=47 (rand) → 70 (STAB) → 140 (2×). The reversed
    /// order would give 28 → 23 (rand) → 46 (crit) → 69 → 138, so
    /// asserting exactly 140 pins the law's floor order.
    #[test]
    fn pipeline_crit_applies_before_rand() {
        let attacker = dummy(&[Type::Ember], 50, 50, 120, 50, 50);
        let defender = dummy(&[Type::Bloom], 50, 50, 50, 80, 50);
        let chart = chart();
        let stages = (Stages::default(), Stages::default());
        let out = compute_damage(
            &ember_note(),
            &ctx(&attacker, &defender, &stages, &chart, true, 85),
        )
        .expect("damaging");
        assert_eq!(out.amount, 140);
    }

    #[test]
    fn immunity_yields_zero() {
        let attacker = dummy(&[Type::Feral], 100, 50, 50, 50, 50);
        let defender = dummy(&[Type::Phantom], 50, 50, 50, 50, 50);
        let chart = chart();
        let stages = (Stages::default(), Stages::default());
        let tackle = MoveSpec {
            id: "tackle".into(),
            name_key: "move.tackle".into(),
            r#type: Type::Feral,
            category: MoveCategory::Physical,
            power: 40,
            accuracy: 100,
            pp: 35,
            priority: 0,
            target: MoveTarget::Foe,
            flags: MoveFlags::default(),
            effects: vec![],
        };
        let out = compute_damage(
            &tackle,
            &ctx(&attacker, &defender, &stages, &chart, false, 100),
        )
        .expect("damaging move");
        assert_eq!(out.amount, 0);
        assert_eq!(out.effectiveness, Eff::Zero);
    }

    #[test]
    fn minimum_one_damage_when_not_immune() {
        // Feeble attacker vs tank: every multiplier floors to 0, but the
        // pipeline guarantees ≥ 1 when the type product > 0.
        let attacker = dummy(&[Type::Feral], 5, 50, 5, 50, 1);
        let mut defender = dummy(&[Type::Feral], 50, 999, 50, 999, 100);
        defender.stats.def = 999;
        let chart = chart();
        let stages = (Stages::default(), Stages::default());
        let tackle = MoveSpec {
            id: "tackle".into(),
            name_key: "move.tackle".into(),
            r#type: Type::Feral,
            category: MoveCategory::Physical,
            power: 40,
            accuracy: 100,
            pp: 35,
            priority: 0,
            target: MoveTarget::Foe,
            flags: MoveFlags::default(),
            effects: vec![],
        };
        let out = compute_damage(
            &tackle,
            &ctx(&attacker, &defender, &stages, &chart, false, 85),
        )
        .expect("damaging");
        assert_eq!(out.amount, 1);
    }

    #[test]
    fn burn_halves_physical_not_special() {
        let mut attacker = dummy(&[Type::Feral], 100, 50, 100, 50, 50);
        attacker.status = Some(MajorStatus::Burn);
        let defender = dummy(&[Type::Feral], 50, 80, 50, 80, 50);
        let chart = chart();
        let stages = (Stages::default(), Stages::default());

        let physical = MoveSpec {
            id: "tackle".into(),
            name_key: "move.tackle".into(),
            r#type: Type::Feral,
            category: MoveCategory::Physical,
            power: 40,
            accuracy: 100,
            pp: 35,
            priority: 0,
            target: MoveTarget::Foe,
            flags: MoveFlags::default(),
            effects: vec![],
        };
        let special = MoveSpec {
            category: MoveCategory::Special,
            ..physical.clone()
        };

        let burned = compute_damage(
            &physical,
            &ctx(&attacker, &defender, &stages, &chart, false, 100),
        )
        .expect("damaging")
        .amount;
        attacker.status = None;
        let unburned = compute_damage(
            &physical,
            &ctx(&attacker, &defender, &stages, &chart, false, 100),
        )
        .expect("damaging")
        .amount;
        // Hand check: term 22·40·150/80? — atk 100 vs def 80:
        // base = floor(22·40·100/80/50)+2 = floor(1100/50)+2 = 24
        // STAB → 36 ; burn → 18.
        assert_eq!(unburned, 36);
        assert_eq!(burned, 18);

        attacker.status = Some(MajorStatus::Burn);
        let special_burned = compute_damage(
            &special,
            &ctx(&attacker, &defender, &stages, &chart, false, 100),
        )
        .expect("damaging")
        .amount;
        assert_eq!(special_burned, 36, "burn must not affect special moves");
    }

    #[test]
    fn crit_ignores_helpful_stages_only() {
        let attacker = dummy(&[Type::Ember], 50, 50, 120, 50, 50);
        let defender = dummy(&[Type::Bloom], 50, 50, 50, 80, 50);
        let chart = chart();

        // Attacker at −2 spa, defender at +2 spd: a crit ignores both.
        let mut stages = (Stages::default(), Stages::default());
        stages.0.bump(StageStat::Spa, -2);
        stages.1.bump(StageStat::Spd, 2);

        let crit = compute_damage(
            &ember_note(),
            &ctx(&attacker, &defender, &stages, &chart, true, 100),
        )
        .expect("damaging")
        .amount;
        let neutral_stages = (Stages::default(), Stages::default());
        let baseline = compute_damage(
            &ember_note(),
            &ctx(&attacker, &defender, &neutral_stages, &chart, true, 100),
        )
        .expect("damaging")
        .amount;
        assert_eq!(
            crit, baseline,
            "crit cancels unfavorable-to-attacker stages"
        );

        // But favorable stages still count: attacker +2 spa stays.
        let mut helpful = (Stages::default(), Stages::default());
        helpful.0.bump(StageStat::Spa, 2);
        let boosted = compute_damage(
            &ember_note(),
            &ctx(&attacker, &defender, &helpful, &chart, true, 100),
        )
        .expect("damaging")
        .amount;
        assert!(boosted > baseline);
    }

    #[test]
    fn weather_scales_ember_and_tide() {
        let attacker = dummy(&[Type::Ember], 50, 50, 120, 50, 50);
        let defender = dummy(&[Type::Feral], 50, 50, 50, 80, 50);
        let chart = chart();
        let stages = (Stages::default(), Stages::default());
        let mut context = ctx(&attacker, &defender, &stages, &chart, false, 100);

        let dry = compute_damage(&ember_note(), &context).expect("dmg").amount;
        context.weather = Some(WeatherKind::Heatwave);
        let boosted = compute_damage(&ember_note(), &context).expect("dmg").amount;
        context.weather = Some(WeatherKind::Downpour);
        let dampened = compute_damage(&ember_note(), &context).expect("dmg").amount;

        // base 28 → STAB 42 (neutral vs feral): dry 42, heatwave floor(28·3/2)=42→63, downpour floor(28/2)=14→21
        assert_eq!(dry, 42);
        assert_eq!(boosted, 63);
        assert_eq!(dampened, 21);
    }

    #[test]
    fn dustchord_boosts_stone_defender_special_defense() {
        // v1.2 #9: special hits vs stone types under dustchord use
        // spd ×3/2 inside D. spa 120 vs spd 80: dry D=80 → base
        // floor(22·40·120/80/50)+2 = 28; dustchord D=120 → base
        // floor(22·40·120/120/50)+2 = floor(880/50)+2 = 19.
        let attacker = dummy(&[Type::Ember], 50, 50, 120, 50, 50);
        let defender = dummy(&[Type::Stone], 50, 50, 50, 80, 50);
        let mut chart_text = String::from("TypeChart(entries: {");
        for a in Type::ALL {
            chart_text.push_str(&format!("{a:?}: {{"));
            for d in Type::ALL {
                chart_text.push_str(&format!("{d:?}: Neutral,"));
            }
            chart_text.push_str("},");
        }
        chart_text.push_str("})");
        let chart: TypeChart = ron::from_str(&chart_text).expect("chart");
        let stages = (Stages::default(), Stages::default());
        let mut context = ctx(&attacker, &defender, &stages, &chart, false, 100);

        let dry = compute_damage(&ember_note(), &context).expect("dmg").amount;
        context.weather = Some(WeatherKind::Dustchord);
        let walled = compute_damage(&ember_note(), &context).expect("dmg").amount;
        // STAB applies to both (Ember attacker): 28→42 dry, 19→28 walled.
        assert_eq!(dry, 42);
        assert_eq!(walled, 28);

        // Physical hits are unaffected by dustchord.
        let physical = MoveSpec {
            id: "shove".into(),
            name_key: "move.shove".into(),
            r#type: Type::Feral,
            category: MoveCategory::Physical,
            power: 40,
            accuracy: 100,
            pp: 30,
            priority: 0,
            target: MoveTarget::Foe,
            flags: MoveFlags::default(),
            effects: vec![],
        };
        context.weather = None;
        let dry_physical = compute_damage(&physical, &context).expect("dmg").amount;
        context.weather = Some(WeatherKind::Dustchord);
        let dust_physical = compute_damage(&physical, &context).expect("dmg").amount;
        assert_eq!(dry_physical, dust_physical);
    }

    #[test]
    fn crit_chance_table_matches_doc() {
        assert_eq!(crit_chance(0), (1, 16));
        assert_eq!(crit_chance(1), (1, 8));
        assert_eq!(crit_chance(2), (1, 4));
        assert_eq!(crit_chance(3), (1, 3));
        assert_eq!(crit_chance(4), (1, 2));
        assert_eq!(crit_chance(9), (1, 2), "stage caps at 4");
    }
}
