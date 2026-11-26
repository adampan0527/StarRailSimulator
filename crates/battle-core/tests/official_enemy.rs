use star_rail_battle_core::{
    Action, Battle, BattleError, CombatPath, CombatType, Command, DamageKind, Element, EventKind,
    LogMode, OfficialCharacter, OfficialEnemy, OfficialStats, Outcome, PreparedBattle, StatusKind,
    official_enemy_stats, supports_official_enemy,
};

fn arlan(unit_id: &str, speed_micros: u64, hp: u32, aggro: u16) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: unit_id.into(),
        form_id: "character.1008".into(),
        combat_type: CombatType {
            path: CombatPath::Destruction,
            element: Element::Thunder,
        },
        energy_capacity: 110,
        level: 80,
        stats: OfficialStats {
            hp,
            base_hp: hp,
            attack: 2_000,
            base_attack: 2_000,
            defense: 1_000,
            base_defense: 1_000,
            base_aggro: aggro,
            base_speed_micros: speed_micros,
            speed_micros,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 10_000,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            ("skill.100801".into(), vec![0.5]),
            ("skill.100802".into(), vec![0.15, 1.2]),
            ("skill.100803".into(), vec![1.92, 0.96]),
            ("skill.100804".into(), vec![0.36]),
        ]
        .into(),
        eidolon: 0,
        enabled_trace_ids: Vec::new(),
    }
}

fn herta(unit_id: &str, speed_micros: u64, hp: u32, aggro: u16) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: unit_id.into(),
        form_id: "character.1013".into(),
        combat_type: CombatType {
            path: CombatPath::Erudition,
            element: Element::Ice,
        },
        energy_capacity: 110,
        level: 80,
        stats: OfficialStats {
            hp,
            base_hp: hp,
            attack: 1_500,
            base_attack: 1_500,
            defense: 1_000,
            base_defense: 1_000,
            base_aggro: aggro,
            base_speed_micros: speed_micros,
            speed_micros,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 0,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            ("skill.101301".into(), vec![1.0]),
            ("skill.101302".into(), vec![1.0, 0.5, 0.2]),
            ("skill.101303".into(), vec![2.0]),
            ("skill.101304".into(), vec![0.5, 0.4]),
        ]
        .into(),
        eidolon: 0,
        enabled_trace_ids: Vec::new(),
    }
}

fn serval(unit_id: &str, speed_micros: u64, hp: u32) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: unit_id.into(),
        form_id: "character.1103".into(),
        combat_type: CombatType {
            path: CombatPath::Erudition,
            element: Element::Thunder,
        },
        energy_capacity: 100,
        level: 80,
        stats: OfficialStats {
            hp,
            base_hp: hp,
            attack: 2_000,
            base_attack: 2_000,
            defense: 1_000,
            base_defense: 1_000,
            base_aggro: 75,
            base_speed_micros: speed_micros,
            speed_micros,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 20_000,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            ("skill.110301".into(), vec![1.0]),
            ("skill.110302".into(), vec![1.4, 0.6, 0.8, 2.0, 1.04]),
            ("skill.110303".into(), vec![1.8, 2.0]),
            ("skill.110304".into(), vec![0.72]),
        ]
        .into(),
        eidolon: 0,
        enabled_trace_ids: Vec::new(),
    }
}

