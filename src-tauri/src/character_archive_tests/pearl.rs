use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::Catalog;

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_pearl_replays_four_elation_at_low_levels_and_full_eidolons() {
    let dir = TestData::new();
    let catalog = Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let [pearl, yao, aha, sparxie] = [
        "character.1503",
        "character.1502",
        "character.1511",
        "character.1501",
    ];
    for (id, eidolon) in [(pearl, 0), (yao, 2), (aha, 2), (sparxie, 2)] {
        profiles.ensure_profile(id).unwrap();
        let mut loadout = catalog.default_loadout_at(id, "base", 80, 6).unwrap();
        loadout.config.eidolon = eidolon;
        for level in loadout.config.skill_levels.values_mut() {
            *level = 1;
        }
        profiles
            .save_preset(id, "真珠欢愉队低等级", &loadout, true)
            .unwrap();
    }

    let team = archive
        .save_team(
            None,
            "真珠与三位欢愉角色",
            &[
                Some(pearl.into()),
                Some(yao.into()),
                Some(aha.into()),
                Some(sparxie.into()),
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
            let mut loadout = catalog.default_loadout_at(pearl, "base", 80, 6).unwrap();
            loadout.config.eidolon = eidolon;
            profiles
                .save_preset(pearl, "真珠满魂", &loadout, true)
                .unwrap();
        }
        let result = run();
        assert_eq!(result, run());
        let hits = || result.targets.iter().flat_map(|target| &target.hits);
        assert!(hits().any(|hit| hit.source_id == pearl
            && hit.kind == DamageKind::Direct
            && hit.action == Action::Basic));
        assert!(hits().any(|hit| {
            hit.source_id != pearl
                && hit.kind == DamageKind::Elation
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == pearl && credit.amount > 0)
        }));
        assert_eq!(
            result.total_damage,
            result
                .characters
                .iter()
                .map(|actor| actor.own_damage + actor.assist_damage)
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
