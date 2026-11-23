import { useId, useState } from 'react';
import type { Action, BattleEvent, DamageKind, Outcome } from './client';

export const actionNames: Record<Action, string> = {
  basic: '普攻',
  skill: '战技',
  enhanced_skill: '强化战技',
  ultimate: '终结技',
  enhanced_ultimate: '强化终结技',
  elation_skill: '欢愉技',
};

export function actionName(action: Action, actorId: string | null): string {
  return action === 'enhanced_skill' && actorId === 'character.1015'
    ? '结束回路连接'
    : actionNames[action];
}

export const damageNames: Record<DamageKind, string> = {
  direct: '伤害',
  weakness_break: '击破伤害',
  super_break: '超击破伤害',
  damage_over_time: '持续伤害',
  additional: '附加伤害',
  follow_up: '追加攻击',
  true_damage: '真实伤害',
  elation: '欢愉伤害',
};

export const outcomeNames: Record<Outcome, string> = {
  ongoing: '进行中',
  victory: '胜利',
  defeat: '失败',
  turn_limit: '达到回合上限',
};

const statusNames = {
  shield: '护盾',
  vulnerability: '易伤',
  speed_up: '加速',
  slow: '减速',
  defense_down: '减防',
  resistance_down: '减抗',
  dot_vulnerability: '持续伤害易伤',
  effect_resistance_down: '效果抵抗降低',
  besotted: '酩酊',
  attack_down: '攻击力降低',
  entangled: '纠缠',
  survival_response: '求生反应',
  wisemans_folly: '智者的短见',
  arcana: '奥迹',
  epiphany: '揭露',
  ultimate_vulnerability: '终结技易伤',
  frozen: '冻结',
  imprisoned: '禁锢',
  weightless: '失重',
  weakness_implanted: '弱点植入',
  unnerved: '惊惶',
  thanatoplum_rebloom: '残梅绽',
  standoff: '绝命对峙',
  befog: '醇醉',
  parry: '格挡',
  taunt: '嘲讽',
  withered_leaf: '枯叶',
  sublimation: '升华',
  vendetta: '血仇',
  patron: '老主顾',
  weaken: '虚弱',
  seam_stitch: '间隙织线',
  supreme_stance: '至高之姿',
  weal_lots: '爻光结界',
  woes_whisper: '凶星低语',
  khaslana: '卡厄斯兰那',
  after_rain: '雨过天晴',
  siren_zone: '海瑟音结界',
  military_merit: '军功',
  peerage: '爵位',
  darkest_riddle: '至暗之谜',
  bondmate: '同袍',
  dance_partner: '共舞者',
  dahlia_zone: '大丽花结界',
  wilt: '败谢',
  faces_of_elation: '欢愉万相',
  emanator_of_elation: '「欢愉」令使',
  deep_learning: '深度学习',
  ripples: '往昔的涟漪',
};
const number = new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 6 });

export function av(micros: number) {
  return number.format(micros / 1_000_000);
}