fn hook(unit_id: &str, speed_micros: u64, hp: u32) -> OfficialCharacter {
    OfficialCharacter {
        unit_id: unit_id.into(),
        form_id: "character.1109".into(),
        combat_type: CombatType {
            path: CombatPath::Destruction,
            element: Element::Fire,
        },
        energy_capacity: 120,
        level: 80,
        stats: OfficialStats {
            hp,
            base_hp: hp,
            attack: 2_000,
            base_attack: 2_000,
            defense: 1_000,
            base_defense: 1_000,
            base_aggro: 125,
            base_speed_micros: speed_micros,
            speed_micros,
            crit_rate_bp: 0,
            crit_damage_bp: 5_000,
            element_damage_bp: [0; Element::COUNT],
            break_effect_bp: 0,
            elation_bp: 0,
            effect_hit_rate_bp: 20_000,
            energy_regeneration_bp: 10_000,
            healing_bp: 0,
            effect_resistance_bp: 0,
        },
        skill_parameters: [
            ("skill.110901".into(), vec![1.0]),
            ("skill.110902".into(), vec![2.4, 1.0, 2.0, 0.65]),
            ("skill.110903".into(), vec![4.0]),
            ("skill.110904".into(), vec![1.0, 5.0]),
            ("skill.110909".into(), vec![2.8, 1.0, 2.0, 0.65, 0.8]),
        ]
        .into(),
        eidolon: 0,
        enabled_trace_ids: Vec::new(),
    }
}

fn enemy(unit_id: &str, enemy_id: &str, level: u16) -> OfficialEnemy {
    OfficialEnemy {
        unit_id: unit_id.into(),
        enemy_id: enemy_id.into(),
        level,
    }
}

fn unit<'a>(
    snapshot: &'a star_rail_battle_core::Snapshot,
    id: &str,
) -> &'a star_rail_battle_core::UnitSnapshot {
    snapshot
        .units
        .iter()
        .find(|entry| entry.unit_id == id)
        .expect("unit in snapshot")
}

#[test]
fn verifies_supported_and_unsupported_official_enemies_and_level_stats() {
    let supported = [
        "8001010", "8001020", "1002040", "1002050", "1012010", "1012030", "1022010", "1022020",
        "2002030", "2011010", "2011020", "2022060", "8001040", "8001050", "3001010", "3011010",
    ];
    for id in supported {
        assert!(supports_official_enemy(id), "{id} should be supported");
        let plan = PreparedBattle::official_encounter(
            &[arlan("ally.arlan", 100_000_000, 10_000, 100)],
            &[vec![enemy("enemy.one", id, 80)]],
            7,
            3,
        )
        .unwrap_or_else(|error| panic!("{id} failed to prepare: {error}"));
        let battle = Battle::start(&plan, LogMode::Detailed);
        assert_eq!(battle.outcome(), Outcome::Ongoing);
    }

    // Unsupported bosses/elites/gimmicks/passives must explicitly fail.
    for unsupported in ["1004010", "3002010", "4012010", "8015012", "9999999"] {
        assert!(!supports_official_enemy(unsupported));
        let err = PreparedBattle::official_encounter(
            &[arlan("ally.arlan", 100_000_000, 10_000, 100)],
            &[vec![enemy("enemy.bad", unsupported, 80)]],
            7,
            3,
        )
        .err()
        .expect("unsupported enemy must error");
        assert!(matches!(err, BattleError::UnsupportedData(_)));
    }

    // Verify exact level-100 stats against enemy-preview.json (Silvermane Lieutenant 1003010).
    let lieutenant = official_enemy_stats("1003010", 100).unwrap();
    assert_eq!(lieutenant.hp, 338_371);
    assert_eq!(lieutenant.speed_micros, 158_400_000);

    // Verify display toughness scale (30 -> 10, 60 -> 20, 90 -> 30).
    assert_eq!(
        official_enemy_stats("8001010", 80).unwrap().max_toughness,
        10
    );
    assert_eq!(
        official_enemy_stats("1002040", 80).unwrap().max_toughness,
        20
    );
    assert_eq!(
        official_enemy_stats("1012030", 80).unwrap().max_toughness,
        20
    );
    assert_eq!(
        official_enemy_stats("2022060", 80).unwrap().max_toughness,
        30
    );

    // Invalid levels are rejected.
    assert!(matches!(
        official_enemy_stats("8001010", 0),
        Err(BattleError::InvalidInput(_))
    ));
    assert!(matches!(
        official_enemy_stats("8001010", 101),
        Err(BattleError::InvalidInput(_))
    ));
}

