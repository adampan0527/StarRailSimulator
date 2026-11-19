use star_rail_battle_core::Session;
use star_rail_profiles::{
    Affix, Archive, LightConeEquipment, Relic, RelicImportSummary, RelicSlot, training_catalog,
};

use crate::{
    character_archive::{CharacterArchive, Error},
    workbench_tests::TestData,
};

const EXAMPLE: &str = include_str!("../../contracts/battle-session.v1.example.json");

#[test]
fn imports_snapshot_relics_into_a_migrated_inventory_once() {
    let dir = TestData::new();
    let template = Session::from_json(EXAMPLE).unwrap();
    let mut catalog = training_catalog(&template);
    let mut boots = catalog.relics.remove("fixture.training_boots").unwrap();
    boots.set_id = "set.102".into();
    catalog.relics.insert("relic.31024".into(), boots);
    let set = catalog.relic_sets.remove("fixture.training").unwrap();
    catalog.relic_sets.insert("set.102".into(), set);

    let expected = Relic {
        data_id: "relic.31024".into(),
        level: 3,
        main_stat: "attack_ratio".into(),
        substats: [("speed".into(), 2_300_000)].into(),
    };
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let manual_id = archive.save_relic(None, &expected).unwrap()[0].id;
    drop(archive);
    rusqlite::Connection::open(dir.path().join("profiles.sqlite"))
        .unwrap()
        .execute_batch("DROP TABLE imported_relics; PRAGMA user_version = 3;")
        .unwrap();

    let relic = |uid: &str, value: f64| {
        serde_json::json!({
            "set_id": "102", "name": "Musketeer of Wild Wheat", "slot": "Feet",
            "rarity": 2, "level": 3, "mainstat": "ATK",
            "substats": [{ "key": "SPD", "value": value, "count": 1, "step": 1 }],
            "location": "1001", "lock": true, "discard": false, "_uid": uid
        })
    };
    let mut snapshot = serde_json::json!({
        "format": "star-rail-simulator.relic-export", "version": 1,
        "source": "reliquary_archiver", "source_version": 4,
        "relics": [relic("first", 2.29999995), relic("second", 2.29999995)]
    });
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    assert_eq!(
        archive.import_relics(&snapshot.to_string()).unwrap(),
        RelicImportSummary {
            added: 2,
            equipped: 1
        }
    );
    let inventory = archive.inventory().unwrap();
    assert_eq!(inventory.len(), 3);
    assert_eq!(inventory[2].id, manual_id);
    assert!(inventory[0].owner_id.is_none());
    assert_eq!(
        inventory[1].owner_id.as_deref(),
        Some("march_7th.preservation")
    );
    assert!(inventory.iter().all(|item| item.relic == expected));
    let ids: Vec<_> = inventory.iter().map(|item| item.id).collect();
    drop(archive);

    let mut archive = CharacterArchive::open(dir.path(), catalog).unwrap();
    assert_eq!(
        archive.import_relics(&snapshot.to_string()).unwrap(),
        RelicImportSummary::default()
    );
    snapshot["relics"] = serde_json::json!([relic("third", 2.3), relic("invalid", 5.0)]);
    assert!(archive.import_relics(&snapshot.to_string()).is_err());
    assert_eq!(
        archive
            .inventory()
            .unwrap()
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        ids
    );
}

