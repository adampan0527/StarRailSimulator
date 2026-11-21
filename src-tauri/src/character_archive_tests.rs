use star_rail_battle_core::Session;
use star_rail_profiles::{
    Affix, Archive, LightConeEquipment, Relic, RelicImportSummary, RelicSlot, training_catalog,
};

use crate::{
    character_archive::{CharacterArchive, Error},
    desktop_relic_export::ExportOrigin,
    workbench_tests::TestData,
};

const EXAMPLE: &str = include_str!("../../contracts/battle-session.v1.example.json");

#[test]
fn dpr_checks_selected_saved_team_against_fixed_scenario() {
    let dir = TestData::new();
    let session = Session::from_json(EXAMPLE).unwrap();
    let mut archive = CharacterArchive::open(dir.path(), training_catalog(&session)).unwrap();
    let slots = [
        Some("dan_heng.hunt".into()),
        None,
        Some("march_7th.preservation".into()),
        None,
    ];
    let selected = archive.save_team(None, "固定靶场", &slots).unwrap();
    let empty = archive
        .save_team(None, "空白配队", &[None, None, None, None])
        .unwrap();

    let checked = archive.dpr_readiness(&selected.id).unwrap();
    assert_eq!(checked.scenario.cycles.get(), 3);
    assert_eq!(checked.scenario.toughness, [30, 120, 30]);
    assert!(checked.result.is_none());
    assert_eq!(
        checked.unsupported_characters,
        ["丹恒 · 巡猎", "三月七 · 存护"]
    );
    assert!(archive.dpr_readiness(&empty.id).is_err());
    assert!(archive.dpr_readiness("team.missing").is_err());
}

#[cfg(bundled_catalog)]
#[test]
fn official_saved_team_runs_fixed_dpr_with_resolved_loadouts() {
    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let arlan = "character.1008";
    let herta = "character.1013";
    let serval = "character.1103";
    let slots = [
        None,
        Some(arlan.into()),
        Some(herta.into()),
        Some(serval.into()),
    ];
    let saved = archive.save_team(None, "角色靶场", &slots).unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&saved.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let first = run();
    assert!(first.total_damage > 0);
    assert_eq!(
        first
            .characters
            .iter()
            .map(|entry| entry.unit_id.as_str())
            .collect::<Vec<_>>(),
        [arlan, herta, serval],
    );
    assert!(first.characters.iter().all(|entry| entry.own_damage > 0));
    assert_eq!(first.targets.len(), 3);
    assert_eq!(
        first
            .targets
            .iter()
            .map(|target| target.damage)
            .sum::<u64>(),
        first.total_damage
    );
    assert_eq!(run(), first);

    let slots = [
        Some(arlan.into()),
        Some("character.1004".into()),
        None,
        None,
    ];
    let mixed = archive.save_team(None, "有未接入角色", &slots).unwrap();
    let checked = archive.dpr_readiness(&mixed.id).unwrap();
    assert_eq!(checked.unsupported_characters, ["瓦尔特"]);
    assert!(checked.result.is_none());
}

#[cfg(bundled_catalog)]
#[test]
fn fixed_dpr_for_saved_team_credits_pelas_debuff_damage() {
    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let team = archive
        .save_team(
            None,
            "佩拉与巡猎",
            &[
                Some("character.1106".into()),
                Some("character.1013".into()),
                Some("dan_heng.hunt".into()),
                Some("natasha.abundance".into()),
            ],
        )
        .unwrap();
    let checked = archive.dpr_readiness(&team.id).unwrap();
    assert!(checked.unsupported_characters.is_empty());
    let result = checked.result.unwrap();
    assert_eq!(result.characters.len(), 4);
    assert!(result.characters.iter().all(|entry| entry.own_damage > 0));
    assert!(result.characters[0].assist_damage > 0);
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|entry| entry.own_damage + entry.assist_damage)
            .sum::<u64>()
    );
}

#[cfg(bundled_catalog)]
#[test]
fn saved_team_with_preservation_characters_does_not_counter_idle_dummies() {
    use star_rail_battle_core::DamageKind;

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let saved = archive
        .save_team(
            None,
            "护盾与反击",
            &[
                Some("march_7th.preservation".into()),
                Some("character.1104".into()),
                Some("character.1107".into()),
                None,
            ],
        )
        .unwrap();
    let checked = archive.dpr_readiness(&saved.id).unwrap();
    assert!(checked.unsupported_characters.is_empty());
    let result = checked.result.unwrap();
    assert!(result.characters.iter().all(|actor| actor.own_damage > 0));
    assert!(
        !result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| hit.kind == DamageKind::FollowUp)
    );
}