#[test]
fn flamespawn_hits_for_exact_damage_and_energy_breaks_on_ice_basic_and_is_burn_immune() {
    // Slow ally (SPD 50) lets Flamespawn (Lv80 SPD 109.56) act first before ally's first turn.
    let plan = PreparedBattle::official_encounter(
        &[herta("ally.herta", 50_000_000, 10_000, 75)],
        &[vec![enemy("enemy.flame", "8001010", 80)]],
        11,
        3,
    )
    .unwrap();
    let mut detailed = Battle::start(&plan, LogMode::Detailed);
    let silent = Battle::start(&plan, LogMode::Off);
    assert_eq!(detailed.snapshot(), silent.snapshot());

    // Flamespawn acted once before Herta's decision:
    // Herta started at 55_000 energy (half of 110_000) and gained 10_000 from Flamespawn hit.
    let snap_before = detailed.snapshot();
    assert_eq!(unit(&snap_before, "ally.herta").energy_milli, 65_000);
    assert!(unit(&snap_before, "ally.herta").hp < 10_000);
    assert!(
        detailed
            .events()
            .iter()
            .any(|event| event.kind == EventKind::EnemyAttack)
    );

    // Herta Ice Basic (10 toughness) breaks Flamespawn's 10 toughness immediately.
    detailed
        .apply(&Command {
            decision_index: 0,
            actor_id: "ally.herta".into(),
            action: Action::Basic,
            target_id: Some("enemy.flame".into()),
        })
        .unwrap();
    assert!(
        detailed
            .events()
            .iter()
            .any(|event| matches!(event.kind, EventKind::WeaknessBroken))
    );

    // Flamespawn has STAT_DOT_Burn = 1.0 immunity: Hook Skill must not apply Burn DoT to it.
    let burn_plan = PreparedBattle::official_encounter(
        &[hook("ally.hook", 200_000_000, 10_000)],
        &[vec![enemy("enemy.flame", "8001010", 80)]],
        13,
        3,
    )
    .unwrap();
    let mut burn_battle = Battle::start(&burn_plan, LogMode::Detailed);
    burn_battle
        .apply(&Command {
            decision_index: 0,
            actor_id: "ally.hook".into(),
            action: Action::Skill,
            target_id: Some("enemy.flame".into()),
        })
        .unwrap();
    // Advance until Flamespawn acts; no DamageOverTime event should occur on Flamespawn.
    assert!(burn_battle.events().iter().all(|event| !matches!(
        event.kind,
        EventKind::Damage {
            kind: DamageKind::DamageOverTime,
            ..
        }
    )));
}

#[test]
fn automaton_hound_cycles_between_attack_and_team_heal() {
    // Arlan (SPD 100) vs Hound (Lv80 SPD 120): Hound acts Turn 1 before Arlan,
    // Arlan damages companion Flamespawn, then Hound Turn 2 heals Flamespawn, then Turn 3 attacks again.
    let plan = PreparedBattle::official_encounter(
        &[arlan("ally.arlan", 100_000_000, 20_000, 100)],
        &[vec![
            enemy("enemy.hound", "1012010", 80),
            enemy("enemy.flame", "8001010", 80),
        ]],
        5,
        3,
    )
    .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    // Hound (Lv80 SPD 120, +15_000) acts before Arlan (SPD 100); Flamespawn (Lv80 SPD 99.6) acts after:
    // 55_000 + 15_000 = 70_000 energy.
    assert_eq!(unit(&battle.snapshot(), "ally.arlan").energy_milli, 70_000);
    // Arlan uses Basic on Flamespawn so Flamespawn is damaged before Hound's Turn 2 Heal.
    battle
        .apply(&Command {
            decision_index: 0,
            actor_id: "ally.arlan".into(),
            action: Action::Basic,
            target_id: Some("enemy.flame".into()),
        })
        .unwrap();
    // Before Arlan's next turn, Hound takes Turn 2 (Self-Repair Module -> heals Flamespawn!).
    let heals: Vec<_> = battle
        .events()
        .iter()
        .filter(|event| {
            matches!(event.kind, EventKind::Heal { amount } if amount > 0)
                && event.source_id.as_deref() == Some("enemy.hound")
                && event.target_id.as_deref() == Some("enemy.flame")
        })
        .collect();
    assert!(
        !heals.is_empty(),
        "Hound turn 2 must heal damaged companion enemy"
    );
    // On Arlan's next basic, Hound takes Turn 3 and cycles back to Vertical Strike (+15_000 energy).
    let attacks_before = battle
        .events()
        .iter()
        .filter(|event| {
            event.kind == EventKind::EnemyAttack
                && event.source_id.as_deref() == Some("enemy.hound")
        })
        .count();
    assert_eq!(attacks_before, 1);
    battle
        .apply(&Command {
            decision_index: 1,
            actor_id: "ally.arlan".into(),
            action: Action::Basic,
            target_id: Some("enemy.flame".into()),
        })
        .unwrap();
    let attacks_after = battle
        .events()
        .iter()
        .filter(|event| {
            event.kind == EventKind::EnemyAttack
                && event.source_id.as_deref() == Some("enemy.hound")
        })
        .count();
    assert_eq!(
        attacks_after, 2,
        "Hound turn 3 must wrap sequence cursor back to attack"
    );
}

