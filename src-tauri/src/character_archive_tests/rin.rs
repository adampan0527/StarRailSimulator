use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::Catalog;

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_rin_replays_the_frozen_archer_joint_and_skill_credit() {
    let dir = TestData::new();
    let catalog = Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let rin = "character.1508";
    let archer = "character.1015";
    profiles.ensure_profile(archer).unwrap();
    let team = archive
        .save_team(
            None,
            "远坂凛与 Archer",
            &[Some(rin.into()), Some(archer.into()), None, None],
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let mut baseline = 0;
    for (eidolon, basic_level, skill_level) in [(0, 1, 1), (0, 6, 10), (6, 6, 10)] {
        let mut loadout = catalog.default_loadout_at(rin, "base", 80, 6).unwrap();
        loadout.config.eidolon = eidolon;
        loadout
            .config
            .skill_levels
            .insert("skill.150801".into(), basic_level);
        for id in ["skill.150802", "skill.150803", "skill.150804"] {
            loadout.config.skill_levels.insert(id.into(), skill_level);
        }
        profiles
            .save_preset(rin, "远坂凛技能", &loadout, true)
            .unwrap();

        let result = run();
        assert_eq!(result, run());
        assert!(result.characters[0].own_damage > baseline);
        baseline = result.characters[0].own_damage;
        let hits = || result.targets.iter().flat_map(|target| &target.hits);
        for action in [Action::EnhancedSkill, Action::Ultimate] {
            assert!(hits().any(|hit| {
                hit.source_id == rin && hit.action == action && hit.kind == DamageKind::Direct
            }));
        }
        for target in &result.targets {
            assert!(target.hits.iter().any(|first| {
                first.source_id == rin
                    && first.action == Action::Skill
                    && first.kind == DamageKind::FollowUp
                    && target.hits.iter().any(|second| {
                        second.source_id == archer
                            && second.action == Action::Skill
                            && second.kind == DamageKind::FollowUp
                            && second.av_micros == first.av_micros
                    })
            }));
        }
        assert!(hits().any(|hit| {
            hit.source_id == archer
                && hit.action == Action::Skill
                && hit.kind == DamageKind::Direct
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == rin && credit.amount > 0)
        }));
        for hit in hits() {
            assert_eq!(
                hit.amount,
                hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
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
    }
}
