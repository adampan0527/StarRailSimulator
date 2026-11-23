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

#[cfg(bundled_catalog)]
mod archer;
#[cfg(bundled_catalog)]
mod dahlia;
#[cfg(bundled_catalog)]
mod rin;
#[cfg(bundled_catalog)]
mod saber;

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

    let acheron = "character.1308";
    let mut edit = archive.profile(acheron).unwrap().edit;
    edit.affinitive_eidolon = true;
    archive.save(acheron, edit).unwrap();
    let slots = [Some(arlan.into()), Some(acheron.into()), None, None];
    let mixed = archive.save_team(None, "有未接入配置", &slots).unwrap();
    let checked = archive.dpr_readiness(&mixed.id).unwrap();
    assert_eq!(checked.unsupported_characters, ["黄泉"]);
    assert!(checked.result.is_none());
}

#[cfg(bundled_catalog)]
#[test]
fn every_registered_official_variant_runs_from_a_saved_loadout() {
    use star_rail_battle_core::supports_official_dpr_variant;

    let dir = TestData::new();
    let catalog = star_rail_profiles::Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();

    for (form_id, character) in &catalog.characters {
        for variant in character.variants.keys() {
            if !supports_official_dpr_variant(form_id, variant) {
                continue;
            }
            profiles.ensure_profile(form_id).unwrap();
            let loadout = catalog.default_loadout_at(form_id, variant, 80, 6).unwrap();
            profiles
                .save_preset(form_id, variant, &loadout, true)
                .unwrap();
            let team = archive
                .save_team(None, form_id, &[Some(form_id.clone()), None, None, None])
                .unwrap();
            let checked = archive
                .dpr_readiness(&team.id)
                .unwrap_or_else(|error| panic!("{form_id}/{variant}: {error}"));
            let result = checked
                .result
                .unwrap_or_else(|| panic!("{form_id}/{variant}: unsupported saved loadout"));
            assert_eq!(result.characters[0].unit_id, *form_id);
            assert_eq!(
                result
                    .targets
                    .iter()
                    .map(|target| target.damage)
                    .sum::<u64>(),
                result.total_damage,
                "{form_id}/{variant}"
            );
        }
    }
}

#[cfg(bundled_catalog)]
#[test]
fn saved_aglaea_accepts_fractional_flat_hp_at_lower_talent_levels() {
    let dir = TestData::new();
    let catalog = star_rail_profiles::Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let aglaea = "character.1402";
    profiles.ensure_profile(aglaea).unwrap();
    let mut loadout = catalog.default_loadout_at(aglaea, "base", 80, 6).unwrap();
    let team = archive
        .save_team(
            None,
            "衣匠低级天赋",
            &[Some(aglaea.into()), None, None, None],
        )
        .unwrap();

    for level in [2, 4] {
        loadout
            .config
            .skill_levels
            .insert("skill.140204".into(), level);
        profiles
            .save_preset(aglaea, "base", &loadout, true)
            .unwrap();
        let readiness = archive.dpr_readiness(&team.id).unwrap();
        assert!(readiness.unsupported_characters.is_empty());
        let result = readiness.result.unwrap();
        assert!(
            result
                .targets
                .iter()
                .flat_map(|target| &target.hits)
                .any(|hit| hit.source_id == "character.1402:garmentmaker")
        );
    }
}

#[cfg(bundled_catalog)]
#[test]
fn saved_yao_guang_runs_elation_from_low_skill_levels() {
    use star_rail_battle_core::{Action, DamageKind};

    let dir = TestData::new();
    let catalog = star_rail_profiles::Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let yao_guang = "character.1502";
    profiles.ensure_profile(yao_guang).unwrap();
    let mut loadout = catalog
        .default_loadout_at(yao_guang, "base", 80, 6)
        .unwrap();
    loadout.config.eidolon = 0;
    for level in loadout.config.skill_levels.values_mut() {
        *level = 1;
    }
    profiles
        .save_preset(yao_guang, "低级技能", &loadout, true)
        .unwrap();
    let team = archive
        .save_team(
            None,
            "爻光低级技能",
            &[Some(yao_guang.into()), None, None, None],
        )
        .unwrap();
    let capture = || {
        let readiness = archive.dpr_readiness(&team.id).unwrap();
        assert!(readiness.unsupported_characters.is_empty());
        readiness.result.unwrap()
    };
    let result = capture();
    assert_eq!(result, capture());
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| hit.kind == DamageKind::Elation
                && hit.action == Action::ElationSkill
                && hit.source_id == yao_guang)
    );
    assert_eq!(result.characters[0].own_damage, result.total_damage);
}

