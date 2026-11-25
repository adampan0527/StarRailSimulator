import { invoke } from '@tauri-apps/api/core';

export type Name = { id: string; name: string; character_id?: string };
export type ScaledStat = { base: number; total: number };
export type CharacterStats = {
  hp: ScaledStat | null;
  attack: ScaledStat | null;
  defense: ScaledStat | null;
  speed: ScaledStat | null;
  crit_rate: number | null;
  crit_damage: number | null;
  element_damage: number | null;
  break_effect: number | null;
  elation: number | null;
  healing_boost: number | null;
  energy_regeneration: number | null;
  effect_hit_rate: number | null;
  effect_resistance: number | null;
};
export type CharacterSummary = Name & {
  rarity?: number;
  element: string;
  path: string;
  source_ids: string[];
  stats: CharacterStats;
};
export type TeamSlots = [
  string | null,
  string | null,
  string | null,
  string | null,
];

export type Action =
  | 'basic'
  | 'skill'
  | 'enhanced_skill'
  | 'ultimate'
  | 'enhanced_ultimate'
  | 'assist_skill'
  | 'elation_skill';
export type Outcome = 'ongoing' | 'victory' | 'defeat' | 'turn_limit';
export type Command = {
  decision_index: number;
  actor_id: string;
  action: Exclude<Action, 'elation_skill'>;
  target_id: string | null;
};

export type Unit = {
  unit_id: string;
  is_ally: boolean;
  hp: number;
  max_hp: number;
  energy_milli: number;
  max_energy_milli: number;
  toughness: number;
  max_toughness: number;
  speed_micros: number;
};

type Status =
  | 'shield'
  | 'vulnerability'
  | 'speed_up'
  | 'slow'
  | 'defense_down'
  | 'resistance_down'
  | 'dot_vulnerability'
  | 'effect_resistance_down'
  | 'besotted'
  | 'attack_down'
  | 'entangled'
  | 'survival_response'
  | 'wisemans_folly'
  | 'imprisoned'
  | 'weightless'
  | 'arcana'
  | 'epiphany'
  | 'frozen'
  | 'weakness_implanted'
  | 'ultimate_vulnerability'
  | 'unnerved'
  | 'thanatoplum_rebloom'
  | 'standoff'
  | 'befog'
  | 'parry'
  | 'taunt'
  | 'fever'
  | 'special_guest'
  | 'bait'
  | 'godmode_player'
  | 'restricted_weakness'
  | 'infinite_fury'
  | 'balefire_bind'
  | 'withered_leaf'
  | 'sublimation'
  | 'vendetta'
  | 'patron'
  | 'weaken'
  | 'seam_stitch'
  | 'supreme_stance'
  | 'weal_lots'
  | 'woes_whisper'
  | 'khaslana'
  | 'after_rain'
  | 'siren_zone'
  | 'military_merit'
  | 'peerage'
  | 'darkest_riddle'
  | 'bondmate'
  | 'dance_partner'
  | 'dahlia_zone'
  | 'wilt'
  | 'faces_of_elation'
  | 'emanator_of_elation'
  | 'deep_learning'
  | 'ripples';
