//! Dialogue & cutscene command interpreter (docs/03-ARCHITECTURE.md §5).
//!
//! A tiny deterministic interpreter, not a scripting language:
//! `(state, cmd) -> (state, [SideEffectReq])`, pure and unit-testable.
//! The game crate executes side-effect requests and resumes the runner;
//! the Act-2 Roster scene is meant to be literally a unit test on its
//! flag outcomes.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use undersong_core::ids::{ItemId, MapId, SpeciesId, TrainerId};

/// Script commands (doc 03 §5). Content references these from
/// `*.script.ron` files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Cmd {
    Say {
        who: String,
        key: String,
    },
    /// Presents choices (string keys); each branch is a sub-script.
    Choice {
        key: String,
        branches: Vec<(String, Vec<Cmd>)>,
    },
    Move {
        npc: String,
        path: Vec<Direction>,
    },
    Face {
        npc: String,
        dir: Direction,
    },
    Wait {
        ms: u32,
    },
    SetFlag {
        flag: String,
    },
    ClearFlag {
        flag: String,
    },
    If {
        flag: String,
        then: Vec<Cmd>,
        #[serde(default)]
        r#else: Vec<Cmd>,
    },
    GiveItem {
        id: ItemId,
        n: u32,
    },
    GiveMote {
        species: SpeciesId,
        level: u8,
    },
    StartBattle {
        trainer: TrainerId,
    },
    Warp {
        map: MapId,
        x: u32,
        y: u32,
    },
    PlayCry {
        species: SpeciesId,
    },
    Music {
        track: String,
        fade_ms: u32,
    },
    ShakeScreen,
    OpenShop {
        table: String,
    },
    HealParty,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// What the interpreter asks the engine to do. The engine performs the
/// request (rendering text, moving sprites, fighting battles…) and calls
/// [`ScriptRunner::resume`] when done.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SideEffectReq {
    ShowText {
        who: String,
        key: String,
    },
    /// Engine must answer with the chosen branch index via `resume_choice`.
    AskChoice {
        key: String,
        options: Vec<String>,
    },
    MoveNpc {
        npc: String,
        path: Vec<Direction>,
    },
    FaceNpc {
        npc: String,
        dir: Direction,
    },
    Wait {
        ms: u32,
    },
    GiveItem {
        id: ItemId,
        n: u32,
    },
    GiveMote {
        species: SpeciesId,
        level: u8,
    },
    StartBattle {
        trainer: TrainerId,
    },
    Warp {
        map: MapId,
        x: u32,
        y: u32,
    },
    PlayCry {
        species: SpeciesId,
    },
    Music {
        track: String,
        fade_ms: u32,
    },
    ShakeScreen,
    OpenShop {
        table: String,
    },
    HealParty,
}

/// Persistent world state the interpreter reads and writes. The save
/// system owns the canonical copy; scripts mutate it through here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptVars {
    pub flags: BTreeSet<String>,
    pub vars: BTreeMap<String, i32>,
}

/// Execution state of one running script: a stack of (command list,
/// program counter) frames, innermost last — `If`/`Choice` push frames.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptRunner {
    frames: Vec<(Vec<Cmd>, usize)>,
    /// Set while waiting for the engine to answer an `AskChoice`.
    pending_choice: Option<Vec<(String, Vec<Cmd>)>>,
    finished: bool,
}

/// One step's outcome: possibly a side effect to perform, and whether
/// the script is done.
#[derive(Debug, Clone, PartialEq)]
pub enum StepResult {
    /// Perform this request, then call `resume` (or `resume_choice` for
    /// `AskChoice`).
    Effect(SideEffectReq),
    /// Script completed.
    Done,
}

