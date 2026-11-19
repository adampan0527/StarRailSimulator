use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU32, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::workbench::{AnalysisRequest, LoadoutEdit, TrainingAnalysis, Workbench};
use star_rail_battle_core::{Command, Outcome};

const SCRIPT: &str = include_str!("../../crates/battle-core/fixtures/full-replay.commands.json");
static NEXT_TEMP: AtomicU32 = AtomicU32::new(0);

#[test]
fn training_analysis_uses_frozen_inputs_and_produces_replayable_commands() {
    let dir = TestData::new();
    let mut app = Workbench::open(dir.path()).unwrap();
    let left = [app.setup().unwrap().enemies[0].id.clone()];
    let request = || AnalysisRequest::FastestVictory {
        enemy_ids: left.to_vec(),
    };
    let frozen = app.analysis_input(request()).unwrap();
    app.save_team(Default::default()).unwrap();
    assert!(app.analysis_input(request()).is_err());
    let TrainingAnalysis::FastestVictory { result, .. } = frozen.run().unwrap() else {
        panic!("expected fastest victory");
    };
    assert!(result.exact);
    assert!(app.replays().unwrap().is_empty());
    let solution = result.victory.unwrap();

    let template = app.setup().unwrap();
    app.save_team(std::array::from_fn(|index| {
        Some(template.profiles[index].id.clone())
    }))
    .unwrap();
    let mut battle = app.start(&left).unwrap();
    for action in solution.commands {
        battle = app.act(battle.id, &action).unwrap();
    }
    assert_eq!(battle.snapshot.outcome, Outcome::Victory);
    assert_eq!(battle.snapshot.av_micros, solution.av_micros);
}

#[test]
fn training_dpr_uses_requested_dummies_and_saved_character_names() {
    let dir = TestData::new();
    let app = Workbench::open(dir.path()).unwrap();
    let request = serde_json::from_value(serde_json::json!({
        "kind": "dpr", "toughness": [30, 120, 30]
    }))
    .unwrap();
    let TrainingAnalysis::Dpr { result, names } =
        app.analysis_input(request).unwrap().run().unwrap()
    else {
        panic!("expected DPR")
    };
    assert_eq!(result.cycles, 3);
    assert_eq!(
        result
            .targets
            .iter()
            .map(|target| target.max_toughness)
            .collect::<Vec<_>>(),
        [30, 120, 30]
    );
    assert_eq!(names.len(), result.characters.len());
    assert_eq!(names.len(), 4);
    assert!(
        names
            .iter()
            .any(|unit| unit.id == "ally.3" && unit.name == "丹恒 · 巡猎")
    );
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|entry| entry.own_damage + entry.assist_damage)
            .sum::<u64>()
    );
    assert!(app.replays().unwrap().is_empty());
}

pub(crate) struct TestData(PathBuf);

impl TestData {
    pub(crate) fn new() -> Self {
        Self::in_directory(&std::env::temp_dir())
    }

