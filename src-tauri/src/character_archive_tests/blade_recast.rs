use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::Catalog;

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_blade_recast_replays_frozen_fury_and_support_credit() {
    const BLADE: &str = "character.1507";
    const ASHVEIL: &str = "character.1504";
    const FEIXIAO: &str = "character.1220";
    const PELA: &str = "character.1106";
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
            "千冶·刃追击队",
            &[
                Some(BLADE.into()),
                Some(ASHVEIL.into()),
                Some(FEIXIAO.into()),
                Some(PELA.into()),
            ],
        )
        .unwrap();

    let replay = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };
    for (eidolon, basic, other) in [(0, 1, 1), (6, 6, 10)] {
        let mut loadout = catalog.default_loadout_at(BLADE, "base", 80, 6).unwrap();
        loadout.config.eidolon = eidolon;
        loadout
            .config
            .skill_levels
            .insert("skill.150701".into(), basic);
        for skill in ["skill.150702", "skill.150703", "skill.150704"] {
            loadout.config.skill_levels.insert(skill.into(), other);
        }
        for trace in ["trace.1507101", "trace.1507102", "trace.1507103"] {
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
                .effective_skill_level(BLADE, &loadout, "skill.150708")
                .unwrap(),
            basic + u16::from(eidolon == 6)
        );
        for skill in [
            "skill.150702",
            "skill.150703",
            "skill.150704",
            "skill.150714",
        ] {
            assert_eq!(
                catalog
                    .effective_skill_level(BLADE, &loadout, skill)
                    .unwrap(),
                other + 2 * u16::from(eidolon == 6)
            );
        }
        profiles
            .save_preset(BLADE, "千冶·刃档案", &loadout, true)
            .unwrap();
        let result = replay();
        assert_eq!(result, replay());
        let hits = || result.targets.iter().flat_map(|target| &target.hits);
        for (action, kind) in [
            (Action::Skill, DamageKind::Direct),
            (Action::Skill, DamageKind::FollowUp),
        ] {
            assert!(
                hits()
                    .any(|hit| hit.source_id == BLADE && hit.action == action && hit.kind == kind),
                "E{eidolon} lacked {action:?} {kind:?}. Actual actions: {:?}",
                hits()
                    .filter(|hit| hit.source_id == BLADE)
                    .map(|hit| (hit.action, hit.kind))
                    .collect::<Vec<_>>()
            );
        }
        assert!(hits().any(|hit| {
            hit.source_id != BLADE
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == BLADE && credit.amount > 0)
        }));
        assert!(hits().all(|hit| {
            hit.amount == hit.credits.iter().map(|credit| credit.amount).sum::<u32>()
        }));
        assert_eq!(
            result
                .targets
                .iter()
                .map(|target| target.damage)
                .sum::<u64>(),
            result.total_damage
        );
    }
}
