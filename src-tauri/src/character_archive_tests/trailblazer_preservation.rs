use star_rail_battle_core::{Action, DamageKind};
use star_rail_profiles::{Catalog, Relic};

use super::{Archive, CharacterArchive, TestData};

#[test]
fn saved_preservation_trailblazer_replays_frozen_basic_levels_and_eidolons() {
    let dir = TestData::new();
    let catalog = Catalog::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.m03-data/4.7.0/catalog.json"
    )))
    .unwrap();
    let form = "trailblazer.preservation";
    let pela = "character.1106";
    let mut archive = CharacterArchive::open(dir.path(), catalog.clone()).unwrap();
    let mut profiles = Archive::open(dir.path().join("profiles.sqlite"), catalog.clone()).unwrap();
    profiles.ensure_profile(pela).unwrap();
    let team = archive
        .save_team(
            None,
            "存护开拓者与佩拉",
            &[Some(form.into()), Some(pela.into()), None, None],
        )
        .unwrap();
    let run = || {
        let checked = archive.dpr_readiness(&team.id).unwrap();
        assert!(checked.unsupported_characters.is_empty());
        checked.result.unwrap()
    };

    let mut previous_own_damage = 0;
    for (eidolon, basic_level, other_level) in [(0, 1, 1), (0, 6, 10), (6, 6, 10)] {
        let mut loadout = catalog.default_loadout_at(form, "base", 80, 6).unwrap();
        loadout.config.eidolon = eidolon;
        // Speed boots put the initial energy cycle and an ultimate decision inside the DPR window.
        loadout.config.relics.feet = Some(Relic {
            data_id: "relic.61014".into(),
            level: 15,
            main_stat: "speed".into(),
            substats: [
                ("hp_flat".into(), 67_740_080),
                ("attack_flat".into(), 33_870_038),
                ("defense_flat".into(), 33_870_038),
                ("hp_ratio".into(), 69_120),
            ]
            .into(),
        });
        loadout
            .config
            .skill_levels
            .insert("skill.800301".into(), basic_level);
        for skill in ["skill.800302", "skill.800303", "skill.800304"] {
            loadout
                .config
                .skill_levels
                .insert(skill.into(), other_level);
        }
        profiles
            .save_preset(form, "存护技能", &loadout, true)
            .unwrap();

        let result = run();
        assert_eq!(result, run());
        assert!(result.characters[0].own_damage > previous_own_damage);
        previous_own_damage = result.characters[0].own_damage;
        assert!(result.characters[1].assist_damage > 0);
        for target in &result.targets {
            assert!(target.hits.iter().any(|hit| {
                hit.source_id == form
                    && hit.action == Action::Ultimate
                    && hit.kind == DamageKind::Direct
            }));
        }
        let hits = || result.targets.iter().flat_map(|target| &target.hits);
        assert!(hits().any(|hit| hit.source_id == form && hit.action == Action::Basic));
        assert!(hits().any(|hit| {
            hit.source_id == form
                && hit
                    .credits
                    .iter()
                    .any(|credit| credit.unit_id == pela && credit.amount > 0)
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