#[cfg(bundled_catalog)]
#[test]
fn saved_hyacine_runs_fractional_skill_and_sprite_healing_coefficients() {
    let dir = TestData::new();
    let catalog = star_rail_profiles::Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let hyacine = "character.1409";
    profiles.ensure_profile(hyacine).unwrap();
    let mut loadout = catalog.default_loadout_at(hyacine, "base", 80, 6).unwrap();
    let team = archive
        .save_team(
            None,
            "风堇低等级配装",
            &[Some(hyacine.into()), None, None, None],
        )
        .unwrap();
    for level in [3, 7] {
        for id in ["skill.140902", "skill.140903"] {
            loadout.config.skill_levels.insert(id.into(), level);
        }
        loadout
            .config
            .skill_levels
            .insert("skill.1140903".into(), level.min(6));
        profiles
            .save_preset(hyacine, "base", &loadout, true)
            .unwrap();
        let readiness = archive.dpr_readiness(&team.id).unwrap();
        assert!(readiness.unsupported_characters.is_empty());
        let result = readiness.result.unwrap();
        assert!(
            result
                .targets
                .iter()
                .flat_map(|target| &target.hits)
                .any(|hit| { hit.source_id == "character.1409:little-ica" && hit.amount > 0 })
        );
    }
}

#[cfg(bundled_catalog)]
#[test]
fn saved_sparxie_and_yao_guang_share_aha_from_low_skill_levels() {
    use star_rail_battle_core::{Action, DamageKind};

    let dir = TestData::new();
    let catalog = star_rail_profiles::Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let [sparxie, yao] = ["character.1501", "character.1502"];
    for (id, eidolon) in [(sparxie, 2), (yao, 0)] {
        profiles.ensure_profile(id).unwrap();
        let mut loadout = catalog.default_loadout_at(id, "base", 80, 6).unwrap();
        loadout.config.eidolon = eidolon;
        for level in loadout.config.skill_levels.values_mut() {
            *level = 1;
        }
        profiles
            .save_preset(id, "低级技能", &loadout, true)
            .unwrap();
    }
    let team = archive
        .save_team(
            None,
            "火花与爻光",
            &[Some(sparxie.into()), Some(yao.into()), None, None],
        )
        .unwrap();
    let capture = || {
        let readiness = archive.dpr_readiness(&team.id).unwrap();
        assert!(readiness.unsupported_characters.is_empty());
        readiness.result.unwrap()
    };
    let result = capture();
    assert_eq!(result, capture());
    for unit in [sparxie, yao] {
        assert!(
            result
                .targets
                .iter()
                .flat_map(|target| &target.hits)
                .any(|hit| {
                    hit.source_id == unit
                        && hit.kind == DamageKind::Elation
                        && hit.action == Action::ElationSkill
                })
        );
    }
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>()
    );
}

#[cfg(bundled_catalog)]
#[test]
fn saved_evernight_and_remembrance_replay_two_real_memosprites() {
    let dir = TestData::new();
    let catalog = star_rail_profiles::Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let [evernight, remembrance] = ["character.1413", "trailblazer.remembrance"];
    profiles.ensure_profile(evernight).unwrap();
    let mut loadout = catalog
        .default_loadout_at(evernight, "base", 80, 6)
        .unwrap();
    loadout.config.eidolon = 2;
    for level in loadout.config.skill_levels.values_mut() {
        *level = 1;
    }
    profiles
        .save_preset(evernight, "低等级技能", &loadout, true)
        .unwrap();
    let team = archive
        .save_team(
            None,
            "长夜与迷迷",
            &[Some(evernight.into()), Some(remembrance.into()), None, None],
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    assert_eq!(result, run());
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == "character.1413:evey" && hit.amount > 0));
    assert!(hits().any(|hit| {
        hit.source_id == "trailblazer.remembrance:mem"
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == evernight && credit.amount > 0)
    }));
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>()
    );
}

