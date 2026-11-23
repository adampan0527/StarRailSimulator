use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::Catalog;

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_gilgamesh_replays_low_levels_and_e6_with_saber() {
    const GILGAMESH: &str = "character.1509";
    const SABER: &str = "character.1014";
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
            "吉尔伽美什与 Saber",
            &[
                Some(GILGAMESH.into()),
                Some(SABER.into()),
                Some("character.1106".into()),
                Some("character.1202".into()),
            ],
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    for (eidolon, basic, other) in [(0, 1, 1), (6, 6, 10)] {
        let mut loadout = catalog
            .default_loadout_at(GILGAMESH, "base", 80, 6)
            .unwrap();
        loadout.config.eidolon = eidolon;
        loadout
            .config
            .skill_levels
            .insert("skill.150901".into(), basic);
        for skill in ["skill.150902", "skill.150903", "skill.150904"] {
            loadout.config.skill_levels.insert(skill.into(), other);
        }
        for trace in ["trace.1509101", "trace.1509102", "trace.1509103"] {
            if !loadout
                .config
                .unlocked_trace_ids
                .iter()
                .any(|id| id == trace)
            {
                loadout.config.unlocked_trace_ids.push(trace.into());
            }
        }
        for (id, level) in [
            ("skill.150909", basic + u16::from(eidolon == 6)),
            ("skill.150905", other + 2 * u16::from(eidolon == 6)),
        ] {
            assert_eq!(
                catalog
                    .effective_skill_level(GILGAMESH, &loadout, id)
                    .unwrap(),
                level
            );
        }
        profiles
            .save_preset(GILGAMESH, "吉尔伽美什技能", &loadout, true)
            .unwrap();

        let result = run();
        assert_eq!(result, run());
        let hits = || result.targets.iter().flat_map(|target| &target.hits);
        assert!(hits().any(|hit| {
            hit.source_id == GILGAMESH
                && hit.action == Action::Skill
                && hit.kind == DamageKind::Direct
        }));
        assert!(hits().any(|hit| {
            hit.source_id == SABER
                && hit.action == Action::Ultimate
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == GILGAMESH && credit.amount > 0)
        }));
        for target in &result.targets {
            assert!(target.hits.iter().any(|first| {
                first.source_id == GILGAMESH
                    && first.kind == DamageKind::FollowUp
                    && target.hits.iter().any(|second| {
                        second.source_id == SABER
                            && second.kind == DamageKind::FollowUp
                            && second.av_micros == first.av_micros
                    })
            }));
        }
        assert!(
            hits().all(
                |hit| hit.amount == hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
            )
        );
    }
}