type EventKind =
  | {
      event:
        | 'wave_started'
        | 'turn_started'
        | 'extra_turn_started'
        | 'triggered_skill'
        | 'enemy_attack'
        | 'follow_up'
        | 'weakness_broken'
        | 'shield_depleted'
        | 'status_cleared'
        | 'unit_defeated';
    }
  | { event: 'action'; detail: Action }
  | { event: 'aha_instant'; detail: { punchline: number; extra: boolean } }
  | { event: 'ascended_aha_instant'; detail: { bliss: number; extra: boolean } }
  | {
      event: 'aventurine_surf_elation';
      detail: { enhanced: boolean; heat: number; punchline: number };
    }
  | { event: 'complete_combustion'; detail: { active: boolean } }
  | { event: 'battle_ended'; detail: Outcome }
  | {
      event:
        | 'skill_points'
        | 'punchline'
        | 'certified_banger'
        | 'bliss'
        | 'party_trick'
        | 'wishpower'
        | 'hidden_mmr'
        | 'aventurine_surf_heat'
        | 'elation_debt'
        | 'toughness'
        | 'cloudflame_luster'
        | 'crimson_knot'
        | 'blind_bet'
        | 'apotheosis'
        | 'saber_core_resonance'
        | 'gilgamesh_interest'
        | 'gilgamesh_golden_rule'
        | 'himeko_assist_uses'
        | 'himeko_source_energy'
        | 'interpretation'
        | 'inspiration'
        | 'herta_answer'
        | 'pocket_trickshot'
        | 'blade_charge'
        | 'memosprite_charge'
        | 'memoria'
        | 'recollection'
        | 'newbud'
        | 'coreflame'
        | 'scourge'
        | 'syzygy'
        | 'moonlight'
        | 'basic_enhancement'
        | 'thrill'
        | 'cerydra_charge'
        | 'archer_charge'
        | 'ashveil_charge'
        | 'blade_recast_charge'
        | 'gluttony'
        | 'robin_vibes'
        | 'robin_rhythm'
        | 'rin_gem_energy'
        | 'magma_will'
        | 'rin_shadow_gem'
        | 'squama_sacrosancta'
        | 'jade_charge'
        | 'rappa_charge'
        | 'chroma_ink'
        | 'pawned_asset';
      detail: { before: number; after: number };
    }
  | {
      event: 'mydei_charge';
      detail: { before_milli: number; after_milli: number };
    }
  | { event: 'mydei_enhanced_skill'; detail: { godslayer: boolean } }
  | { event: 'energy'; detail: { before_milli: number; after_milli: number } }
  | { event: 'debt_collector'; detail: { active: boolean } }
  | {
      event: 'flying_aureus';
      detail: { before_halves: number; after_halves: number };
    }
  | {
      event: 'jade_tiles';
      detail: { suits: [number, number, number]; hidden_hand: boolean };
    }
  | {
      event: 'slashed_dream';
      detail: {
        before: number;
        after: number;
        overflow_before: number;
        overflow_after: number;
      };
    }
  | {
      event: 'damage';
      detail: {
        kind: DamageKind;
        amount: number;
        hp_loss: number;
        absorbed: number;
        critical: boolean;
      };
    }
  | { event: 'heal' | 'hp_spent'; detail: { amount: number } }
  | {
      event: 'shock_applied';
      detail: { origin: Action; from_break: boolean; turns: number };
    }
  | { event: 'shock_extended'; detail: { extra_turns: number } }
  | { event: 'shock_expired'; detail: { origin: Action; from_break: boolean } }
  | {
      event: 'gauge_shifted';
      detail: { before_quanta: number; after_quanta: number };
    }
  | { event: 'status_applied' | 'status_expired'; detail: Status };

export type BattleEvent = {
  av_micros: number;
  wave_index: number;
  source_id: string | null;
  target_id: string | null;
  kind: EventKind;
};

export type BattleSnapshot = {
  outcome: Outcome;
  av_micros: number;
  wave_index: number;
  decision_index: number;
  ordinary_actor_id: string | null;
  skill_points: number;
  skill_point_cap?: number;
  enemy_hp_lost: number;
  units: Unit[];
};

export type BattleView = {
  id: number;
  snapshot: BattleSnapshot;
  order: { unit_id: string; remaining_av_micros: number }[];
  events: BattleEvent[];
  names: Name[];
};

export type ReplaySummary = {
  id: number;
  started_at: number;
  finished: boolean;
};

type DamageCredit = { unit_id: string; amount: number };
export type DamageKind =
  | 'direct'
  | 'weakness_break'
  | 'super_break'
  | 'damage_over_time'
  | 'additional'
  | 'follow_up'
  | 'true_damage'
  | 'elation';
type DprHit = {
  av_micros: number;
  source_id: string;
  action: Action;
  kind: DamageKind;
  critical: boolean;
  amount: number;
  credits: DamageCredit[];
};

export type DprResult = {
  cycles: number;
  total_damage: number;
  dpr: number;
  characters: {
    unit_id: string;
    own_damage: number;
    assist_damage: number;
    dpr: number;
  }[];
  targets: { max_toughness: number; damage: number; hits: DprHit[] }[];
};

export type PlannedReplay = {
  frames: { snapshot: BattleSnapshot; event_count: number }[];
  events: BattleEvent[];
};

export const battleHistory = {
  list: () => invoke<ReplaySummary[]>('battle_history'),
  replay: (sessionId: number) =>
    invoke<BattleView>('battle_replay', { sessionId }),
};

export function errorCode(error: unknown) {
  if (typeof error === 'object' && error !== null && 'code' in error) {
    return error.code;
  }
}

export function describeError(error: unknown) {
  switch (errorCode(error)) {
    case 'replay_not_found':
      return '找不到这场战斗的记录。';
    case 'incompatible_version':
      return '当前版本无法读取这份存档或重算回放。';
    case 'storage_failed':
      return '无法读取或写入本地存档，请重试。';
    default:
      return '无法调用后端，请在桌面应用中重试。';
  }
}