#[cfg(bundled_catalog)]
#[test]
fn saved_permansor_uses_fractional_shields_and_keeps_bondmate_provenance() {
    use star_rail_battle_core::{Action, DamageKind};

    let dir = TestData::new();
    let catalog = star_rail_profiles::Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let [permansor, herta] = ["character.1414", "character.1013"];
    profiles.ensure_profile(permansor).unwrap();
    let mut loadout = catalog
        .default_loadout_at(permansor, "base", 80, 6)
        .unwrap();
    loadout.config.eidolon = 2;
    for skill in ["skill.141402", "skill.141403", "skill.141404"] {
        loadout.config.skill_levels.insert(skill.into(), 7);
    }
    profiles
        .save_preset(permansor, "base", &loadout, true)
        .unwrap();
    let team = archive
        .save_team(
            None,
            "同袍与龙灵",
            &[Some(permansor.into()), Some(herta.into()), None, None],
        )
        .unwrap();

    let run = || {
        let readiness = archive.dpr_readiness(&team.id).unwrap();
        assert!(readiness.unsupported_characters.is_empty());
        readiness.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == permansor && hit.kind == DamageKind::FollowUp));
    assert!(hits().any(|hit| {
        hit.source_id == herta
            && hit.kind == DamageKind::Additional
            && hit.action == Action::Skill
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == permansor && credit.amount > 0)
    }));
    assert_eq!(result, run());
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|role| role.own_damage + role.assist_damage)
            .sum::<u64>()
    );
}

#[cfg(bundled_catalog)]
#[test]
fn welt_fixed_dpr_uses_default_and_saved_base_variant() {
    let dir = TestData::new();
    let welt = "character.1004";
    let catalog_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    );
    let json = std::fs::read_to_string(catalog_path).unwrap();
    let catalog = star_rail_profiles::Catalog::from_json(&json).unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let team = archive
        .save_team(None, "瓦尔特靶场", &[Some(welt.into()), None, None, None])
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let enhanced = run();

    let base = catalog.default_loadout_at(welt, "base", 80, 6).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog).unwrap();
    let profile = profiles.profile(welt).unwrap();
    assert_eq!(
        profile.presets[&profile.current_preset].variant_id,
        "novaflare"
    );
    profiles.save_preset(welt, "原版", &base, true).unwrap();

    let original = run();
    assert_eq!(original, run());
    assert_eq!(original.characters[0].unit_id, welt);
    assert!(original.total_damage > 0);
    assert!(enhanced.total_damage > original.total_damage);
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