impl ScriptRunner {
    pub fn new(script: Vec<Cmd>) -> Self {
        Self {
            frames: vec![(script, 0)],
            pending_choice: None,
            finished: false,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Runs until the next side effect or completion. Pure: all state is
    /// in `self` and `vars`.
    pub fn step(&mut self, vars: &mut ScriptVars) -> StepResult {
        assert!(
            self.pending_choice.is_none(),
            "resume_choice must answer a pending AskChoice before stepping"
        );
        loop {
            let Some((cmds, pc)) = self.frames.last_mut() else {
                self.finished = true;
                return StepResult::Done;
            };
            let Some(cmd) = cmds.get(*pc).cloned() else {
                self.frames.pop();
                continue;
            };
            *pc += 1;

            match cmd {
                Cmd::Say { who, key } => {
                    return StepResult::Effect(SideEffectReq::ShowText { who, key });
                }
                Cmd::Choice { key, branches } => {
                    let options = branches.iter().map(|(k, _)| k.clone()).collect();
                    self.pending_choice = Some(branches);
                    return StepResult::Effect(SideEffectReq::AskChoice { key, options });
                }
                Cmd::Move { npc, path } => {
                    return StepResult::Effect(SideEffectReq::MoveNpc { npc, path });
                }
                Cmd::Face { npc, dir } => {
                    return StepResult::Effect(SideEffectReq::FaceNpc { npc, dir });
                }
                Cmd::Wait { ms } => return StepResult::Effect(SideEffectReq::Wait { ms }),
                Cmd::SetFlag { flag } => {
                    vars.flags.insert(flag);
                }
                Cmd::ClearFlag { flag } => {
                    vars.flags.remove(&flag);
                }
                Cmd::If { flag, then, r#else } => {
                    let branch = if vars.flags.contains(&flag) {
                        then
                    } else {
                        r#else
                    };
                    if !branch.is_empty() {
                        self.frames.push((branch, 0));
                    }
                }
                Cmd::GiveItem { id, n } => {
                    return StepResult::Effect(SideEffectReq::GiveItem { id, n });
                }
                Cmd::GiveMote { species, level } => {
                    return StepResult::Effect(SideEffectReq::GiveMote { species, level });
                }
                Cmd::StartBattle { trainer } => {
                    return StepResult::Effect(SideEffectReq::StartBattle { trainer });
                }
                Cmd::Warp { map, x, y } => {
                    return StepResult::Effect(SideEffectReq::Warp { map, x, y });
                }
                Cmd::PlayCry { species } => {
                    return StepResult::Effect(SideEffectReq::PlayCry { species });
                }
                Cmd::Music { track, fade_ms } => {
                    return StepResult::Effect(SideEffectReq::Music { track, fade_ms });
                }
                Cmd::ShakeScreen => return StepResult::Effect(SideEffectReq::ShakeScreen),
                Cmd::OpenShop { table } => {
                    return StepResult::Effect(SideEffectReq::OpenShop { table });
                }
                Cmd::HealParty => return StepResult::Effect(SideEffectReq::HealParty),
                Cmd::End => {
                    self.frames.clear();
                    self.finished = true;
                    return StepResult::Done;
                }
            }
        }
    }

    /// Answers a pending `AskChoice` by branch index (clamped to the last
    /// option, deterministically, if the engine passes junk).
    pub fn resume_choice(&mut self, index: usize) {
        let branches = self
            .pending_choice
            .take()
            .expect("resume_choice without a pending AskChoice");
        let clamped = index.min(branches.len().saturating_sub(1));
        if let Some((_, branch)) = branches.into_iter().nth(clamped)
            && !branch.is_empty()
        {
            self.frames.push((branch, 0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_to_end(runner: &mut ScriptRunner, vars: &mut ScriptVars) -> Vec<SideEffectReq> {
        let mut effects = Vec::new();
        loop {
            match runner.step(vars) {
                StepResult::Effect(SideEffectReq::AskChoice { .. }) => {
                    panic!("test script must not ask choices in run_to_end")
                }
                StepResult::Effect(effect) => effects.push(effect),
                StepResult::Done => return effects,
            }
        }
    }

    #[test]
    fn linear_script_emits_effects_in_order_and_sets_flags() {
        let script = vec![
            Cmd::Say {
                who: "fisher_old".into(),
                key: "quietcoast.fisher.1".into(),
            },
            Cmd::SetFlag {
                flag: "met.fisher_old".into(),
            },
            Cmd::Say {
                who: "fisher_old".into(),
                key: "quietcoast.fisher.2".into(),
            },
            Cmd::End,
        ];
        let mut vars = ScriptVars::default();
        let mut runner = ScriptRunner::new(script);
        let effects = run_to_end(&mut runner, &mut vars);
        assert_eq!(effects.len(), 2);
        assert!(
            matches!(&effects[0], SideEffectReq::ShowText { key, .. } if key == "quietcoast.fisher.1")
        );
        assert!(vars.flags.contains("met.fisher_old"));
        assert!(runner.is_finished());
    }

    #[test]
    fn if_branches_on_flags_and_recontext_pairs_work() {
        // The doc 04 §2 fisher script shape: a recontext line only after
        // the Act-2 reveal flag.
        let script = vec![Cmd::If {
            flag: "story.act2.read_roster".into(),
            then: vec![Cmd::Say {
                who: "fisher_old".into(),
                key: "quietcoast.fisher.recontext".into(),
            }],
            r#else: vec![Cmd::Say {
                who: "fisher_old".into(),
                key: "quietcoast.fisher.2".into(),
            }],
        }];

        let mut fresh = ScriptVars::default();
        let effects = run_to_end(&mut ScriptRunner::new(script.clone()), &mut fresh);
        assert!(
            matches!(&effects[0], SideEffectReq::ShowText { key, .. } if key == "quietcoast.fisher.2")
        );

        let mut revealed = ScriptVars::default();
        revealed.flags.insert("story.act2.read_roster".into());
        let effects = run_to_end(&mut ScriptRunner::new(script), &mut revealed);
        assert!(
            matches!(&effects[0], SideEffectReq::ShowText { key, .. } if key == "quietcoast.fisher.recontext")
        );
    }

    #[test]
    fn choice_pauses_until_answered_and_runs_the_branch() {
        let script = vec![
            Cmd::Choice {
                key: "reed.starter".into(),
                branches: vec![
                    (
                        "choice.yes".into(),
                        vec![Cmd::SetFlag {
                            flag: "starter.accepted".into(),
                        }],
                    ),
                    (
                        "choice.no".into(),
                        vec![Cmd::SetFlag {
                            flag: "starter.refused".into(),
                        }],
                    ),
                ],
            },
            Cmd::End,
        ];
        let mut vars = ScriptVars::default();
        let mut runner = ScriptRunner::new(script);
        let StepResult::Effect(SideEffectReq::AskChoice { options, .. }) = runner.step(&mut vars)
        else {
            panic!("expected AskChoice");
        };
        assert_eq!(
            options,
            vec!["choice.yes".to_string(), "choice.no".to_string()]
        );
        runner.resume_choice(1);
        while !matches!(runner.step(&mut vars), StepResult::Done) {}
        assert!(vars.flags.contains("starter.refused"));
        assert!(!vars.flags.contains("starter.accepted"));
    }

    #[test]
    fn end_terminates_nested_frames_immediately() {
        let script = vec![
            Cmd::If {
                flag: "x".into(),
                then: vec![],
                r#else: vec![
                    Cmd::End,
                    Cmd::SetFlag {
                        flag: "unreachable".into(),
                    },
                ],
            },
            Cmd::SetFlag {
                flag: "also_unreachable".into(),
            },
        ];
        let mut vars = ScriptVars::default();
        let mut runner = ScriptRunner::new(script);
        while !matches!(runner.step(&mut vars), StepResult::Done) {}
        assert!(vars.flags.is_empty());
    }

    #[test]
    fn scripts_parse_from_ron_content_shape() {
        // Doc 04 §2's dialogue-script example, verbatim shape.
        let text = r#"[
            Say(who: "fisher_old", key: "quietcoast.fisher.1"),
            If(flag: "story.act2.read_roster",
               then: [Say(who: "fisher_old", key: "quietcoast.fisher.recontext")],
               else: []),
            SetFlag(flag: "met.fisher_old"),
            End,
        ]"#;
        let script: Vec<Cmd> = ron::from_str(text).expect("parse");
        assert_eq!(script.len(), 4);
        let mut vars = ScriptVars::default();
        let mut runner = ScriptRunner::new(script);
        let effects = {
            let mut effects = Vec::new();
            while let StepResult::Effect(e) = runner.step(&mut vars) {
                effects.push(e);
            }
            effects
        };
        assert_eq!(effects.len(), 1);
        assert!(vars.flags.contains("met.fisher_old"));
    }

    #[test]
    fn runner_state_serializes_for_save_mid_script() {
        let script = vec![
            Cmd::Say {
                who: "a".into(),
                key: "k1".into(),
            },
            Cmd::Say {
                who: "a".into(),
                key: "k2".into(),
            },
        ];
        let mut vars = ScriptVars::default();
        let mut runner = ScriptRunner::new(script);
        let _ = runner.step(&mut vars);
        let text = ron::to_string(&runner).expect("serialize");
        let mut restored: ScriptRunner = ron::from_str(&text).expect("deserialize");
        let StepResult::Effect(SideEffectReq::ShowText { key, .. }) = restored.step(&mut vars)
        else {
            panic!("expected second line after restore");
        };
        assert_eq!(key, "k2");
    }
}
