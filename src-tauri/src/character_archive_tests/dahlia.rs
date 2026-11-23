use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::Catalog;

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_dahlia_replays_frozen_base_and_e6_with_the_break_team() {
    let dir = TestData::new();
    let catalog = Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let [dahlia, firefly, fugue, trailblazer] = [
        "character.1321",
        "character.1310",
        "character.1225",
        "trailblazer.harmony",
    ];
    for id in [dahlia, firefly, fugue, trailblazer] {
        profiles.ensure_profile(id).unwrap();
    }
    let team = archive
        .save_team(
            None,
            "大丽花与击破队",
            &[dahlia, firefly, fugue, trailblazer].map(|id| Some(id.into())),
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };

    let mut baseline = 0;
    for (eidolon, level) in [(0, 7), (6, 10)] {
        let mut loadout = catalog.default_loadout_at(dahlia, "base", 80, 6).unwrap();
        loadout.config.eidolon = eidolon;
        loadout.config.skill_levels.insert("skill.132101".into(), 6);
        for id in ["skill.132102", "skill.132103", "skill.132104"] {
            loadout.config.skill_levels.insert(id.into(), level);
        }
        profiles
            .save_preset(dahlia, "大丽花技能", &loadout, true)
            .unwrap();

        let result = run();
        assert_eq!(result, run());
        assert!(result.total_damage > baseline);
        baseline = result.total_damage;
        let hits = || result.targets.iter().flat_map(|target| &target.hits);
        for action in [Action::Skill, Action::Ultimate] {
            assert!(hits().any(|hit| {
                hit.source_id == dahlia && hit.kind == DamageKind::Direct && hit.action == action
            }));
        }
        let mut bounces = std::collections::BTreeMap::new();
        for hit in hits().filter(|hit| hit.source_id == dahlia && hit.kind == DamageKind::FollowUp)
        {
            *bounces.entry(hit.av_micros).or_insert(0) += 1;
        }
        assert!(
            bounces
                .values()
                .any(|&count| count == if eidolon == 0 { 5 } else { 10 })
        );
        assert!(hits().any(|hit| hit.source_id == dahlia && hit.kind == DamageKind::SuperBreak));
        assert!(hits().any(|hit| {
            hit.source_id == firefly
                && hit.kind == DamageKind::SuperBreak
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == dahlia && credit.amount > 0)
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