#[cfg(bundled_catalog)]
#[test]
fn saved_team_replays_summons_flying_aureus_and_arcana() {
    use star_rail_battle_core::{Action, DamageKind};

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [jing_yuan, feixiao, black_swan, topaz] = [
        "character.1204",
        "character.1220",
        "character.1307",
        "character.1112",
    ];
    let team = archive
        .save_team(
            None,
            "召唤、飞黄与奥迹",
            &[jing_yuan, feixiao, black_swan, topaz].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    for source in [jing_yuan, topaz] {
        assert!(hits().any(|hit| hit.source_id == source && hit.kind == DamageKind::FollowUp));
    }
    assert!(hits().any(|hit| {
        hit.source_id == feixiao && hit.action == Action::Ultimate && hit.kind == DamageKind::Direct
    }));
    assert!(
        hits().any(|hit| hit.source_id == black_swan && hit.kind == DamageKind::DamageOverTime)
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
fn black_swan_preserves_arcana_sources_with_both_saved_variants() {
    use star_rail_battle_core::DamageKind;
    use star_rail_profiles::Catalog;

    let dir = TestData::new();
    let black_swan = "character.1307";
    let catalog = Catalog::from_json(include_str!("../../.m03-data/4.7.0/catalog.json")).unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let forms = [
        black_swan,
        "character.1005",
        "character.1108",
        "character.1210",
    ];
    let team = archive
        .save_team(None, "奥迹与引爆", &forms.map(|id| Some(id.into())))
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        let result = checked.result.unwrap();
        assert!(
            result
                .targets
                .iter()
                .flat_map(|target| &target.hits)
                .any(|hit| {
                    hit.source_id == black_swan && hit.kind == DamageKind::DamageOverTime
                })
        );
        assert_eq!(
            result.total_damage,
            result
                .characters
                .iter()
                .map(|actor| actor.own_damage + actor.assist_damage)
                .sum::<u64>()
        );
        result
    };
    let enhanced = run();
    let base = catalog
        .default_loadout_at(black_swan, "base", 80, 6)
        .unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog).unwrap();
    let profile = profiles.profile(black_swan).unwrap();
    assert_eq!(
        profile.presets[&profile.current_preset].variant_id,
        "novaflare"
    );
    profiles
        .save_preset(black_swan, "原版", &base, true)
        .unwrap();
    let original = run();
    assert_ne!(enhanced.total_damage, original.total_damage);
    assert_eq!(original, run());
}

#[cfg(bundled_catalog)]
#[test]
fn saved_quantum_team_credits_matrix_and_defense_debuffs() {
    let dir = TestData::new();
    let catalog =
        star_rail_profiles::Catalog::from_json(include_str!("../../.m03-data/4.7.0/catalog.json"))
            .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let [seele, silver_wolf, fu_xuan, arlan] = [
        "character.1102",
        "character.1006",
        "character.1208",
        "character.1008",
    ];
    let mut edit = archive.profile(fu_xuan).unwrap().edit;
    edit.eidolon = 1;
    archive.save(fu_xuan, edit).unwrap();
    let team = archive
        .save_team(
            None,
            "量子配队",
            &[seele, silver_wolf, fu_xuan, arlan].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    assert!(result.characters[0].own_damage > 0);
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    for support in [silver_wolf, fu_xuan] {
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
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|actor| actor.own_damage + actor.assist_damage)
            .sum::<u64>()
    );
    assert_eq!(result, run());

    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    for form in [seele, silver_wolf] {
        let profile = profiles.profile(form).unwrap();
        assert_eq!(
            profile.presets[&profile.current_preset].variant_id,
            "novaflare"
        );
        let loadout = catalog.default_loadout_at(form, "base", 80, 6).unwrap();
        profiles.save_preset(form, "原版", &loadout, true).unwrap();
    }
    let original = run();
    assert_ne!(result.total_damage, original.total_damage);
    assert!(original.characters[0].own_damage > 0);
    assert_eq!(original, run());
}

#[cfg(bundled_catalog)]
#[test]
fn saved_team_replays_slashed_dream_and_sparkles_assists() {
    use star_rail_battle_core::{Action, DamageKind};

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [acheron, aventurine, sparkle, silver_wolf] = [
        "character.1308",
        "character.1304",
        "character.1306",
        "character.1006",
    ];
    let mut edit = archive.profile(aventurine).unwrap().edit;
    edit.eidolon = 2;
    archive.save(aventurine, edit).unwrap();
    let team = archive
        .save_team(
            None,
            "残梦与护盾支援",
            &[acheron, aventurine, sparkle, silver_wolf].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| {
        hit.source_id == acheron && hit.action == Action::Ultimate && hit.kind == DamageKind::Direct
    }));
    assert!(hits().any(|hit| {
        hit.source_id == acheron
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == sparkle && credit.amount > 0)
    }));
    assert!(hits().any(|hit| {
        hit.source_id == acheron
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == aventurine && credit.amount > 0)
    }));
    assert!(hits().any(|hit| hit.source_id == aventurine && hit.amount > 0));
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
fn saved_team_replays_tile_attacks_with_energy_and_skill_point_support() {
    use star_rail_battle_core::{Action, DamageKind};

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [qingque, sparkle, huohuo, argenti] = [
        "character.1201",
        "character.1306",
        "character.1217",
        "character.1302",
    ];
    for (actor, eidolon) in [(qingque, 4), (sparkle, 4)] {
        let mut edit = archive.profile(actor).unwrap().edit;
        edit.eidolon = eidolon;
        archive.save(actor, edit).unwrap();
    }
    let team = archive
        .save_team(
            None,
            "琼玉牌与回能支援",
            &[qingque, sparkle, huohuo, argenti].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == qingque && hit.kind == DamageKind::FollowUp));
    assert!(hits().any(|hit| hit.source_id == argenti && hit.action == Action::EnhancedUltimate));
    for support in [sparkle, huohuo] {
        assert!(hits().any(|hit| {
            [qingque, argenti].contains(&hit.source_id.as_str())
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == support && credit.amount > 0)
        }));
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
fn saved_team_replays_lightning_lord_with_three_harmony_supports() {
    use star_rail_battle_core::DamageKind;

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [jing_yuan, ruan_mei, robin, sunday] = [
        "character.1204",
        "character.1303",
        "character.1309",
        "character.1313",
    ];
    let team = archive
        .save_team(
            None,
            "协奏与神君",
            &[jing_yuan, ruan_mei, robin, sunday].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| {
        hit.source_id == jing_yuan && hit.kind == DamageKind::FollowUp && hit.amount > 0
    }));
    assert!(hits().any(|hit| hit.source_id == robin && hit.kind == DamageKind::Additional));
    for support in [ruan_mei, robin, sunday] {
        assert!(hits().any(|hit| {
            hit.source_id == jing_yuan
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == support && credit.amount > 0)
        }));
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
fn saved_team_replays_hp_consumption_and_boothills_breaks() {
    use star_rail_battle_core::DamageKind;

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [blade, jingliu, boothill, ruan_mei] = [
        "character.1205",
        "character.1212",
        "character.1315",
        "character.1303",
    ];
    let mut edit = archive.profile(blade).unwrap().edit;
    edit.eidolon = 6;
    archive.save(blade, edit).unwrap();
    let team = archive
        .save_team(
            None,
            "生命消耗与弱点击破",
            &[blade, jingliu, boothill, ruan_mei].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == blade && hit.kind == DamageKind::FollowUp));
    assert!(hits().any(|hit| hit.source_id == jingliu && hit.kind == DamageKind::Direct));
    assert!(hits().any(|hit| hit.source_id == boothill && hit.kind == DamageKind::WeaknessBreak));
    assert!(hits().any(|hit| {
        [blade, jingliu, boothill].contains(&hit.source_id.as_str())
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == ruan_mei && credit.amount > 0)
    }));
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
fn saved_team_replays_super_break_and_lingshas_summon() {
    use star_rail_battle_core::{Action, DamageKind};

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [imbibitor_lunae, trailblazer, lingsha, sparkle] = [
        "character.1213",
        "trailblazer.harmony",
        "character.1222",
        "character.1306",
    ];
    let team = archive
        .save_team(
            None,
            "伴舞与浮元",
            &[imbibitor_lunae, trailblazer, lingsha, sparkle].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| {
        hit.source_id == imbibitor_lunae
            && hit.kind == DamageKind::Direct
            && hit.action == Action::Basic
    }));
    assert!(hits().any(|hit| hit.source_id == lingsha && hit.kind == DamageKind::FollowUp));
    assert!(hits().any(|hit| {
        hit.kind == DamageKind::SuperBreak
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == trailblazer && credit.amount > 0)
    }));
    assert!(hits().any(|hit| {
        hit.kind == DamageKind::SuperBreak
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == lingsha && credit.amount > 0)
    }));
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
fn saved_team_tracks_super_break_providers_alongside_jades_follow_up() {
    use star_rail_battle_core::DamageKind;

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [firefly, fugue, trailblazer, jade] = [
        "character.1310",
        "character.1225",
        "trailblazer.harmony",
        "character.1314",
    ];
    let team = archive
        .save_team(
            None,
            "云火昭与伴舞",
            &[firefly, fugue, trailblazer, jade].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == jade && hit.kind == DamageKind::FollowUp));
    assert!(hits().any(|hit| hit.source_id == firefly && hit.kind == DamageKind::SuperBreak));
    for provider in [fugue, trailblazer] {
        assert!(hits().any(|hit| {
            hit.kind == DamageKind::SuperBreak
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == provider && credit.amount > 0)
        }));
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
fn saved_team_replays_parry_counters_and_erudition_damage() {
    use star_rail_battle_core::{Action, DamageKind};

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [yunli, rappa, the_herta, fugue] = [
        "character.1221",
        "character.1317",
        "character.1401",
        "character.1225",
    ];
    let team = archive
        .save_team(
            None,
            "格挡与智识连携",
            &[yunli, rappa, the_herta, fugue].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| {
        hit.source_id == yunli && hit.action == Action::Ultimate && hit.kind == DamageKind::FollowUp
    }));
    assert!(hits().any(|hit| hit.source_id == rappa && hit.kind == DamageKind::SuperBreak));
    assert!(result.targets.iter().all(|target| {
        target
            .hits
            .iter()
            .any(|hit| hit.source_id == the_herta && hit.kind == DamageKind::Direct)
    }));
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
fn saved_team_replays_memosprite_and_true_damage_providers() {
    use star_rail_battle_core::DamageKind;

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [trailblazer, tribbie, anaxa, the_herta] = [
        "trailblazer.remembrance",
        "character.1403",
        "character.1405",
        "character.1401",
    ];
    let mut edit = archive.profile(tribbie).unwrap().edit;
    edit.eidolon = 1;
    archive.save(tribbie, edit).unwrap();
    let team = archive
        .save_team(
            None,
            "忆灵与同谐结界",
            &[trailblazer, tribbie, anaxa, the_herta].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert_eq!(result.targets.len(), 3);
    assert_eq!(result.characters.len(), 4);
    let mem = format!("{trailblazer}:mem");
    assert!(hits().any(|hit| hit.source_id == mem && hit.amount > 0));
    assert!(hits().any(|hit| hit.source_id == anaxa && hit.kind == DamageKind::Direct));
    for provider in [trailblazer, tribbie] {
        assert!(hits().any(|hit| {
            hit.kind == DamageKind::TrueDamage
                && (hit.source_id == provider
                    || hit
                        .credits
                        .iter()
                        .any(|credit| credit.unit_id == provider && credit.amount > 0))
        }));
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
fn saved_team_replays_two_memosprites_vendetta_and_recorded_damage() {
    use star_rail_battle_core::DamageKind;

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [aglaea, mydei, cipher, trailblazer] = [
        "character.1402",
        "character.1404",
        "character.1406",
        "trailblazer.remembrance",
    ];
    for (id, eidolon) in [(aglaea, 1), (mydei, 6)] {
        let mut edit = archive.profile(id).unwrap().edit;
        edit.eidolon = eidolon;
        archive.save(id, edit).unwrap();
    }
    let team = archive
        .save_team(
            None,
            "衣匠、迷迷与逐击真伤",
            &[aglaea, mydei, cipher, trailblazer].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert_eq!(result.targets.len(), 3);
    assert_eq!(result.characters.len(), 4);
    for sprite in ["character.1402:garmentmaker", "trailblazer.remembrance:mem"] {
        assert!(hits().any(|hit| hit.source_id == sprite && hit.amount > 0));
    }
    assert!(hits().any(|hit| {
        hit.source_id == mydei
            && hit.action == star_rail_battle_core::Action::Skill
            && hit.kind == DamageKind::Direct
    }));
    assert!(hits().any(|hit| hit.source_id == cipher && hit.kind == DamageKind::TrueDamage));
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|character| character.own_damage + character.assist_damage)
            .sum::<u64>()
    );
    assert_eq!(result, run());
}

#[cfg(bundled_catalog)]
#[test]
fn saved_team_replays_netherwing_and_little_ica_with_hp_consumption() {
    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [castorice, mydei, hyacine, lingsha] = [
        "character.1407",
        "character.1404",
        "character.1409",
        "character.1222",
    ];
    for (id, eidolon, cone) in [
        (castorice, 2, "cone.23040"),
        (mydei, 6, "cone.23039"),
        (hyacine, 1, "cone.23042"),
        (lingsha, 0, "cone.23013"),
    ] {
        let mut edit = archive.profile(id).unwrap().edit;
        edit.eidolon = eidolon;
        edit.light_cone = Some(LightConeEquipment::Custom(
            star_rail_battle_core::LightCone {
                data_id: cone.into(),
                level: 80,
                ascension: 6,
                superimposition: 1,
            },
        ));
        archive.save(id, edit).unwrap();
    }
    let team = archive
        .save_team(
            None,
            "死龙、小伊卡与失血",
            &[castorice, mydei, hyacine, lingsha].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert_eq!(result.targets.len(), 3);
    assert_eq!(result.characters.len(), 4);
    assert!(hits().any(|hit| hit.source_id == "character.1407:netherwing" && hit.amount > 0));
    assert!(hits().any(|hit| hit.source_id == "character.1409:little-ica" && hit.amount > 0));
    assert!(hits().any(|hit| hit.source_id == mydei && hit.amount > 0));
    assert!(hits().any(|hit| {
        hit.source_id == "character.1407:netherwing"
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == hyacine && credit.amount > 0)
    }));
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|character| character.own_damage + character.assist_damage)
            .sum::<u64>()
    );
    assert_eq!(result, run());
}