#[cfg(bundled_catalog)]
#[test]
fn saved_periodic_damage_team_keeps_the_source_for_each_character() {
    use star_rail_battle_core::DamageKind;

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let forms = ["character.1109", "character.1108", "character.1111"];
    let saved = archive
        .save_team(
            None,
            "持续伤害",
            &[
                Some(forms[0].into()),
                Some(forms[1].into()),
                Some(forms[2].into()),
                None,
            ],
        )
        .unwrap();
    let result = archive.dpr_readiness(&saved.id).unwrap().result.unwrap();
    for form in forms {
        assert!(
            result
                .targets
                .iter()
                .flat_map(|target| &target.hits)
                .any(|hit| hit.source_id == form && hit.kind == DamageKind::DamageOverTime),
            "{form}"
        );
    }
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|entry| entry.own_damage + entry.assist_damage)
            .sum::<u64>()
    );
}

#[cfg(bundled_catalog)]
#[test]
fn saved_break_and_follow_up_team_runs_with_the_frozen_catalog() {
    use star_rail_battle_core::DamageKind;

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let forms = ["character.1003", "character.1206", "character.1209"];
    let saved = archive
        .save_team(
            None,
            "击破与追击",
            &[
                Some(forms[0].into()),
                Some(forms[1].into()),
                Some(forms[2].into()),
                None,
            ],
        )
        .unwrap();
    let run = || archive.dpr_readiness(&saved.id).unwrap().result.unwrap();
    let result = run();
    assert!(result.characters.iter().all(|actor| actor.own_damage > 0));
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.kind == DamageKind::WeaknessBreak));
    assert!(hits().any(|hit| hit.kind == DamageKind::FollowUp && hit.source_id == forms[0]));
    assert_eq!(result, run());
}

#[cfg(bundled_catalog)]
#[test]
fn support_team_credits_actual_buff_damage_and_conserves_the_total() {
    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let himeko = "character.1003";
    let supports = ["character.1009", "character.1101", "character.1202"];
    let team = archive
        .save_team(
            None,
            "同谐归因",
            &[
                Some(himeko.into()),
                Some(supports[0].into()),
                Some(supports[1].into()),
                Some(supports[2].into()),
            ],
        )
        .unwrap();
    let run = || archive.dpr_readiness(&team.id).unwrap().result.unwrap();
    let result = run();
    assert_eq!(result.characters.len(), 4);
    assert!(result.characters[0].own_damage > 0);
    for character in &result.characters[1..] {
        assert!(character.assist_damage > 0, "{}", character.unit_id);
    }
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.source_id == himeko
                    && hit
                        .credits
                        .iter()
                        .any(|credit| credit.unit_id == supports[2] && credit.amount > 0)
            })
    );
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|entry| entry.own_damage + entry.assist_damage)
            .sum::<u64>()
    );
    assert_eq!(result, run());
}

#[cfg(bundled_catalog)]
#[test]
fn saved_detonation_team_records_the_original_sources_and_support_credits() {
    use star_rail_battle_core::{Action, DamageKind};

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [guinaifen, jiaoqiu, kafka, sampo] = [
        "character.1210",
        "character.1218",
        "character.1005",
        "character.1108",
    ];
    let team = archive
        .save_team(
            None,
            "持续伤害引爆",
            &[guinaifen, jiaoqiu, kafka, sampo].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    for source in [guinaifen, jiaoqiu, kafka, sampo] {
        assert!(
            hits().any(|hit| { hit.source_id == source && hit.kind == DamageKind::DamageOverTime }),
            "{source}"
        );
    }
    assert!(result.targets.iter().any(|target| {
        target.hits.iter().enumerate().any(|(index, hit)| {
            hit.source_id == kafka
                && hit.action == Action::Ultimate
                && hit.kind == DamageKind::Direct
                && target.hits[index + 1..]
                    .iter()
                    .take_while(|next| next.av_micros == hit.av_micros)
                    .any(|next| {
                        next.source_id == sampo
                            && next.action == Action::Ultimate
                            && next.kind == DamageKind::DamageOverTime
                    })
        })
    }));
    for support in [guinaifen, jiaoqiu] {
        assert!(
            hits().any(|hit| {
                hit.source_id != support
                    && hit
                        .credits
                        .iter()
                        .any(|credit| credit.unit_id == support && credit.amount > 0)
            }),
            "{support}"
        );
    }
    for hit in hits() {
        assert_eq!(
            hit.amount,
            hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
        );
    }
    assert_eq!(
        result.total_damage,
        result
            .targets
            .iter()
            .map(|target| target.damage)
            .sum::<u64>()
    );
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|actor| actor.own_damage + actor.assist_damage)
            .sum::<u64>()
    );
    assert_eq!(result, run());
}

