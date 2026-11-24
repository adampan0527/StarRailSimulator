use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::Catalog;

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_evanescia_replays_frozen_levels_with_fox_and_ascension() {
    let dir = TestData::new();
    let catalog = Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let [evanescia, yao, aha, trailblazer] = [
        "character.1505",
        "character.1502",
        "character.1511",
        "trailblazer.elation",
    ];
    for (id, eidolon) in [(evanescia, 0), (yao, 0), (aha, 2), (trailblazer, 0)] {
        profiles.ensure_profile(id).unwrap();
        let mut loadout = catalog.default_loadout_at(id, "base", 80, 6).unwrap();
        loadout.config.eidolon = eidolon;
        for level in loadout.config.skill_levels.values_mut() {
            *level = 1;
        }
        profiles
            .save_preset(id, "绯英欢愉队低等级", &loadout, true)
            .unwrap();
    }
    let team = archive
        .save_team(
            None,
            "绯英欢愉队",
            &[
                Some(evanescia.into()),
                Some(yao.into()),
                Some(aha.into()),
                Some(trailblazer.into()),
            ],
        )
        .unwrap();
    let run = || {
        let readiness = archive.dpr_readiness(&team.id).unwrap();
        assert!(readiness.unsupported_characters.is_empty());
        readiness.result.unwrap()
    };
    for eidolon in [0, 6] {
        if eidolon == 6 {
            let mut loadout = catalog
                .default_loadout_at(evanescia, "base", 80, 6)
                .unwrap();
            loadout.config.eidolon = eidolon;
            profiles
                .save_preset(evanescia, "绯英满魂", &loadout, true)
                .unwrap();
        }
        let result = run();
        assert_eq!(result, run());
        let hits = || result.targets.iter().flat_map(|target| &target.hits);
        for (action, kind) in [
            (Action::Skill, DamageKind::Direct),
            (Action::Ultimate, DamageKind::Direct),
            (Action::Ultimate, DamageKind::Elation),
            (Action::ElationSkill, DamageKind::Elation),
        ] {
            assert!(
                hits().any(|hit| hit.source_id == evanescia
                    && hit.action == action
                    && hit.kind == kind),
                "{eidolon}: {action:?} {kind:?}"
            );
        }
        assert!(hits().any(|hit| hit.source_id == evanescia && hit.kind == DamageKind::FollowUp));
        assert!(hits().any(|hit| {
            hit.source_id == evanescia
                && hit.kind == DamageKind::Elation
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id != evanescia && credit.amount > 0)
        }));
        assert_eq!(
            result.total_damage,
            result
                .characters
                .iter()
                .map(|unit| unit.own_damage + unit.assist_damage)
                .sum::<u64>()
        );
        for hit in hits() {
            assert_eq!(
                u64::from(hit.amount),
                hit.credits
                    .iter()
                    .map(|credit| u64::from(credit.amount))
                    .sum::<u64>()
            );
        }
    }
}