#[cfg(bundled_catalog)]
#[test]
fn saved_cerydra_gives_low_level_peerage_to_anaxa() {
    use star_rail_battle_core::{Action, DamageKind};

    let dir = TestData::new();
    let catalog = star_rail_profiles::Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let [cerydra, anaxa] = ["character.1412", "character.1405"];
    profiles.ensure_profile(cerydra).unwrap();
    let mut loadout = catalog.default_loadout_at(cerydra, "base", 80, 6).unwrap();
    loadout
        .config
        .skill_levels
        .values_mut()
        .for_each(|level| *level = 2);
    profiles
        .save_preset(cerydra, "低档技能", &loadout, true)
        .unwrap();
    let team = archive
        .save_team(
            None,
            "爵位复制",
            &[Some(cerydra.into()), Some(anaxa.into()), None, None],
        )
        .unwrap();
    let capture = || {
        let readiness = archive.dpr_readiness(&team.id).unwrap();
        assert!(readiness.unsupported_characters.is_empty());
        readiness.result.unwrap()
    };
    let result = capture();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    assert!(hits().any(|hit| hit.source_id == cerydra && hit.kind == DamageKind::Additional));
    assert!(hits().any(|hit| {
        hit.source_id == anaxa
            && hit.action == Action::Skill
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == cerydra && credit.amount == hit.amount)
    }));
    assert_eq!(result, capture());
    assert_eq!(
        result
            .characters
            .iter()
            .map(|unit| unit.own_damage + unit.assist_damage)
            .sum::<u64>(),
        result.total_damage
    );
}