#[cfg(bundled_catalog)]
#[test]
fn saved_break_support_team_runs_enhanced_basic_and_credits_direct_buffs() {
    use star_rail_battle_core::{Action, DamageKind};

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [gallagher, hanya, yukong, himeko] = [
        "character.1301",
        "character.1215",
        "character.1207",
        "character.1003",
    ];
    let mut gallagher_edit = archive.profile(gallagher).unwrap().edit;
    gallagher_edit.eidolon = 1;
    archive.save(gallagher, gallagher_edit).unwrap();
    let saved = archive
        .save_team(
            None,
            "击破与战技点支援",
            &[gallagher, hanya, yukong, himeko].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&saved.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    assert!(result.characters.iter().all(|actor| actor.own_damage > 0));
    assert!(result.targets.iter().any(|target| {
        target.hits.iter().enumerate().any(|(index, hit)| {
            hit.source_id == gallagher
                && hit.action == Action::Ultimate
                && hit.kind == DamageKind::WeaknessBreak
                && target.hits[index + 1..]
                    .iter()
                    .take_while(|next| next.av_micros == hit.av_micros)
                    .any(|next| {
                        next.source_id == gallagher
                            && next.action == Action::Basic
                            && next.kind == DamageKind::Direct
                    })
        })
    }));
    for source in [hanya, yukong] {
        assert!(
            result
                .targets
                .iter()
                .flat_map(|target| &target.hits)
                .any(|hit| {
                    hit.source_id != source
                        && hit.kind == DamageKind::Direct
                        && hit
                            .credits
                            .iter()
                            .any(|credit| credit.unit_id == source && credit.amount > 0)
                }),
            "{source}"
        );
    }
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|actor| actor.own_damage + actor.assist_damage)
            .sum::<u64>()
    );
    assert_eq!(result, run());
}

#[cfg(bundled_catalog)]
#[test]
fn saved_abundance_team_credits_eidolon_damage_bonus_and_resistance_down() {
    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let bailu = "character.1211";
    let luocha = "character.1203";
    let lynx = "character.1110";
    let forms = [bailu, luocha, lynx, "character.1003"];
    for (form, eidolon) in [(bailu, 4), (luocha, 6)] {
        let mut edit = archive.profile(form).unwrap().edit;
        edit.eidolon = eidolon;
        archive.save(form, edit).unwrap();
    }
    let team = archive
        .save_team(None, "治疗与伤害支援", &forms.map(|id| Some(id.into())))
        .unwrap();
    let checked = archive.dpr_readiness(&team.id).unwrap();
    assert!(checked.unsupported_characters.is_empty());
    let result = checked.result.unwrap();
    assert_eq!(
        result
            .characters
            .iter()
            .map(|actor| actor.unit_id.as_str())
            .collect::<Vec<_>>(),
        forms
    );
    for source in [bailu, luocha] {
        assert!(
            result
                .targets
                .iter()
                .flat_map(|target| &target.hits)
                .any(|hit| {
                    hit.source_id != source
                        && hit
                            .credits
                            .iter()
                            .any(|credit| credit.unit_id == source && credit.amount > 0)
                }),
            "{source}"
        );
    }
    assert!(result.characters[2].own_damage > 0);
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|actor| actor.own_damage + actor.assist_damage)
            .sum::<u64>()
    );
    assert_eq!(
        result,
        archive.dpr_readiness(&team.id).unwrap().result.unwrap()
    );
}

#[cfg(bundled_catalog)]
#[test]
fn saved_follow_up_team_replays_bounces_and_keeps_each_damage_source() {
    use star_rail_battle_core::{Action, DamageKind};

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [misha, moze, xueyi, hanya] = [
        "character.1312",
        "character.1223",
        "character.1214",
        "character.1215",
    ];
    let team = archive
        .save_team(
            None,
            "弹射与追击",
            &[misha, moze, xueyi, hanya].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    let ultimate_hits = || {
        hits().filter(|hit| {
            hit.source_id == misha
                && hit.action == Action::Ultimate
                && hit.kind == DamageKind::Direct
        })
    };
    let ultimate = ultimate_hits().next().unwrap().av_micros;
    assert!(
        ultimate_hits()
            .filter(|hit| hit.av_micros == ultimate)
            .count()
            >= 4
    );
    for teammate in [moze, xueyi] {
        assert!(
            hits().any(|hit| hit.source_id == teammate && hit.kind == DamageKind::FollowUp),
            "{teammate}"
        );
    }
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|actor| actor.own_damage + actor.assist_damage)
            .sum::<u64>()
    );
    assert_eq!(result, run());
}

