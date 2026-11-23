use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::Catalog;

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_destruction_trailblazer_replays_frozen_blast_ultimate_and_credits() {
    let dir = TestData::new();
    let catalog = Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let [trailblazer, pela] = ["trailblazer.destruction", "character.1106"];
    let team = archive
        .save_team(
            None,
            "毁灭开拓者与佩拉",
            &[Some(trailblazer.into()), None, Some(pela.into()), None],
        )
        .unwrap();
    let mut loadout = catalog
        .default_loadout_at(trailblazer, "base", 80, 6)
        .unwrap();
    for level in loadout.config.skill_levels.values_mut() {
        *level = 1;
    }
    let run = || {
        let readiness = archive.dpr_readiness(&team.id).unwrap();
        assert!(readiness.unsupported_characters.is_empty());
        readiness.result.unwrap()
    };

    let mut prior_own_damage = 0;
    for (eidolon, ultimate_level) in [(0, 1), (6, 3)] {
        loadout.config.eidolon = eidolon;
        for skill in ["skill.800103", "skill.800108", "skill.800109"] {
            assert_eq!(
                catalog
                    .effective_skill_level(trailblazer, &loadout, skill)
                    .unwrap(),
                ultimate_level
            );
        }
        profiles
            .save_preset(trailblazer, "毁灭开拓者低等级技能", &loadout, true)
            .unwrap();

        let result = run();
        assert_eq!(result, run());
        assert_eq!(result.targets.len(), 3);
        assert!(result.characters[0].own_damage > prior_own_damage);
        prior_own_damage = result.characters[0].own_damage;
        let mut receives_pela_credit = false;
        for target in &result.targets {
            assert!(target.hits.iter().any(|hit| {
                hit.source_id == trailblazer
                    && hit.action == Action::EnhancedUltimate
                    && hit.kind == DamageKind::Direct
            }));
            for hit in target
                .hits
                .iter()
                .filter(|hit| hit.source_id == trailblazer)
            {
                assert_ne!(hit.action, Action::Ultimate);
                assert_eq!(
                    hit.credits.iter().map(|credit| credit.amount).sum::<u32>(),
                    hit.amount
                );
                receives_pela_credit |= hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == pela && credit.amount > 0);
            }
        }
        assert!(receives_pela_credit);
        assert_eq!(
            result.total_damage,
            result
                .characters
                .iter()
                .map(|unit| unit.own_damage + unit.assist_damage)
                .sum::<u64>()
        );
    }
}
