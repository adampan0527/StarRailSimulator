use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::Catalog;

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_saber_runs_fractional_and_maximum_skill_levels() {
    let dir = TestData::new();
    let catalog = Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let saber = "character.1014";
    profiles.ensure_profile(saber).unwrap();
    let team = archive
        .save_team(
            None,
            "Saber 固定靶场",
            &[Some(saber.into()), None, None, None],
        )
        .unwrap();
    let run = || {
        let readiness = archive.dpr_readiness(&team.id).unwrap();
        assert!(readiness.unsupported_characters.is_empty());
        readiness.result.unwrap()
    };
    let mut loadout = catalog.default_loadout_at(saber, "base", 80, 6).unwrap();
    for (id, level) in [
        ("skill.101401", 2),
        ("skill.101402", 7),
        ("skill.101403", 7),
        ("skill.101404", 7),
    ] {
        loadout.config.skill_levels.insert(id.into(), level);
    }
    let mut total_damage = 0;
    for (eidolon, skill_level) in [(0, None), (0, Some(10)), (6, Some(10))] {
        loadout.config.eidolon = eidolon;
        if let Some(level) = skill_level {
            loadout.config.skill_levels.insert("skill.101401".into(), 6);
            for id in ["skill.101402", "skill.101403", "skill.101404"] {
                loadout.config.skill_levels.insert(id.into(), level);
            }
        }
        profiles
            .save_preset(saber, "Saber 技能", &loadout, true)
            .unwrap();
        let result = run();
        assert_eq!(result, run());
        assert!(
            result.total_damage > total_damage,
            "eidolon {eidolon}, level {skill_level:?}"
        );
        assert_eq!(result.total_damage, result.characters[0].own_damage);
        total_damage = result.total_damage;
        let mut ultimate_hits = std::collections::BTreeMap::new();
        let mut enhanced_basic_hits = std::collections::BTreeMap::new();
        for hit in result.targets.iter().flat_map(|target| &target.hits) {
            assert_eq!(hit.source_id, saber);
            if hit.kind == DamageKind::Direct {
                match hit.action {
                    Action::Ultimate => *ultimate_hits.entry(hit.av_micros).or_insert(0) += 1,
                    Action::Basic => *enhanced_basic_hits.entry(hit.av_micros).or_insert(0) += 1,
                    _ => {}
                }
            }
        }
        assert!(ultimate_hits.values().any(|&count| count >= 13));
        assert!(enhanced_basic_hits.values().any(|&count| count >= 3));
    }
}