#[test]
fn backfills_v4_relic_details_and_preserves_local_equipment() {
    let dir = TestData::new();
    let template = Session::from_json(EXAMPLE).unwrap();
    let mut catalog = training_catalog(&template);
    let boots = catalog.relics.get_mut("fixture.training_boots").unwrap();
    boots.rarity = 5;
    boots.max_level = 15;
    boots.initial_substats = [3, 4];
    boots.set_id = "set.102".into();
    let set = catalog.relic_sets.remove("fixture.training").unwrap();
    catalog.relic_sets.insert("set.102".into(), set);
    let group = catalog.sub_affixes.get_mut("sub.feet").unwrap();
    for (key, base, step) in [
        ("hp_flat", 33.87, 4.234),
        ("crit_rate", 0.02592, 0.00324),
        ("crit_damage", 0.05184, 0.00648),
    ] {
        group.insert(
            key.into(),
            Affix {
                base,
                step,
                step_count: 2,
            },
        );
    }

    let fourth =
        serde_json::json!({ "key": "CRIT DMG_", "value": 5.8320003, "count": 1, "step": 1 });
    let mut preview = serde_json::json!({
        "set_id": "102", "name": "Musketeer of Wild Wheat", "slot": "Feet",
        "rarity": 5, "level": 0, "mainstat": "ATK_",
        "substats": [
            { "key": "SPD", "value": 2.3, "count": 1, "step": 1 },
            { "key": "HP", "value": 33.87, "count": 1, "step": 0 },
            { "key": "CRIT Rate_", "value": 2.592, "count": 1, "step": 0 }
        ],
        "location": "", "lock": false, "discard": false, "_uid": "preview"
    });
    let mut enhanced = preview.clone();
    enhanced["level"] = 6.into();
    enhanced["_uid"] = "enhanced".into();
    enhanced["substats"][0] =
        serde_json::json!({ "key": "SPD", "value": 4.3, "count": 2, "step": 1 });
    enhanced["substats"]
        .as_array_mut()
        .unwrap()
        .push(fourth.clone());
    let snapshot = |a: &serde_json::Value, b: &serde_json::Value| {
        serde_json::json!({
            "source": "reliquary_archiver", "version": 4, "relics": [a, b]
        })
        .to_string()
    };
    let march = "march_7th.preservation";
    let dan = "dan_heng.hunt";

    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    archive
        .import_relics(&snapshot(&preview, &enhanced))
        .unwrap();
    let inventory = archive.inventory().unwrap();
    let enhanced_id = inventory[0].id;
    let preview_id = inventory[1].id;
    let mut edit = archive.profile(march).unwrap().edit;
    edit.relics.insert(RelicSlot::Feet, enhanced_id);
    archive.save(march, edit).unwrap();
    drop(archive);
    rusqlite::Connection::open(dir.path().join("profiles.sqlite"))
        .unwrap()
        .execute_batch(
            "ALTER TABLE imported_relics DROP COLUMN source_details; PRAGMA user_version = 4;",
        )
        .unwrap();

    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    assert!(
        archive
            .inventory()
            .unwrap()
            .iter()
            .all(|item| item.source.is_none())
    );
    preview["location"] = "1001".into();
    preview["preview_substats"] = serde_json::json!([fourth]);
    enhanced["location"] = "1002".into();
    assert_eq!(
        archive
            .import_relics(&snapshot(&preview, &enhanced))
            .unwrap(),
        RelicImportSummary::default()
    );
    drop(archive);

    let mut archive = CharacterArchive::open(dir.path(), catalog).unwrap();
    let inventory = archive.inventory().unwrap();
    assert_eq!(inventory.len(), 2);
    assert_eq!(inventory[0].id, enhanced_id);
    assert_eq!(inventory[0].owner_id.as_deref(), Some(march));
    let source = inventory[0].source.as_ref().unwrap();
    assert_eq!(source.location, "1002");
    assert_eq!(source.rolls["speed"].count, 2);
    assert_eq!(source.rolls["speed"].step, 1);
    assert!(archive.profile(dan).unwrap().edit.relics.is_empty());
    assert_eq!(inventory[1].id, preview_id);
    assert!(inventory[1].owner_id.is_none());
    let source = inventory[1].source.as_ref().unwrap();
    assert_eq!(source.location, "1001");
    assert_eq!(
        source.preview_substats.as_ref().unwrap()["crit_damage"],
        58_320
    );
    assert!(!inventory[1].relic.substats.contains_key("crit_damage"));

    let mut draft = inventory[0].relic.clone();
    draft.substats.insert("speed".into(), 4_600_000);
    archive.save_relic(Some(enhanced_id), &draft).unwrap();
    let mut edit = archive.profile(march).unwrap().edit;
    edit.relics.clear();
    archive.save(march, edit).unwrap();
    preview.as_object_mut().unwrap().remove("preview_substats");
    assert_eq!(
        archive
            .import_relics(&snapshot(&preview, &enhanced))
            .unwrap(),
        RelicImportSummary::default()
    );
    let inventory = archive.inventory().unwrap();
    assert!(inventory.iter().all(|item| item.owner_id.is_none()));
    assert_eq!(inventory[0].relic, draft);
    let source = inventory[0].source.as_ref().unwrap();
    assert_eq!(source.location, "1002");
    assert!(source.rolls.is_empty());
    assert!(
        inventory[1]
            .source
            .as_ref()
            .unwrap()
            .preview_substats
            .is_some()
    );
}

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
