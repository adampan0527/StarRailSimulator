use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::Catalog;

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_silver_wolf_999_replays_frozen_levels_transform_and_assist_credit() {
    let dir = TestData::new();
    let catalog = Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let [wolf, yao, aha, pearl] = [
        "character.1506",
        "character.1502",
        "character.1511",
        "character.1503",
    ];
    for (id, eidolon) in [(wolf, 0), (yao, 2), (aha, 2), (pearl, 2)] {
        profiles.ensure_profile(id).unwrap();
        let mut loadout = catalog.default_loadout_at(id, "base", 80, 6).unwrap();
        loadout.config.eidolon = eidolon;
        for level in loadout.config.skill_levels.values_mut() {
            *level = 1;
        }
        profiles
            .save_preset(id, "低技能等级欢愉队", &loadout, true)
            .unwrap();
    }
    let team = archive
        .save_team(
            None,
            "银狼LV.999欢愉队",
            &[
                Some(wolf.into()),
                Some(yao.into()),
                Some(aha.into()),
                Some(pearl.into()),
            ],
        )
        .unwrap();
    let run = || {
        let readiness = archive.dpr_readiness(&team.id).unwrap();
        assert!(readiness.unsupported_characters.is_empty());
        readiness.result.unwrap()
    };
    let low_level = run();
    assert_eq!(low_level, run());
    let mut max_loadout = catalog.default_loadout_at(wolf, "base", 80, 6).unwrap();
    max_loadout.config.eidolon = 6;
    profiles
        .save_preset(wolf, "满魂银狼LV.999", &max_loadout, true)
        .unwrap();
    let full_eidolons = run();
    assert_eq!(full_eidolons, run());
    for report in [&low_level, &full_eidolons] {
        let hits = || report.targets.iter().flat_map(|target| &target.hits);
        assert!(
            hits()
                .filter(|hit| hit.source_id == wolf
                    && hit.kind == DamageKind::Elation
                    && hit.action == Action::Basic)
                .count()
                >= 100
        );
        assert!(hits().any(|hit| hit.source_id == wolf
            && hit.kind == DamageKind::Elation
            && hit.action == Action::ElationSkill));
        assert!(hits().any(|hit| {
            hit.source_id == wolf
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id != wolf && credit.amount > 0)
        }));
        for hit in hits() {
            assert_eq!(
                u64::from(hit.amount),
                hit.credits
                    .iter()
                    .map(|credit| u64::from(credit.amount))
                    .sum::<u64>()
            );
        }
        assert_eq!(
            report.total_damage,
            report
                .characters
                .iter()
                .map(|actor| actor.own_damage + actor.assist_damage)
                .sum::<u64>()
        );
    }
    assert!(
        full_eidolons
            .characters
            .iter()
            .any(|actor| actor.unit_id == wolf && actor.assist_damage > 0)
    );
}