function eventTitle({ kind, source_id }: BattleEvent): string {
  switch (kind.event) {
    case 'wave_started':
      return '战斗开始';
    case 'turn_started':
      return '回合开始';
    case 'extra_turn_started':
      return '额外回合开始';
    case 'triggered_skill':
      return '自动施放战技';
    case 'ascended_aha_instant':
    case 'aha_instant':
      return kind.detail.extra ? '额外阿哈时刻' : '阿哈时刻';
    case 'punchline':
      return '笑点变化';
    case 'certified_banger':
      return '好活当赏变化';
    case 'bliss':
      return '爽点变化';
    case 'party_trick':
      return '绝活共赏变化';
    case 'wishpower':
      return '愿力变化';
    case 'elation_debt':
      return '欢愉负债变化';
    case 'action':
      return actionName(kind.detail, source_id);
    case 'complete_combustion':
      return kind.detail.active ? '进入完全燃烧' : '结束完全燃烧';
    case 'enemy_attack':
      return '敌方攻击';
    case 'follow_up':
      return '追加攻击';
    case 'skill_points':
      return '战技点变化';
    case 'energy':
      return '能量变化';
    case 'flying_aureus':
      return '飞黄变化';
    case 'slashed_dream':
      return '残梦变化';
    case 'crimson_knot':
      return '集真赤变化';
    case 'blind_bet':
      return '盲注变化';
    case 'jade_tiles':
      return '琼玉牌变化';
    case 'apotheosis':
      return '升格变化';
    case 'saber_core_resonance':
      return '炉心共鸣变化';
    case 'interpretation':
      return '解读变化';
    case 'inspiration':
      return '灵感变化';
    case 'herta_answer':
      return '谜底变化';
    case 'pocket_trickshot':
      return '优势口袋变化';
    case 'blade_charge':
      return '倏忽恩赐充能';
    case 'memosprite_charge':
      return '忆灵充能';
    case 'recollection':
      return '追忆变化';
    case 'memoria':
      return '忆质变化';
    case 'newbud':
      return '新蕊变化';
    case 'coreflame':
      return '火种变化';
    case 'scourge':
      return '毁伤变化';
    case 'mydei_charge':
      return '以血还血充能';
    case 'mydei_enhanced_skill':
      return kind.detail.godslayer ? '弑神登神' : '弑王成王';
    case 'syzygy':
      return '朔望变化';
    case 'moonlight':
      return '月色变化';
    case 'basic_enhancement':
      return '普攻强化档位';
    case 'thrill':
      return '爆点';
    case 'cerydra_charge':
      return '军功充能';
    case 'archer_charge':
      return 'Archer 充能';
    case 'rin_gem_energy':
      return '宝石能量';
    case 'rin_shadow_gem':
      return '影子宝石';
    case 'squama_sacrosancta':
      return '逆鳞变化';
    case 'debt_collector':
      return kind.detail.active ? '指定收债人' : '收债人结束';
    case 'jade_charge':
      return '翡翠天赋充能';
    case 'rappa_charge':
      return '乱破天赋充能';
    case 'chroma_ink':
      return '彩墨变化';
    case 'pawned_asset':
      return '当品变化';
    case 'damage':
      return damageNames[kind.detail.kind];
    case 'heal':
      return '治疗';
    case 'hp_spent':
      return '消耗生命';
    case 'shock_applied':
      return '施加触电';
    case 'shock_extended':
      return '延长触电';
    case 'shock_expired':
      return '触电结束';
    case 'cloudflame_luster':
      return '云火昭变化';
    case 'toughness':
      return '韧性变化';
    case 'weakness_broken':
      return '弱点击破';
    case 'gauge_shifted':
      return '行动条调整';
    case 'status_applied':
      return `施加${statusNames[kind.detail]}`;
    case 'status_expired':
      return `${statusNames[kind.detail]}结束`;
    case 'status_cleared':
      return '解除状态';
    case 'shield_depleted':
      return '护盾耗尽';
    case 'unit_defeated':
      return '单位退场';
    case 'battle_ended':
      return outcomeNames[kind.detail];
  }
}

