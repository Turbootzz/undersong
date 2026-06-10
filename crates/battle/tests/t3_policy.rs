use battle::ai::{AiTier, choose};
use battle::{Action, BattleKind, BattleState, MoteBuilder};
use undersong_core::chart::TypeChart;
use undersong_core::moves::{MoveCategory, MoveFlags, MoveSpec, MoveTarget};
use undersong_core::rng::BattleRng;
use undersong_core::species::{GrowthCurve, SpeciesSpec, StatSpread};
use undersong_core::types::Type;

fn chart() -> TypeChart {
    let mut rows = String::from("TypeChart(entries: {");
    for a in Type::ALL {
        rows.push_str(&format!("{a:?}: {{"));
        for d in Type::ALL {
            let eff = match (a, d) {
                (Type::Ember, Type::Bloom) => "Double",
                (Type::Bloom, Type::Tide) => "Double",
                (Type::Tide, Type::Ember) => "Double",
                (Type::Bloom, Type::Ember) => "Half",
                (Type::Tide, Type::Bloom) => "Half",
                (Type::Ember, Type::Tide) => "Half",
                _ => "Neutral",
            };
            rows.push_str(&format!("{d:?}: {eff},"));
        }
        rows.push_str("},");
    }
    rows.push_str("})");
    ron::from_str(&rows).expect("chart")
}

fn mk(id: &str, ty: Type) -> (SpeciesSpec, MoveSpec) {
    let spec = SpeciesSpec {
        id: id.into(),
        name_key: format!("motif.{id}"),
        types: vec![ty],
        base_stats: StatSpread {
            hp: 80,
            atk: 80,
            def: 80,
            spa: 80,
            spd: 80,
            spe: 80,
        },
        catch_rate: 100,
        base_exp_yield: 60,
        ev_yield: std::collections::BTreeMap::new().into(),
        growth_curve: GrowthCurve::MediumFast,
        learnset: vec![],
        abilities: vec![],
        hidden_ability: None,
        tags: vec![],
    };
    let mv = MoveSpec {
        id: format!("{id}_hit").into(),
        name_key: "move.x".into(),
        r#type: ty,
        category: MoveCategory::Physical,
        power: 60,
        accuracy: 100,
        pp: 30,
        priority: 0,
        target: MoveTarget::Foe,
        flags: MoveFlags::default(),
        effects: vec![],
    };
    (spec, mv)
}

#[test]
fn t3_switches_out_of_a_wall() {
    let (ember_s, ember_m) = mk("emb", Type::Ember);
    let (tide_s, tide_m) = mk("tid", Type::Tide);
    let (bloom_s, bloom_m) = mk("blo", Type::Bloom);
    // Me: bloom fielded (walled by foe ember: my bloom ½ vs ember, foe 2× vs me)
    // Bench: tide (2× vs ember, takes ½). Correct: switch to tide.
    let me_bloom = MoteBuilder::new(&bloom_s, 30)
        .moves(vec![bloom_m.clone()])
        .build();
    let me_tide = MoteBuilder::new(&tide_s, 30)
        .moves(vec![tide_m.clone()])
        .build();
    let foe_ember = MoteBuilder::new(&ember_s, 30)
        .moves(vec![ember_m.clone()])
        .build();
    let state = BattleState::new(
        BattleKind::Trainer,
        vec![me_bloom, me_tide],
        vec![foe_ember],
        chart(),
    );
    let mut rng = BattleRng::from_seed(1);
    let action = choose(AiTier::T3, &state, 0, &mut rng);
    eprintln!("T3 chose: {action:?}");
    assert_eq!(action, Action::Switch { to: 1 }, "switch out of the wall");
}
