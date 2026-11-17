use star_rail_battle_core::Session;
use star_rail_profiles::{Archive, LightConeEquipment, Relic, RelicSlot, training_catalog};

use crate::{
    character_archive::{CharacterArchive, Error},
    workbench_tests::TestData,
};

const EXAMPLE: &str = include_str!("../../contracts/battle-session.v1.example.json");

#[cfg(all(not(debug_assertions), bundled_catalog))]
#[test]
fn clean_release_archive_opens_the_bundled_roster() {
    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let index = serde_json::to_value(archive.index().unwrap()).unwrap();
    let roster = index["characters"].as_array().unwrap();
    assert_eq!(roster.len(), 94);
    for (id, rarity) in [("character.1003", 5), ("march_7th.hunt", 4)] {
        let character = roster.iter().find(|character| character["id"] == id);
        assert_eq!(character.unwrap()["rarity"], rarity);
    }
    assert!(!dir.path().join("catalog.json").exists());
    let himeko = serde_json::to_value(archive.profile("character.1003").unwrap()).unwrap();
    assert!(himeko["edit"]["light_cone"].is_null());
    let traces = &himeko["trace_bonuses"];
    assert!((traces["attack_ratio"].as_f64().unwrap() - 0.18).abs() < 1e-9);
    assert!((traces["fireaddedratio"].as_f64().unwrap() - 0.224).abs() < 1e-9);
}

#[test]
fn identical_relics_are_distinct_and_a_conflict_keeps_the_saved_character() {
    let template = Session::from_json(EXAMPLE).unwrap();
    let dir = TestData::new();
    let first = &template.input.team[0].character_form_id;
    let second = &template.input.team[1].character_form_id;
    let catalog = training_catalog(&template);
    let mut cone = catalog
        .default_light_cone("fixture.destruction_cone")
        .unwrap();
    cone.superimposition = 3;
    let mut archive = CharacterArchive::open(dir.path(), catalog).unwrap();
    let index = serde_json::to_value(archive.index().unwrap()).unwrap();
    let roster = index["characters"].as_array().unwrap();
    assert_eq!(roster.len(), template.input.team.len());
    assert!(roster.iter().all(|character| character["rarity"] == 5));
    let options = archive.relic_options("fixture.training_boots", 3).unwrap();
    assert_eq!(options.substats["speed"], [2_000_000, 2_300_000, 2_600_000]);

    let boots = Relic {
        data_id: "fixture.training_boots".into(),
        level: 3,
        main_stat: "attack_ratio".into(),
        substats: [("speed".into(), 2_300_000)].into(),
    };
    archive.save_relic(None, &boots).unwrap();
    let inventory = archive.save_relic(None, &boots).unwrap();
    let (id, other_id) = (inventory[0].id, inventory[1].id);
    assert_ne!(id, other_id);

    let mut draft = archive.profile(first).unwrap().edit;
    draft.eidolon = 3;
    draft.affinitive_eidolon = true;
    draft.light_cone = Some(LightConeEquipment::Custom(cone.clone()));
    draft.relics.insert(RelicSlot::Feet, id);
    archive.save(first, draft.clone()).unwrap();
    assert!(archive.delete_relic(id).is_err());

    let second_saved = archive.profile(second).unwrap().edit;
    let mut conflict = second_saved.clone();
    conflict.eidolon = 2;
    conflict.light_cone = Some(LightConeEquipment::Custom(cone));
    conflict.relics.insert(RelicSlot::Feet, id);
    assert!(matches!(
        archive.save(second, conflict.clone()),
        Err(Error::Profile(star_rail_profiles::Error::Invalid {
            field: "inventory.occupied",
            ..
        }))
    ));
    assert_eq!(archive.profile(second).unwrap().edit, second_saved);
    conflict.relics.insert(RelicSlot::Feet, other_id);
    archive.save(second, conflict.clone()).unwrap();

    let slots = [None, Some(first.clone()), None, Some(second.clone())];
    let team = archive.save_team(None, "样例配队", &slots).unwrap();
    drop(archive);

    let mut reopened = CharacterArchive::open(dir.path(), training_catalog(&template)).unwrap();
    let saved = serde_json::to_value(reopened.index().unwrap()).unwrap();
    assert_eq!(saved["teams"], serde_json::json!([team]));
    assert_eq!(reopened.profile(first).unwrap().edit, draft);
    assert_eq!(reopened.profile(second).unwrap().edit, conflict);
    assert_eq!(
        reopened.inventory().unwrap()[0].owner_id.as_deref(),
        Some(first.as_str())
    );

    draft.relics.clear();
    draft.light_cone = None;
    assert!(
        reopened
            .save(first, draft)
            .unwrap()
            .edit
            .light_cone
            .is_none()
    );
    conflict.relics.insert(RelicSlot::Feet, id);
    reopened.save(second, conflict).unwrap();
    assert!(reopened.delete_relic(other_id).is_ok());
}

#[test]
fn opens_the_legacy_team_and_saves_independent_named_formations() {
    let template = Session::from_json(EXAMPLE).unwrap();
    let dir = TestData::new();
    let catalog = training_catalog(&template);
    let first = template.input.team[0].character_form_id.clone();
    let second = template.input.team[1].character_form_id.clone();
    let legacy_slots = [None, Some(first.clone()), None, None];
    let db = dir.path().join("profiles.sqlite");
    let mut legacy = Archive::open(&db, catalog.clone()).unwrap();
    legacy.ensure_profile(&first).unwrap();
    legacy
        .save_team_slots("desktop.team", "当前队伍", &legacy_slots)
        .unwrap();
    drop(legacy);

    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let slots = [Some(second.clone()), None, None, Some(first.clone())];
    let created = archive.save_team(None, "第二队", &slots).unwrap();
    let empty = archive
        .save_team(None, "空白配队", &Default::default())
        .unwrap();
    assert_ne!(created.id, empty.id);
    assert!(
        archive
            .save_team(
                Some(&created.id),
                "重复角色",
                &[Some(first.clone()), Some(first), None, None]
            )
            .is_err()
    );
    assert!(
        archive
            .save_team(Some("team.missing"), "无效配队", &slots)
            .is_err()
    );
    let legacy = archive
        .save_team(Some("desktop.team"), "第一队", &legacy_slots)
        .unwrap();
    drop(archive);

    let reopened = CharacterArchive::open(dir.path(), catalog).unwrap();
    let index = serde_json::to_value(reopened.index().unwrap()).unwrap();
    assert_eq!(index["teams"], serde_json::json!([legacy, created, empty]));
    assert_eq!(legacy.slots, legacy_slots);
    assert_eq!(created.slots, slots);
}