function eventDetail({ kind }: BattleEvent) {
  switch (kind.event) {
    case 'aha_instant':
      return `${kind.detail.punchline} 笑点`;
    case 'ascended_aha_instant':
      return `${kind.detail.bliss} 爽点`;
    case 'skill_points':
    case 'punchline':
    case 'certified_banger':
    case 'bliss':
    case 'party_trick':
    case 'wishpower':
    case 'elation_debt':
    case 'blind_bet':
    case 'cloudflame_luster':
    case 'toughness':
    case 'crimson_knot':
    case 'apotheosis':
    case 'saber_core_resonance':
    case 'interpretation':
    case 'inspiration':
    case 'herta_answer':
    case 'pocket_trickshot':
    case 'blade_charge':
    case 'syzygy':
    case 'moonlight':
    case 'basic_enhancement':
    case 'thrill':
    case 'squama_sacrosancta':
    case 'jade_charge':
    case 'rappa_charge':
    case 'chroma_ink':
    case 'pawned_asset':
    case 'cerydra_charge':
    case 'recollection':
    case 'archer_charge':
    case 'rin_gem_energy':
    case 'rin_shadow_gem':
    case 'memoria':
    case 'newbud':
    case 'coreflame':
    case 'scourge':
      return `${kind.detail.before} → ${kind.detail.after}`;
    case 'memosprite_charge':
      return `${kind.detail.before}% → ${kind.detail.after}%`;
    case 'mydei_charge':
    case 'energy':
      return `${kind.detail.before_milli / 1000} → ${kind.detail.after_milli / 1000}`;
    case 'flying_aureus':
      return `${kind.detail.before_halves / 2} → ${kind.detail.after_halves / 2}`;
    case 'jade_tiles':
      return kind.detail.hidden_hand ? '暗杠' : kind.detail.suits.join(' / ');
    case 'slashed_dream': {
      const { before, after, overflow_before, overflow_after } = kind.detail;
      const dreams = `${before} → ${after}`;
      return overflow_before === overflow_after
        ? dreams
        : `${dreams} · 四相断我 ${overflow_before} → ${overflow_after}`;
    }
    case 'heal':
      return `恢复生命 ${kind.detail.amount}`;
    case 'hp_spent':
      return `消耗生命 ${kind.detail.amount}`;
    case 'damage': {
      const { amount, hp_loss, absorbed, critical } = kind.detail;
      const values = [`伤害 ${amount}`, `实际扣除生命 ${hp_loss}`];
      if (absorbed > 0) values.push(`抵挡 ${absorbed}`);
      if (critical) values.push('暴击');
      return values.join(' · ');
    }
    case 'gauge_shifted':
      return `${number.format(kind.detail.before_quanta / 1_000_000_000_000)} → ${number.format(kind.detail.after_quanta / 1_000_000_000_000)} 行动条点数`;
    default:
      return '';
  }
}

const keyEvents = new Set([
  'wave_started',
  'action',
  'weakness_broken',
  'unit_defeated',
  'battle_ended',
]);

export function Timeline({
  events,
  unitName,
}: {
  events: BattleEvent[];
  unitName: (id: string) => string;
}) {
  const titleId = useId();
  const [all, setAll] = useState(false);
  const [selected, setSelected] = useState<number | null>(null);
  const visible = events
    .map((event, index) => ({ event, index }))
    .filter(({ event }) => all || keyEvents.has(event.kind.event));
  const active =
    visible.find(({ index }) => index === selected) ?? visible.at(-1);

  function actors(event: BattleEvent) {
    return [event.source_id, event.target_id]
      .filter((id): id is string => id !== null)
      .map(unitName)
      .join(' → ');
  }

  return (
    <section className="panel timeline" aria-labelledby={titleId}>
      <div className="section-heading">
        <h3 id={titleId}>事件时间轴</h3>
        <span className="tag">{visible.length} 条</span>
      </div>
      {visible.length === 0 ? (
        <p>战斗尚无事件。</p>
      ) : (
        <>
          <div className="event-toolbar">
            <label>
              事件范围
              <select
                value={all ? 'all' : 'key'}
                onChange={(event) => setAll(event.target.value === 'all')}
              >
                <option value="key">关键事件</option>
                <option value="all">全部事件</option>
              </select>
            </label>
            <label>
              定位事件
              <select
                value={active?.index ?? ''}
                onChange={(event) => setSelected(Number(event.target.value))}
              >
                {visible.map(({ event, index }) => (
                  <option key={index} value={index}>
                    {av(event.av_micros)} AV · {eventTitle(event)}{' '}
                    {actors(event)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {active && (
            <div className="event-detail" role="status">
              <span className="numeric">{av(active.event.av_micros)} AV</span>
              <strong>{eventTitle(active.event)}</strong>
              <span>{actors(active.event)}</span>
              {eventDetail(active.event) && (
                <span>{eventDetail(active.event)}</span>
              )}
            </div>
          )}
        </>
      )}
    </section>
  );
}
