//! The launch-24 abilities (doc 02 §10), as a closed enum the engine
//! hooks against. Content names map here at resolve time; unknown ids
//! become `None` (a content error the validator catches separately).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Ability {
    #[default]
    None,
    CrescendoEmber,
    CrescendoTide,
    CrescendoBloom,
    /// On entry: foes' atk −1.
    Dissonance,
    /// Sound moves ×1.3.
    Amplify,
    /// Immune to sound moves.
    Damper,
    /// Immune to stone moves.
    Floating,
    /// 30% paralysis on being contacted.
    LiveWire,
    /// 1/8 recoil to contacters.
    ThornCoat,
    HeatHaze,
    RainCaller,
    FlurryCaller,
    DustCaller,
    /// Accuracy can't drop; ignores target evasion stages.
    PerfectPitch,
    /// +1 spe on first entry each battle.
    StageFright,
    /// Cannot be struck critically.
    ThickHide,
    /// Speed can't be lowered (incl. the paralysis penalty).
    MetronomeSoul,
    /// +1 atk/spa when an ally faints (doubles).
    Understudy,
    /// Restores 1/16 HP each turn in any weather.
    EncoreHeart,
    /// Foe flinch chance +10% on sound moves; catch-assist flag.
    Keysmith,
    /// Immune to sleep.
    Vigor,
    /// Immune to flinch.
    IronEar,
    /// ×1.3 damage in singles, ×0.9 in doubles.
    Soloist,
    /// ×1.2 damage in doubles.
    Chorister,
    /// Normal-effect feral moves become resonant type, ×1.2.
    TuningFork,
}

impl Ability {
    pub fn from_id(id: &str) -> Ability {
        match id {
            "crescendo_ember" => Ability::CrescendoEmber,
            "crescendo_tide" => Ability::CrescendoTide,
            "crescendo_bloom" => Ability::CrescendoBloom,
            "dissonance" => Ability::Dissonance,
            "amplify" => Ability::Amplify,
            "damper" => Ability::Damper,
            "floating" => Ability::Floating,
            "live_wire" => Ability::LiveWire,
            "thorn_coat" => Ability::ThornCoat,
            "heat_haze" => Ability::HeatHaze,
            "rain_caller" => Ability::RainCaller,
            "flurry_caller" => Ability::FlurryCaller,
            "dust_caller" => Ability::DustCaller,
            "perfect_pitch" => Ability::PerfectPitch,
            "stage_fright" => Ability::StageFright,
            "thick_hide" => Ability::ThickHide,
            "metronome_soul" => Ability::MetronomeSoul,
            "understudy" => Ability::Understudy,
            "encore_heart" => Ability::EncoreHeart,
            "keysmith" => Ability::Keysmith,
            "vigor" => Ability::Vigor,
            "iron_ear" => Ability::IronEar,
            "soloist" => Ability::Soloist,
            "chorister" => Ability::Chorister,
            "tuning_fork" => Ability::TuningFork,
            _ => Ability::None,
        }
    }

    /// The weather an on-entry caller sets (doc 02 v1.5 #4).
    pub fn called_weather(self) -> Option<undersong_core::moves::WeatherKind> {
        use undersong_core::moves::WeatherKind;
        match self {
            Ability::HeatHaze => Some(WeatherKind::Heatwave),
            Ability::RainCaller => Some(WeatherKind::Downpour),
            Ability::FlurryCaller => Some(WeatherKind::Flurry),
            Ability::DustCaller => Some(WeatherKind::Dustchord),
            _ => None,
        }
    }
}

/// Held items with battle hooks (doc 02 v1.5 #1 launch set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HeldItem {
    #[default]
    None,
    /// Once per battle: ≤ 1/2 max HP → restore 20 HP.
    OranChime { used: bool },
    /// §9: non-participants holding it gain 50% exp unsplit.
    ExpShare,
}

impl HeldItem {
    pub fn from_id(id: &str) -> HeldItem {
        match id {
            "oran_chime" => HeldItem::OranChime { used: false },
            "exp_share" => HeldItem::ExpShare,
            _ => HeldItem::None,
        }
    }
}
