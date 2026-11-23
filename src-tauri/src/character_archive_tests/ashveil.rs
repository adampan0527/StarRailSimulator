use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::Catalog;

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_ashveil_replays_frozen_bait_and_follow_ups() {
    const ASHVEIL: &str = "character.1504";
    const FEIXIAO: &str = "character.1220";
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
            "不死途追击配队",
            &[
                Some(ASHVEIL.into()),
                Some(FEIXIAO.into()),
                Some("character.1110".into()),
                Some("character.1202".into()),
            ],
        )
        .unwrap();

    let replay = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    for (eidolon, basic, other) in [(0, 1, 1), (6, 6, 10)] {
        let mut loadout = catalog.default_loadout_at(ASHVEIL, "base", 80, 6).unwrap();
        loadout.config.eidolon = eidolon;
        loadout
            .config
            .skill_levels
            .insert("skill.150401".into(), basic);
        for skill in ["skill.150402", "skill.150403", "skill.150404"] {
            loadout.config.skill_levels.insert(skill.into(), other);
        }
        for trace in ["trace.1504101", "trace.1504102", "trace.1504103"] {
            if !loadout
                .config
                .unlocked_trace_ids
                .iter()
                .any(|id| id == trace)
            {
                loadout.config.unlocked_trace_ids.push(trace.into());
            }
        }
        assert_eq!(
            catalog
                .effective_skill_level(ASHVEIL, &loadout, "skill.150401")
                .unwrap(),
            basic + u16::from(eidolon == 6),
        );
        for skill in ["skill.150402", "skill.150403", "skill.150404"] {
            assert_eq!(
                catalog
                    .effective_skill_level(ASHVEIL, &loadout, skill)
                    .unwrap(),
                other + 2 * u16::from(eidolon == 6),
            );
        }
        profiles
            .save_preset(ASHVEIL, "不死途档案", &loadout, true)
            .unwrap();

        let result = replay();
        assert_eq!(result, replay());
        let hits = || result.targets.iter().flat_map(|target| &target.hits);
        assert!(hits().any(|hit| {
            hit.source_id == ASHVEIL
                && hit.action == Action::Skill
                && hit.kind == DamageKind::Direct
        }));
        assert!(hits().any(|hit| {
            hit.source_id == ASHVEIL
                && hit.action != Action::Ultimate
                && hit.kind == DamageKind::FollowUp
        }));
        assert!(hits().any(|hit| {
            hit.source_id == ASHVEIL
                && hit.action == Action::Ultimate
                && hit.kind == DamageKind::FollowUp
        }));
        assert!(hits().any(|hit| {
            hit.source_id == FEIXIAO
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == ASHVEIL && credit.amount > 0)
        }));
        assert!(
            hits().all(
                |hit| hit.amount == hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
            )
        );
    }
}
