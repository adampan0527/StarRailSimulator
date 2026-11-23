use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::Catalog;

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_robin_sunny_replays_frozen_bird_levels_and_fever_credit() {
    let dir = TestData::new();
    let catalog = Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let form = "character.1512";
    let sprite = "character.1512:summer_songbirds";
    let allies = ["character.1106", "character.1203", "character.1221"];
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    for ally in allies {
        profiles.ensure_profile(ally).unwrap();
    }
    let team = archive
        .save_team(
            None,
            "晴歌与队友",
            &[
                Some(form.into()),
                Some(allies[0].into()),
                Some(allies[1].into()),
                Some(allies[2].into()),
            ],
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(
            checked.unsupported_characters.is_empty(),
            "{:?}",
            checked.unsupported_characters
        );
        checked.result.unwrap()
    };

    for (eidolon, level) in [(0, 1), (6, 6)] {
        let mut loadout = catalog.default_loadout_at(form, "base", 80, 6).unwrap();
        loadout.config.eidolon = eidolon;
        for skill in [
            "skill.151201",
            "skill.151202",
            "skill.151203",
            "skill.151204",
            "skill.1151201",
            "skill.1151203",
        ] {
            loadout.config.skill_levels.insert(skill.into(), level);
        }
        profiles
            .save_preset(form, "晴歌技能", &loadout, true)
            .unwrap();
        let result = run();
        assert_eq!(result, run());
        let hits = || result.targets.iter().flat_map(|target| &target.hits);
        assert!(hits().any(|hit| {
            hit.source_id == sprite
                && hit.action == Action::Skill
                && hit.kind == DamageKind::Direct
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == form && credit.amount > 0)
        }));
        assert_eq!(
            hits().any(|hit| hit.source_id == sprite && hit.kind == DamageKind::TrueDamage),
            eidolon == 6
        );
        assert!(hits().any(|hit| {
            allies.contains(&hit.source_id.as_str())
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == form && credit.amount > 0)
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