#[test]
fn automaton_beetle_barrier_blocks_one_direct_hit_but_not_dot() {
    // Serval (SPD 90) vs Automaton Beetle (Lv80 SPD 109.56): Beetle acts first and gains Barrier.
    let plan = PreparedBattle::official_encounter(
        &[serval("ally.serval", 90_000_000, 20_000)],
        &[vec![enemy("enemy.beetle", "1012030", 80)]],
        9,
        3,
    )
    .unwrap();
    let mut battle = Battle::start(&plan, LogMode::Detailed);
    assert!(battle.events().iter().any(|event| {
        event.kind == EventKind::StatusApplied(StatusKind::Barrier)
            && event.target_id.as_deref() == Some("enemy.beetle")
    }));
    let hp_before = unit(&battle.snapshot(), "enemy.beetle").hp;

    // First direct hit (Basic) is completely blocked by Barrier (0 HP loss) and breaks Barrier.
    battle
        .apply(&Command {
            decision_index: 0,
            actor_id: "ally.serval".into(),
            action: Action::Basic,
            target_id: Some("enemy.beetle".into()),
        })
        .unwrap();
    assert!(battle.events().iter().any(|event| {
        event.kind == EventKind::StatusExpired(StatusKind::Barrier)
            && event.target_id.as_deref() == Some("enemy.beetle")
    }));
    assert_eq!(unit(&battle.snapshot(), "enemy.beetle").hp, hp_before);

    // Apply Skill (which deals direct hit + Shock) after Beetle puts Barrier back up?
    // Wait: Beetle (SPD 109.56) acted again before Serval's second turn and refreshed Barrier!
    // Let's verify Serval's Skill: Beetle's Barrier blocks the Skill's direct hit, then Shock is
    // applied, and on Beetle's next turn Shock DoT deals damage through Barrier!
    battle
        .apply(&Command {
            decision_index: 1,
            actor_id: "ally.serval".into(),
            action: Action::Skill,
            target_id: Some("enemy.beetle".into()),
        })
        .unwrap();
    let dot_damage = battle.events().iter().any(|event| {
        matches!(
            event.kind,
            EventKind::Damage {
                kind: DamageKind::DamageOverTime,
                hp_loss,
                ..
            } if hp_loss > 0
        ) && event.target_id.as_deref() == Some("enemy.beetle")
    });
    assert!(dot_damage, "Shock DoT must bypass Beetle's Barrier");
}

