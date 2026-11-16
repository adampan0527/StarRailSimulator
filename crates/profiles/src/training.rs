use std::collections::BTreeMap;

use star_rail_battle_core::Session;

use crate::{
    Affix, Catalog, Character, CharacterVariant, Eidolon, Growth, LightConeData, RelicData,
    RelicSet, RelicSlot, Requirement, Resource, Skill, Superimposition, Trace,
};

fn training_name(id: &str) -> &str {
    match id {
        "trailblazer.destruction" => "开拓者 · 毁灭",
        "march_7th.preservation" => "三月七 · 存护",
        "dan_heng.hunt" => "丹恒 · 巡猎",
        "natasha.abundance" => "娜塔莎 · 丰饶",
        _ => id,
    }
}

/// Builds the profile catalog for the shared M02 training fixture.
pub fn training_catalog(session: &Session) -> Catalog {
    let growth = BTreeMap::from([
        (
            "hp".into(),
            Growth {
                base: 100.0,
                step: 10.0,
            },
        ),
        (
            "atk".into(),
            Growth {
                base: 100.0,
                step: 0.0,
            },
        ),
        (
            "def".into(),
            Growth {
                base: 100.0,
                step: 0.0,
            },
        ),
    ]);
    let promotions = std::array::from_fn(|_| growth.clone());
    let level_one = Requirement {
        level: 1,
        ascension: 0,
    };
    let skill = Skill {
        name: "测试技能".into(),
        kind: "basic".into(),
        description: "合成测试".into(),
        parameters: vec![vec![0.5]; 8],
        upgrade_requirements: vec![level_one; 6],
        level_source: None,
        conditional_levels: Vec::new(),
        hidden: false,
        display_skill_id: None,
    };
    let rank = Eidolon {
        id: "eidolon.fixture".into(),
        name: "合成星魂".into(),
        description: "验证星魂加级".into(),
        skill_bonuses: BTreeMap::new(),
        parameters: Vec::new(),
    };
    let trace = Trace {
        name: "合成行迹".into(),
        description: String::new(),
        requirement: Requirement {
            level: 40,
            ascension: 3,
        },
        prerequisites: Vec::new(),
        properties: Vec::new(),
        parameters: Vec::new(),
        automatic: false,
    };
    let characters = session
        .input
        .team
        .iter()
        .map(|ally| {
            let id = &ally.character_form_id;
            let (path, element, source_ids): (_, _, &[_]) = match id.as_str() {
                "trailblazer.destruction" => ("destruction", "physical", &["8001", "8002"]),
                "march_7th.preservation" => ("preservation", "ice", &["1001"]),
                "dan_heng.hunt" => ("hunt", "wind", &["1002"]),
                "natasha.abundance" => ("abundance", "physical", &["1105"]),
                _ => ("destruction", "physical", &[id.as_str()]),
            };
            let mut ranks = std::array::from_fn(|_| rank.clone());
            ranks[2].skill_bonuses.insert("basic".into(), 1);
            let variant = CharacterVariant {
                skills: ally
                    .config
                    .skill_levels
                    .keys()
                    .map(|id| (id.clone(), skill.clone()))
                    .collect(),
                traces: BTreeMap::from([
                    ("trace.first".into(), trace.clone()),
                    (
                        "trace.child".into(),
                        Trace {
                            prerequisites: vec!["trace.first".into()],
                            ..trace.clone()
                        },
                    ),
                ]),
                eidolons: ranks,
                servant: None,
            };
            (
                id.clone(),
                Character {
                    name: training_name(id).into(),
                    source_ids: source_ids.iter().map(|id| (*id).into()).collect(),
                    rarity: 5,
                    path: path.into(),
                    element: element.into(),
                    promotions: promotions.clone(),
                    resource: Resource {
                        kind: "energy".into(),
                        capacity: Some(120),
                    },
                    default_variant: "base".into(),
                    variants: BTreeMap::from([("base".into(), variant)]),
                    affinitive_eidolon: (id == "trailblazer.destruction").then(|| rank.clone()),
                },
            )
        })
        .collect();
    let cone = LightConeData {
        name: "训练光锥".into(),
        path: "destruction".into(),
        promotions,
        rarity: 4,
        effect_name: "合成效果".into(),
        description: String::new(),
        ranks: std::array::from_fn(|i| Superimposition {
            parameters: vec![0.125 * (i + 1) as f64],
            properties: Vec::new(),
        }),
    };
    let boots = RelicData {
        name: "训练靴".into(),
        slot: RelicSlot::Feet,
        rarity: 2,
        set_id: "fixture.training".into(),
        max_level: 6,
        main_group: "main.feet".into(),
        sub_group: "sub.feet".into(),
        initial_substats: [0, 0],
        upgrade_interval: 3,
    };
    Catalog {
        schema_version: 1,
        game_version: session.input.game_version.clone(),
        data_version: session.input.data_version.clone(),
        characters,
        light_cones: BTreeMap::from([("fixture.destruction_cone".into(), cone)]),
        relics: BTreeMap::from([("fixture.training_boots".into(), boots)]),
        relic_sets: BTreeMap::from([(
            "fixture.training".into(),
            RelicSet {
                name: "练习".into(),
                effects: BTreeMap::new(),
                properties: BTreeMap::new(),
            },
        )]),
        main_affixes: BTreeMap::from([(
            "main.feet".into(),
            BTreeMap::from([(
                "attack_ratio".into(),
                Affix {
                    base: 0.10,
                    step: 0.0,
                    step_count: 0,
                },
            )]),
        )]),
        sub_affixes: BTreeMap::from([(
            "sub.feet".into(),
            BTreeMap::from([(
                "speed".into(),
                Affix {
                    base: 2.0,
                    step: 0.3,
                    step_count: 2,
                },
            )]),
        )]),
        properties: BTreeMap::from([
            ("speed".into(), "速度".into()),
            ("attack_ratio".into(), "攻击力".into()),
        ]),
    }
}