    pub(crate) fn in_directory(parent: &Path) -> Self {
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let dir = parent.join(format!(
            "star-rail-m04-{}-{}-{sequence}",
            std::process::id(),
            time.as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestData {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_completed_session_keeps_frozen_equipment_and_is_available_after_reopen() {
    let dir = TestData::new();
    let mut app = Workbench::open(dir.path()).unwrap();
    let setup = app.setup().unwrap();
    let enemy_ids = setup
        .enemies
        .into_iter()
        .map(|enemy| enemy.id)
        .collect::<Vec<_>>();
    let original = app.start(&enemy_ids).unwrap();
    let commands: Vec<Command> = serde_json::from_str(SCRIPT).unwrap();

    let trailblazer = &setup.profiles[0].id;
    let new_profile = app
        .save_loadout(
            trailblazer,
            "速度训练",
            LoadoutEdit {
                light_cone_equipped: true,
                boots_speed_micros: Some(2_600_000),
            },
        )
        .unwrap();
    assert_eq!(new_profile.stats.speed.map(|speed| speed.total), Some(92.6));
    let edited = app.start(&enemy_ids).unwrap();
    let mut original_step = original;
    let mut edited_step = edited;
    for (index, command) in commands.iter().enumerate() {
        assert!(original_step.legal_actions.contains(command));
        original_step = app.act(original_step.id, command).unwrap();
        edited_step = app.act(edited_step.id, command).unwrap();
        if index == 1 {
            assert_eq!(original_step.snapshot.av_micros, 108_342_362);
            assert_eq!(edited_step.snapshot.av_micros, 107_991_361);
        }
    }
    assert_eq!(original_step.snapshot.outcome, Outcome::Victory);
    assert_eq!(edited_step.snapshot.outcome, Outcome::Victory);
    drop(app);

    let reopened = Workbench::open(dir.path()).unwrap();
    assert_eq!(reopened.replays().unwrap().len(), 2);
    let old_replay = reopened.replay(original_step.id).unwrap();
    assert_eq!(old_replay.snapshot, original_step.snapshot);
    assert_eq!(old_replay.events, original_step.events);
    assert_eq!(old_replay.events.len(), 64);
    assert_eq!(
        reopened.setup().unwrap().profiles[0].current_preset,
        "速度训练"
    );
}

#[test]
fn invalid_drafts_and_stale_commands_leave_the_persisted_state_intact() {
    let dir = TestData::new();
    let mut app = Workbench::open(dir.path()).unwrap();
    let setup = app.setup().unwrap();
    let trailblazer = &setup.profiles[0].id;
    assert!(
        app.save_loadout(
            trailblazer,
            "无效",
            LoadoutEdit {
                light_cone_equipped: true,
                boots_speed_micros: Some(2_300_001),
            }
        )
        .is_err()
    );
    assert_eq!(
        app.setup().unwrap().profiles[0].current_preset,
        setup.profiles[0].current_preset
    );
    assert!(app.start(&[]).is_err());
    assert!(app.replays().unwrap().is_empty());

    let battle = app.start(&[setup.enemies[0].id.clone()]).unwrap();
    assert_eq!(battle.snapshot.units.len(), 5);
    let command = battle.legal_actions[0].clone();
    let storage = rusqlite::Connection::open(dir.path().join("training-replays.sqlite")).unwrap();
    storage
        .execute_batch(
            "CREATE TRIGGER fail_replay_write BEFORE UPDATE ON replays
             BEGIN SELECT RAISE(ABORT, 'test write failure'); END;",
        )
        .unwrap();
    assert!(app.act(battle.id, &command).is_err());
    let unchanged = app.replay(battle.id).unwrap();
    assert_eq!(unchanged.snapshot, battle.snapshot);
    assert_eq!(unchanged.events, battle.events);
    storage
        .execute_batch("DROP TRIGGER fail_replay_write;")
        .unwrap();
    let advanced = app.act(battle.id, &command).unwrap();
    assert!(app.act(battle.id, &command).is_err());
    drop(app);

    let reopened = Workbench::open(dir.path()).unwrap();
    let replay = reopened.replay(battle.id).unwrap();
    assert_eq!(replay.snapshot, advanced.snapshot);
    assert_eq!(replay.events, advanced.events);
}

#[test]
fn the_saved_lineup_controls_equal_av_priority_and_target_selection() {
    let dir = TestData::new();
    let mut app = Workbench::open(dir.path()).unwrap();
    let setup = app.setup().unwrap();
    let ids = &setup.team_slots;
    let slots = [None, ids[1].clone(), None, ids[2].clone()];
    app.save_team(slots.clone()).unwrap();
    let enemy = setup.enemies[1].id.clone();
    let battle = app.start(std::slice::from_ref(&enemy)).unwrap();
    assert_eq!(battle.snapshot.units.len(), 3);
    assert_eq!(battle.order[0].unit_id, "ally.3");
    assert_eq!(battle.order[1].unit_id, "ally.2");
    assert_eq!(
        battle.legal_actions[0].target_id.as_deref(),
        Some(enemy.as_str())
    );
    assert!(
        app.save_team([ids[0].clone(), ids[0].clone(), None, None])
            .is_err()
    );
    drop(app);
    let mut app = Workbench::open(dir.path()).unwrap();
    assert_eq!(app.setup().unwrap().team_slots, slots);
    app.save_team(Default::default()).unwrap();
    assert!(app.start(&[enemy]).is_err());
}
