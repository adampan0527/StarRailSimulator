use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::Catalog;

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_elation_trailblazer_replays_low_levels_and_ascended_elation() {
    let dir = TestData::new();
    let catalog = Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let [trailblazer, aha, sparxie] = ["trailblazer.elation", "character.1511", "character.1501"];
    for (id, eidolon) in [(trailblazer, 0), (aha, 2), (sparxie, 2)] {
        profiles.ensure_profile(id).unwrap();
        let mut loadout = catalog.default_loadout_at(id, "base", 80, 6).unwrap();
        loadout.config.eidolon = eidolon;
        for level in loadout.config.skill_levels.values_mut() {
            *level = 1;
        }
        profiles
            .save_preset(id, "欢愉联动低等级", &loadout, true)
            .unwrap();
    }
    let team = archive
        .save_team(
            None,
            "欢愉开拓者与升格阿哈",
            &[
                Some(trailblazer.into()),
                Some(aha.into()),
                Some(sparxie.into()),
                None,
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
                .default_loadout_at(trailblazer, "base", 80, 6)
                .unwrap();
            loadout.config.eidolon = eidolon;
            profiles
                .save_preset(trailblazer, "欢愉开拓者满魂", &loadout, true)
                .unwrap();
        }
        let result = run();
        assert_eq!(result, run());
        let hits = || result.targets.iter().flat_map(|target| &target.hits);
        for (kind, action) in [
            (DamageKind::Direct, Action::Skill),
            (DamageKind::Elation, Action::Skill),
            (DamageKind::Elation, Action::ElationSkill),
        ] {
            assert!(hits().any(|hit| hit.source_id == trailblazer
                && hit.kind == kind
                && hit.action == action));
        }
        assert!(hits().any(|hit| {
            hit.source_id == aha
                && hit.kind == DamageKind::TrueDamage
                && hit.action == Action::ElationSkill
        }));
        assert!(hits().any(|hit| {
            hit.source_id != trailblazer
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == trailblazer && credit.amount > 0)
        }));
        assert_eq!(
            result.total_damage,
            result
                .characters
                .iter()
                .map(|actor| actor.own_damage + actor.assist_damage)
                .sum::<u64>()
        );
    }
}
