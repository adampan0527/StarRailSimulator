use std::collections::BTreeMap;

use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::Catalog;

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_archer_uses_frozen_skill_levels_and_credits_follow_ups() {
    let dir = TestData::new();
    let catalog = Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let archer = "character.1015";
    let sparkle = "character.1306";
    let herta = "character.1013";
    let team = archive
        .save_team(
            None,
            "Archer 固定靶场",
            &[
                Some(archer.into()),
                Some(sparkle.into()),
                Some(herta.into()),
                None,
            ],
        )
        .unwrap();
    let mut loadout = catalog.default_loadout_at(archer, "base", 80, 6).unwrap();
    let run = || {
        let readiness = archive.dpr_readiness(&team.id).unwrap();
        assert!(readiness.unsupported_characters.is_empty());
        readiness.result.unwrap()
    };
    let mut previous_damage = 0;
    for (eidolon, basic_level, skill_level) in [(0, 1, 1), (0, 6, 10), (6, 6, 10)] {
        loadout.config.eidolon = eidolon;
        loadout
            .config
            .skill_levels
            .insert("skill.101501".into(), basic_level);
        for id in ["skill.101502", "skill.101503", "skill.101504"] {
            loadout.config.skill_levels.insert(id.into(), skill_level);
        }
        profiles
            .save_preset(archer, "Archer 技能", &loadout, true)
            .unwrap();
        let result = run();
        assert_eq!(result, run());
        assert!(result.characters[0].own_damage > previous_damage);
        previous_damage = result.characters[0].own_damage;
        let mut projections_by_av = BTreeMap::new();
        let mut has_follow_up = false;
        let mut has_ultimate = false;
        for hit in result.targets.iter().flat_map(|target| &target.hits) {
            if hit.source_id != archer {
                continue;
            }
            match (hit.action, hit.kind) {
                (Action::Skill, DamageKind::Direct) => {
                    *projections_by_av.entry(hit.av_micros).or_insert(0) += 1;
                }
                (Action::Ultimate, DamageKind::Direct) => has_ultimate = true,
                (_, DamageKind::FollowUp) => has_follow_up = true,
                _ => {}
            }
        }
        assert!(has_follow_up);
        assert!(has_ultimate);
        assert!(projections_by_av.values().any(|&count| count >= 2));
        assert!(projections_by_av.values().all(|&count| count <= 5));
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