#[cfg(bundled_catalog)]
#[test]
fn saved_hysilens_team_replays_zone_detonations_and_keeps_dot_sources() {
    use star_rail_battle_core::{Action, DamageKind};

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let [hysilens, kafka, black_swan, jiaoqiu] = [
        "character.1410",
        "character.1005",
        "character.1307",
        "character.1218",
    ];
    let mut edit = archive.profile(hysilens).unwrap().edit;
    edit.eidolon = 1;
    archive.save(hysilens, edit).unwrap();
    let team = archive
        .save_team(
            None,
            "海妖结界与持续伤害",
            &[hysilens, kafka, black_swan, jiaoqiu].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let hits = || result.targets.iter().flat_map(|target| &target.hits);
    for source in [hysilens, kafka, black_swan, jiaoqiu] {
        assert!(
            hits().any(|hit| hit.source_id == source && hit.kind == DamageKind::DamageOverTime)
        );
    }
    assert!(hits().any(|hit| {
        hit.source_id == kafka
            && hit.kind == DamageKind::DamageOverTime
            && hit.action == Action::Ultimate
            && hit
                .credits
                .iter()
                .any(|credit| credit.unit_id == hysilens && credit.amount > 0)
    }));
    assert_eq!(result, run());
    assert_eq!(
        result.total_damage,
        result
            .characters
            .iter()
            .map(|character| character.own_damage + character.assist_damage)
            .sum::<u64>()
    );
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

#[cfg(bundled_catalog)]
#[test]
fn saved_anaxa_team_replays_extra_skills_and_credits_erudition_bonus() {
    use star_rail_battle_core::{Action, DamageKind};

    let dir = TestData::new();
    let mut archive = CharacterArchive::installed(dir.path()).unwrap();
    let anaxa = "character.1405";
    let argenti = "character.1302";
    let team = archive
        .save_team(
            None,
            "那刻夏和银枝",
            &[Some(anaxa.into()), Some(argenti.into()), None, None],
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let result = run();
    let mut skill_hits = std::collections::BTreeMap::new();
    for hit in result.targets.iter().flat_map(|target| &target.hits) {
        if hit.source_id == anaxa && hit.action == Action::Skill && hit.kind == DamageKind::Direct {
            *skill_hits.entry(hit.av_micros).or_insert(0) += 1;
        }
    }
    assert!(skill_hits.values().any(|&count| count == 10));
    assert!(
        result
            .targets
            .iter()
            .flat_map(|target| &target.hits)
            .any(|hit| {
                hit.source_id == argenti
                    && hit
                        .credits
                        .iter()
                        .any(|credit| credit.unit_id == anaxa && credit.amount > 0)
            })
    );
    assert_eq!(result, run());
}

#[cfg(bundled_catalog)]
#[test]
fn saved_cyrene_runs_with_every_frozen_heir_and_preserves_damage_credit() {
    use star_rail_battle_core::DamageKind;

    let dir = TestData::new();
    let catalog = star_rail_profiles::Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let cyrene = "character.1415";
    profiles.ensure_profile(cyrene).unwrap();
    let mut loadout = catalog.default_loadout_at(cyrene, "base", 80, 6).unwrap();
    loadout.config.eidolon = 6;
    profiles
        .save_preset(cyrene, "base", &loadout, true)
        .unwrap();
    let heirs = [
        "trailblazer.remembrance",
        "character.1402",
        "character.1403",
        "character.1404",
        "character.1407",
        "character.1405",
        "character.1409",
        "character.1406",
        "character.1408",
        "character.1410",
        "character.1412",
        "character.1413",
        "character.1414",
    ];
    for heir in heirs {
        let slots = [
            Some(cyrene.into()),
            Some(heir.into()),
            Some("character.1013".into()),
            Some("character.1313".into()),
        ];
        let team = archive.save_team(None, heir, &slots).unwrap();
        let run = || {
            let checked = archive.dpr_readiness(&team.id).unwrap();
            assert!(checked.unsupported_characters.is_empty(), "{heir}");
            checked.result.unwrap()
        };
        let result = run();
        assert_eq!(result, run(), "{heir}");
        let hits = || result.targets.iter().flat_map(|target| &target.hits);
        assert!(
            hits().any(|hit| hit.source_id == "character.1415:demiurge"),
            "{heir}"
        );
        assert!(
            hits().any(|hit| {
                hit.kind == DamageKind::TrueDamage
                    && hit
                        .credits
                        .iter()
                        .any(|credit| credit.unit_id == cyrene && credit.amount > 0)
            }),
            "{heir}"
        );
        assert_eq!(
            result.total_damage,
            result
                .characters
                .iter()
                .map(|role| role.own_damage + role.assist_damage)
                .sum::<u64>(),
            "{heir}"
        );
    }
}
