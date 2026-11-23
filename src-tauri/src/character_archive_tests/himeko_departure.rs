use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::Catalog;

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_himeko_departure_replays_starblazer_at_low_levels_and_e6() {
    const HIMEKO: &str = "character.1510";
    const DAN_HENG: &str = "dan_heng.hunt";
    let dir = TestData::new();
    let catalog = Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    let team = archive
        .save_team(
            None,
            "拓星者回放",
            &[
                Some(HIMEKO.into()),
                Some(DAN_HENG.into()),
                Some("character.1004".into()),
                Some("character.1202".into()),
            ],
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    let mut results = Vec::new();
    for (eidolon, basic, other) in [(0, 1, 1), (6, 6, 10)] {
        let mut loadout = catalog.default_loadout_at(HIMEKO, "base", 80, 6).unwrap();
        loadout.config.eidolon = eidolon;
        loadout
            .config
            .skill_levels
            .insert("skill.151001".into(), basic);
        for skill in ["skill.151002", "skill.151003", "skill.151004"] {
            loadout.config.skill_levels.insert(skill.into(), other);
        }
        for trace in ["trace.1510101", "trace.1510102", "trace.1510103"] {
            if !loadout
                .config
                .unlocked_trace_ids
                .iter()
                .any(|id| id == trace)
            {
                loadout.config.unlocked_trace_ids.push(trace.into());
            }
        }
        for skill in [
            "skill.151008",
            "skill.151009",
            "skill.151014",
            "skill.151022",
            "skill.151025",
            "skill.151026",
        ] {
            assert_eq!(
                catalog
                    .effective_skill_level(HIMEKO, &loadout, skill)
                    .unwrap(),
                other + 2 * u16::from(eidolon == 6),
            );
        }
        profiles
            .save_preset(HIMEKO, "拓星者技能", &loadout, true)
            .unwrap();
        let result = run();
        assert_eq!(result, run());
        let hits = || result.targets.iter().flat_map(|target| &target.hits);
        assert!(hits().any(|hit| hit.source_id == HIMEKO
            && hit.action == Action::Skill
            && hit.kind == DamageKind::Direct));
        assert!(hits().any(|hit| hit.source_id == HIMEKO
            && hit.action == Action::Ultimate
            && hit.kind == DamageKind::Direct));
        assert!(hits().any(|hit| {
            hit.source_id != HIMEKO
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == HIMEKO && credit.amount > 0)
        }));
        assert!(
            hits().all(
                |hit| hit.amount == hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
            )
        );
        results.push(result.total_damage);
    }
    assert!(results[1] > results[0]);
}