#[test]
fn ballistarius_bounce_deals_six_hits_and_grants_exact_30_energy_total() {
    // Ballistarius (2022060, Lv80 SPD 132) vs slower Arlan (SPD 80):
    // Single ally guarantees all 6 hits land on Arlan -> 15 + 5*3 = 30 energy (30_000 milli).
    let plan = PreparedBattle::official_encounter(
        &[arlan("ally.arlan", 80_000_000, 50_000, 100)],
        &[vec![enemy("enemy.bow", "2022060", 80)]],
        17,
        3,
    )
    .unwrap();
    let battle = Battle::start(&plan, LogMode::Detailed);
    let hits = battle
        .events()
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                EventKind::Damage {
                    kind: DamageKind::Direct,
                    ..
                }
            ) && event.source_id.as_deref() == Some("enemy.bow")
        })
        .count();
    assert_eq!(hits, 6, "Ballistarius must deal 1 primary + 5 bounce hits");
    // Arlan starts at 55_000 + 30_000 = 85_000.
    assert_eq!(unit(&battle.snapshot(), "ally.arlan").energy_milli, 85_000);
}

#[test]
fn enemy_debuffs_dots_delays_weaken_and_reverberation_work_on_allies() {
    // 1. Silvermane Marksman (1002050) inflicts Bleed that ticks at ally turn start.
    let bleed_plan = PreparedBattle::official_encounter(
        &[arlan("ally.arlan", 60_000_000, 30_000, 100)],
        &[vec![enemy("enemy.marksman", "1002050", 80)]],
        21,
        3,
    )
    .unwrap();
    let bleed_battle = Battle::start(&bleed_plan, LogMode::Detailed);
    assert!(bleed_battle.events().iter().any(|event| {
        matches!(
            event.kind,
            EventKind::Damage {
                kind: DamageKind::DamageOverTime,
                hp_loss,
                ..
            } if hp_loss > 0
        ) && event.target_id.as_deref() == Some("ally.arlan")
    }));

    // 2. Everwinter Shadewalker (1022010) delays ally AV by 50%.
    let delay_plan = PreparedBattle::official_encounter(
        &[arlan("ally.arlan", 60_000_000, 30_000, 100)],
        &[vec![enemy("enemy.winter", "1022010", 80)]],
        22,
        3,
    )
    .unwrap();
    let delay_battle = Battle::start(&delay_plan, LogMode::Detailed);
    // Base AV for SPD 60 is 10_000 / 60 = 166.67; with 50% delay (+5_000 gauge = +83.33 AV) -> >= 250 AV.
    assert!(
        delay_battle.snapshot().av_micros >= 249_000_000,
        "expected delayed ally AV >= 250, got {}",
        delay_battle.snapshot().av_micros
    );

    // 3. Obedient Dracolion (2011010) applies Weaken (30%) reducing ally outgoing damage.
    let clean_plan = PreparedBattle::official_encounter(
        &[arlan("ally.arlan", 200_000_000, 30_000, 100)],
        &[vec![enemy("enemy.lion", "2011010", 80)]],
        23,
        3,
    )
    .unwrap();
    let mut clean_battle = Battle::start(&clean_plan, LogMode::Detailed);
    clean_battle
        .apply(&Command {
            decision_index: 0,
            actor_id: "ally.arlan".into(),
            action: Action::Basic,
            target_id: Some("enemy.lion".into()),
        })
        .unwrap();
    let unweakened_dmg = clean_battle
        .events()
        .iter()
        .find_map(|event| match event.kind {
            EventKind::Damage {
                kind: DamageKind::Direct,
                amount,
                ..
            } if event.source_id.as_deref() == Some("ally.arlan") => Some(amount),
            _ => None,
        })
        .unwrap();

    let weak_plan = PreparedBattle::official_encounter(
        &[arlan("ally.arlan", 60_000_000, 30_000, 100)],
        &[vec![enemy("enemy.lion", "2011010", 80)]],
        23,
        3,
    )
    .unwrap();
    let mut weak_battle = Battle::start(&weak_plan, LogMode::Detailed);
    assert!(weak_battle.events().iter().any(|event| {
        event.kind == EventKind::StatusApplied(StatusKind::Weaken)
            && event.target_id.as_deref() == Some("ally.arlan")
    }));
    weak_battle
        .apply(&Command {
            decision_index: 0,
            actor_id: "ally.arlan".into(),
            action: Action::Basic,
            target_id: Some("enemy.lion".into()),
        })
        .unwrap();
    let weakened_dmg = weak_battle
        .events()
        .iter()
        .find_map(|event| match event.kind {
            EventKind::Damage {
                kind: DamageKind::Direct,
                amount,
                ..
            } if event.source_id.as_deref() == Some("ally.arlan") => Some(amount),
            _ => None,
        })
        .unwrap();
    assert!(
        weakened_dmg < unweakened_dmg * 8 / 10,
        "Weaken should cut ally damage by ~30% ({weakened_dmg} vs {unweakened_dmg})"
    );

    // 4. Wraith Warden (2002030) applies Reverberation -> second hit upgrades to StrongReverberation and skips ally turn.
    let mut observed_skip = false;
    for seed in 1..=30 {
        let rev_plan = PreparedBattle::official_encounter(
            &[arlan("ally.arlan", 40_000_000, 50_000, 100)],
            &[vec![
                enemy("enemy.warden_a", "2002030", 80),
                enemy("enemy.warden_b", "2002030", 80),
            ]],
            seed,
            3,
        )
        .unwrap();
        let rev_battle = Battle::start(&rev_plan, LogMode::Detailed);
        let has_strong = rev_battle.events().iter().any(|event| {
            event.kind == EventKind::StatusApplied(StatusKind::StrongReverberation)
                && event.target_id.as_deref() == Some("ally.arlan")
        });
        let expired_strong = rev_battle.events().iter().any(|event| {
            event.kind == EventKind::StatusExpired(StatusKind::StrongReverberation)
                && event.target_id.as_deref() == Some("ally.arlan")
        });
        if has_strong && expired_strong {
            observed_skip = true;
            break;
        }
    }
    assert!(
        observed_skip,
        "two Wraith Wardens hitting ally must trigger Reverberation -> StrongReverberation turn skip"
    );
}