#[cfg(bundled_catalog)]
#[test]
fn saved_follow_up_team_credits_mozes_prey_on_ratios_talent() {
    use star_rail_battle_core::DamageKind;

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let ratio = "character.1305";
    let moze = "character.1223";
    let forms = [ratio, moze, "character.1106", "character.1110"];
    let team = archive
        .save_team(None, "负面状态与追击", &forms.map(|id| Some(id.into())))
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| {
        hit.source_id == ratio
            && hit.kind == DamageKind::FollowUp
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == moze && credit.amount > 0)
    }));
    assert!(hits().any(|hit| hit.source_id == moze && hit.kind == DamageKind::FollowUp));
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|actor| actor.own_damage + actor.assist_damage)
            .sum::<u64>()
    );
    assert_eq!(result, run());
}

#[cfg(bundled_catalog)]
#[test]
fn saved_follow_up_team_tracks_numby_and_debt_assists() {
    use star_rail_battle_core::DamageKind;

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [ratio, topaz, march, moze] = [
        "character.1305",
        "character.1112",
        "march_7th.hunt",
        "character.1223",
    ];
    let team = archive
        .save_team(
            None,
            "账账与师父",
            &[ratio, topaz, march, moze].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == topaz && hit.kind == DamageKind::FollowUp));
    assert!(hits().any(|hit| {
        hit.source_id == ratio
            && hit.kind == DamageKind::FollowUp
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == topaz && credit.amount > 0)
    }));
    assert!(hits().any(|hit| hit.source_id == march && hit.kind == DamageKind::Direct));
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|actor| actor.own_damage + actor.assist_damage)
            .sum::<u64>()
    );
    assert_eq!(result, run());
}

fn remove_archiver_schema(db: &rusqlite::Connection) {
    db.execute_batch(
        "DROP TABLE archiver_relics;
         DROP TABLE archiver_light_cones;
         DROP TABLE archiver_progression;
         DROP TABLE archiver_account;",
    )
    .unwrap();
}

