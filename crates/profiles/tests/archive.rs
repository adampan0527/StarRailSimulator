use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU32, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::Connection;
use star_rail_battle_core::{Battle, LogMode, PreparedBattle, Session};
use star_rail_profiles::{Archive, Catalog, Error, FrozenTeam, Loadout, Team, training_catalog};

const EXAMPLE: &str = include_str!("../../../contracts/battle-session.v1.example.json");
static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

struct TempDb(PathBuf);

impl TempDb {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "star-rail-m03-{}-{nanos}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> PathBuf {
        self.0.join("profiles.sqlite")
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn team(session: &Session, id: &str) -> Team {
    Team {
        id: id.into(),
        name: id.into(),
        character_form_ids: session
            .input
            .team
            .iter()
            .map(|ally| ally.character_form_id.clone())
            .collect(),
    }
}

fn seed(path: &Path, session: &Session) -> Archive {
    let mut archive = Archive::open(path, training_catalog(session)).unwrap();
    for ally in &session.input.team {
        archive.ensure_profile(&ally.character_form_id).unwrap();
        archive
            .save_preset(
                &ally.character_form_id,
                "fixture",
                &Loadout {
                    variant_id: "base".into(),
                    affinitive_eidolon: false,
                    config: ally.config.clone(),
                },
                true,
            )
            .unwrap();
    }
    archive
}

#[test]
fn both_reopened_teams_read_current_presets_and_old_fixture_still_replays() {
    let template = Session::from_json(EXAMPLE).unwrap();
    let db = TempDb::new();
    let mut archive = seed(&db.path(), &template);
    let alpha = team(&template, "alpha");
    let mut beta = team(&template, "beta");
    beta.character_form_ids.rotate_left(1);
    archive.save_team(&alpha).unwrap();
    archive.save_team(&beta).unwrap();
    let old = archive.freeze_team("alpha").unwrap();
    let encoded = serde_json::to_string(&old).unwrap();
    let id = &alpha.character_form_ids[0];
    let mut updated = old.members[0].loadout.clone();
    updated.config.light_cone.as_mut().unwrap().superimposition = 5;
    archive.save_preset(id, "updated", &updated, true).unwrap();
    drop(archive);

    let archive = Archive::open(db.path(), training_catalog(&template)).unwrap();
    assert_eq!(archive.team("alpha").unwrap(), alpha);
    assert_eq!(archive.team("beta").unwrap(), beta);
    assert_eq!(archive.profile(id).unwrap().current_preset, "updated");
    assert_eq!(archive.profile_ids().unwrap().len(), 4);
    assert_eq!(archive.team_ids().unwrap(), vec!["alpha", "beta"]);
    for team in ["alpha", "beta"] {
        let snapshot = archive.freeze_team(team).unwrap();
        assert_eq!(
            snapshot
                .members
                .iter()
                .find(|member| &member.character_form_id == id)
                .unwrap()
                .loadout,
            updated
        );
    }
    let decoded = FrozenTeam::from_json(&encoded, archive.catalog()).unwrap();
    assert_eq!(decoded, old);
    assert_eq!(
        old.members[0]
            .loadout
            .config
            .light_cone
            .as_ref()
            .unwrap()
            .superimposition,
        1
    );
    let regenerated = old.fixture_session(&template).unwrap();
    let regenerated = Session::from_json(&serde_json::to_string(&regenerated).unwrap()).unwrap();
    assert!(regenerated.commands.is_empty());
    let prepared = PreparedBattle::new(&regenerated.input).unwrap();
    let mut battle = Battle::start(&prepared, LogMode::Detailed);
    for command in &template.commands {
        battle.apply(command).unwrap();
    }
    assert_eq!(
        regenerated.input.team[0].config,
        template.input.team[0].config
    );
}

#[test]
fn rejects_invalid_drafts_and_preserves_the_active_preset() {
    let template = Session::from_json(EXAMPLE).unwrap();
    let db = TempDb::new();
    let mut archive = seed(&db.path(), &template);
    let id = &template.input.team[0].character_form_id;
    let baseline = archive.profile(id).unwrap();
    let mut draft = baseline.presets["fixture"].clone();
    draft.config.relics.head = draft.config.relics.feet.take();
    assert!(matches!(
        archive.save_preset(id, "invalid", &draft, true),
        Err(Error::Invalid {
            field: "relic.slot",
            ..
        })
    ));
    draft = baseline.presets["fixture"].clone();
    draft
        .config
        .relics
        .feet
        .as_mut()
        .unwrap()
        .substats
        .insert("speed".into(), 2_300_001);
    assert!(matches!(
        archive.save_preset(id, "invalid", &draft, true),
        Err(Error::Invalid {
            field: "relic.substats",
            ..
        })
    ));
    draft = baseline.presets["fixture"].clone();
    draft.config.ascension = 4;
    assert!(archive.save_preset(id, "invalid", &draft, true).is_err());
    draft = baseline.presets["fixture"].clone();
    draft.config.unlocked_trace_ids.push("trace.child".into());
    assert!(archive.save_preset(id, "invalid", &draft, true).is_err());
    draft = baseline.presets["fixture"].clone();
    draft.config.light_cone.as_mut().unwrap().superimposition = 6;
    assert!(archive.save_preset(id, "invalid", &draft, true).is_err());
    assert_eq!(archive.profile(id).unwrap(), baseline);
}

#[test]
fn failed_second_write_rolls_back_the_preset_and_allows_retry_after_reopen() {
    let template = Session::from_json(EXAMPLE).unwrap();
    let db = TempDb::new();
    let mut archive = seed(&db.path(), &template);
    let id = &template.input.team[0].character_form_id;
    let before = archive.profile(id).unwrap();
    let mut draft = before.presets["fixture"].clone();
    draft.config.eidolon = 3;
    let fault = Connection::open(db.path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER fail_switch BEFORE UPDATE OF current_preset ON profiles
         WHEN NEW.current_preset = 'retry' BEGIN SELECT RAISE(ABORT, 'test failure'); END;",
        )
        .unwrap();
    assert!(matches!(
        archive.save_preset(id, "retry", &draft, true),
        Err(Error::Database(_))
    ));
    drop(archive);
    let mut archive = Archive::open(db.path(), training_catalog(&template)).unwrap();
    assert_eq!(archive.profile(id).unwrap(), before);
    fault.execute_batch("DROP TRIGGER fail_switch").unwrap();
    archive.save_preset(id, "retry", &draft, true).unwrap();
    archive.rename_preset(id, "retry", "renamed").unwrap();
    assert_eq!(archive.profile(id).unwrap().presets["renamed"], draft);
    assert!(archive.delete_preset(id, "renamed").is_err());
    archive.select_preset(id, "fixture").unwrap();
    archive.delete_preset(id, "renamed").unwrap();
    assert!(!archive.profile(id).unwrap().presets.contains_key("renamed"));
}

#[test]
fn defaults_and_rank_levels_do_not_merge_or_double_count() {
    let template = Session::from_json(EXAMPLE).unwrap();
    let catalog = training_catalog(&template);
    let id = "trailblazer.destruction";
    let mut draft = catalog.default_loadout(id).unwrap();
    assert_eq!(
        (
            draft.config.level,
            draft.config.ascension,
            draft.config.eidolon
        ),
        (80, 6, 0)
    );
    assert_eq!(
        draft.config.unlocked_trace_ids,
        vec!["trace.child", "trace.first"]
    );
    draft.affinitive_eidolon = true;
    catalog.validate_loadout(id, &draft).unwrap();
    assert_eq!(
        catalog.effective_skill_level(id, &draft, "basic").unwrap(),
        6
    );
    draft.config.eidolon = 3;
    assert_eq!(
        catalog.effective_skill_level(id, &draft, "basic").unwrap(),
        7
    );
    assert_eq!(draft.config.skill_levels["basic"], 6);
    let cone = catalog
        .default_light_cone("fixture.destruction_cone")
        .unwrap();
    assert_eq!(
        (cone.level, cone.ascension, cone.superimposition),
        (80, 6, 1)
    );
    let data = catalog.light_cone(&cone.data_id).unwrap();
    for rank in 1..=5 {
        assert_eq!(
            data.rank(rank).unwrap().parameters,
            vec![0.125 * f64::from(rank)]
        );
    }
    assert!(data.rank(0).is_err());
    assert!(data.rank(6).is_err());
}

#[test]
fn archive_rejects_incompatible_versions_and_missing_or_duplicate_team_members() {
    let template = Session::from_json(EXAMPLE).unwrap();
    let db = TempDb::new();
    let mut archive = seed(&db.path(), &template);
    let mut team = team(&template, "alpha");
    let first = team.character_form_ids[0].clone();
    team.character_form_ids[1] = first;
    assert!(archive.save_team(&team).is_err());
    team.character_form_ids[1] = "missing.profile".into();
    assert!(archive.save_team(&team).is_err());
    assert!(archive.team_ids().unwrap().is_empty());
    let mut different = training_catalog(&template);
    different.data_version = "fixture.other".into();
    assert!(matches!(
        Archive::open(db.path(), different),
        Err(Error::Version {
            kind: "archive catalog",
            ..
        })
    ));
    drop(archive);
    let conn = Connection::open(db.path()).unwrap();
    conn.pragma_update(None, "user_version", 99).unwrap();
    assert!(matches!(
        Archive::open(db.path(), training_catalog(&template)),
        Err(Error::Version {
            kind: "archive schema",
            ..
        })
    ));
}

#[test]
fn catalog_rejects_changed_content_under_a_pinned_version() {
    use sha2::{Digest, Sha256};

    let template = Session::from_json(EXAMPLE).unwrap();
    let content = serde_json::to_string(&training_catalog(&template)).unwrap();
    let hash = Sha256::digest(content.as_bytes());
    let document = format!(r#"{{"data_version":"sr.0.0.m03.{hash:x}","content":{content}}}"#);
    assert!(Catalog::from_json(&document).is_ok());
    let altered = document.replace("训练靴", "已修改");
    assert!(matches!(
        Catalog::from_json(&altered),
        Err(Error::Invalid {
            field: "catalog.data_version",
            ..
        })
    ));
}