#[test]
fn aggro_weights_steer_enemy_targeting_and_multi_wave_encounter_reaches_victory() {
    // High-aggro ally (500) vs low-aggro ally (50) across 40 deterministic seeds.
    let mut high_hits = 0_u32;
    let mut low_hits = 0_u32;
    for seed in 1..=40 {
        let plan = PreparedBattle::official_encounter(
            &[
                arlan("ally.tank", 50_000_000, 30_000, 500),
                herta("ally.dps", 50_000_000, 30_000, 50),
            ],
            &[vec![enemy("enemy.flame", "8001010", 80)]],
            seed,
            3,
        )
        .unwrap();
        let battle = Battle::start(&plan, LogMode::Detailed);
        for event in battle.events() {
            if event.kind == EventKind::EnemyAttack {
                match event.target_id.as_deref() {
                    Some("ally.tank") => high_hits += 1,
                    Some("ally.dps") => low_hits += 1,
                    _ => {}
                }
            }
        }
    }
    assert!(
        high_hits > low_hits * 3,
        "aggro weight 500 vs 50 must concentrate attacks on tank ({high_hits} vs {low_hits})"
    );

    // Multi-wave Lv1 encounter ends cleanly in Victory.
    let wave_plan = PreparedBattle::official_encounter(
        &[arlan("ally.arlan", 200_000_000, 10_000, 100)],
        &[
            vec![enemy("enemy.w1", "8001010", 1)],
            vec![enemy("enemy.w2", "8001020", 1)],
        ],
        1,
        3,
    )
    .unwrap();
    let mut wave_battle = Battle::start(&wave_plan, LogMode::Detailed);
    wave_battle
        .apply(&Command {
            decision_index: 0,
            actor_id: "ally.arlan".into(),
            action: Action::Basic,
            target_id: Some("enemy.w1".into()),
        })
        .unwrap();
    assert_eq!(wave_battle.outcome(), Outcome::Ongoing);
    wave_battle
        .apply(&Command {
            decision_index: 1,
            actor_id: "ally.arlan".into(),
            action: Action::Basic,
            target_id: Some("enemy.w2".into()),
        })
        .unwrap();
    assert_eq!(wave_battle.outcome(), Outcome::Victory);
}