#[test]
fn imports_archiver_account_over_an_existing_relic_archive() {
    let dir = TestData::new();
    let template = Session::from_json(EXAMPLE).unwrap();
    let mut catalog = training_catalog(&template);
    let id = "march_7th.preservation";
    let variant = catalog
        .characters
        .get_mut(id)
        .unwrap()
        .variants
        .get_mut("base")
        .unwrap();
    variant.skills.get_mut("basic").unwrap().kind = "Normal".into();
    let trace = variant.traces.remove("trace.first").unwrap();
    variant.traces.clear();
    variant.traces.insert("trace.1001101".into(), trace);
    let cone = catalog
        .light_cones
        .remove("fixture.destruction_cone")
        .unwrap();
    catalog.light_cones.insert("cone.20000".into(), cone);
    let boots = catalog.relics.remove("fixture.training_boots").unwrap();
    catalog.relics.insert("relic.31024".into(), boots);

    let mut account = serde_json::json!({
        "source": "reliquary_archiver", "version": 4,
        "metadata": { "uid": 123, "trailblazer": "Stelle" },
        "characters": [
            {"id": "1001", "level": 45, "ascension": 3, "eidolon": 2, "ability_version": 0,
             "skills": {"basic": 3}, "traces": {"ability_1": true}},
            {"id": "1002", "level": 0, "ascension": 0, "eidolon": 0, "ability_version": 0,
             "skills": {}, "traces": {}}
        ],
        "light_cones": [
            {"id": "20000", "level": 40, "ascension": 3, "superimposition": 2, "location": "1001", "_uid": "10"},
            {"id": "20000", "level": 40, "ascension": 3, "superimposition": 2, "location": "", "_uid": "11"}
        ],
        "relics": [{
            "id": "31024", "name": "Test boots", "slot": "Feet", "rarity": 2, "level": 3,
            "mainstat": "ATK", "substats": [{"key": "SPD", "value": 2.3, "count": 1, "step": 1}],
            "location": "1001", "lock": false, "discard": false, "_uid": "20"
        }]
    });

    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let v1 = serde_json::json!({
        "format": "star-rail-simulator.relic-export", "version": 1,
        "source": "reliquary_archiver", "source_version": 4, "relics": account["relics"],
    });
    archive.import_relics(&v1.to_string()).unwrap();
    let relic_id = archive.inventory().unwrap()[0].id;
    drop(archive);
    {
        let db = rusqlite::Connection::open(dir.path().join("profiles.sqlite")).unwrap();
        remove_archiver_schema(&db);
        db.pragma_update(None, "user_version", 5).unwrap();
    }

    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let live = serde_json::json!({ "event": "InitialScan", "data": account });
    let summary = archive
        .import_account(&live.to_string(), ExportOrigin::Live)
        .unwrap();
    assert_eq!(summary.characters_applied, 1);
    assert_eq!(summary.characters_skipped, 1);
    assert_eq!(summary.light_cones_added, 2);
    assert_eq!(summary.light_cones_equipped, 1);
    assert_eq!(summary.relics.added, 0);
    let profile = archive.profile(id).unwrap();
    let Some(LightConeEquipment::Inventory(cone_id)) = profile.edit.light_cone else {
        panic!("expected an equipped inventory light cone");
    };
    assert_eq!(profile.edit.relics[&RelicSlot::Feet], relic_id);
    drop(archive);

    let db = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let profile = db.profile(id).unwrap();
    let config = &profile.presets[&profile.current_preset].config;
    assert_eq!((config.level, config.ascension, config.eidolon), (45, 3, 2));
    assert_eq!(config.skill_levels["basic"], 3);
    assert_eq!(config.unlocked_trace_ids, ["trace.1001101"]);
    drop(db);

    let mut archive = CharacterArchive::open(dir.path(), catalog).unwrap();
    let summary = archive
        .import_account(&account.to_string(), ExportOrigin::File)
        .unwrap();
    assert_eq!(summary.characters_applied, 0);
    assert_eq!(summary.light_cones_added, 0);
    assert_eq!(summary.relics.added, 0);

    account["characters"][0]["eidolon"] = 3.into();
    account["light_cones"][0]["superimposition"] = 0.into();
    assert!(
        archive
            .import_account(&account.to_string(), ExportOrigin::File)
            .is_err()
    );
    assert_eq!(archive.profile(id).unwrap().edit.eidolon, 2);

    account["light_cones"][0]["superimposition"] = 3.into();
    account["relics"][0]["level"] = 4.into();
    let summary = archive
        .import_account(&account.to_string(), ExportOrigin::File)
        .unwrap();
    assert_eq!(summary.characters_applied, 1);
    assert_eq!(summary.light_cones_updated, 1);
    assert_eq!(summary.relics_updated, 1);
    assert_eq!(
        archive.profile(id).unwrap().edit.light_cone,
        Some(LightConeEquipment::Inventory(cone_id))
    );
    assert_eq!(archive.inventory().unwrap()[0].id, relic_id);

    let mut edit = archive.profile(id).unwrap().edit;
    edit.eidolon = 5;
    edit.light_cone = None;
    edit.relics.clear();
    archive.save(id, edit.clone()).unwrap();
    account["characters"][0]["eidolon"] = 4.into();
    account["light_cones"][0]["superimposition"] = 4.into();
    let summary = archive
        .import_account(&account.to_string(), ExportOrigin::File)
        .unwrap();
    assert_eq!(summary.characters_preserved, 1);
    assert_eq!(archive.profile(id).unwrap().edit, edit);
    account["metadata"]["uid"] = 456.into();
    assert!(matches!(
        archive.import_account(&account.to_string(), ExportOrigin::File),
        Err(Error::Profile(star_rail_profiles::Error::Invalid {
            field: "archiver.uid",
            ..
        }))
    ));
}

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
    {
        let db = rusqlite::Connection::open(dir.path().join("profiles.sqlite")).unwrap();
        remove_archiver_schema(&db);
        db.execute_batch("DROP TABLE imported_relics; PRAGMA user_version = 3;")
            .unwrap();
    }

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
    {
        let db = rusqlite::Connection::open(dir.path().join("profiles.sqlite")).unwrap();
        remove_archiver_schema(&db);
        db.execute_batch(
            "ALTER TABLE imported_relics DROP COLUMN source_details; PRAGMA user_version = 4;",
        )
        .unwrap();
    }

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
