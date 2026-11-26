# 4.7.0 敌方静态技能与 AI 决策归类审计

数据基线：`DimbreathBot/TurnBasedGameData` 修订 `dad9155eb00fdeda31fb8456507c53590d49071f`（国服 4.7.0），与 [enemy-preview.json](../src/team/enemy-preview.json) 严格 1:1 对齐，共覆盖 **159** 个高难实体战斗单位。所有 275 份上游源文件均通过 [enemy-ai-lock.json](../scripts/m03/enemy-ai-lock.json) 的 SHA-256 校验，可完全离线重建。

## 总体统计

| 指标                                 | 数值                                                              |
| ------------------------------------ | ----------------------------------------------------------------- |
| 高难可选敌方单位                     | 159                                                               |
| 独立 ConfigAI 决策文件               | 114                                                               |
| 独立 ConfigCharacter 角色配置文件    | 159                                                               |
| 已解析技能条目（MonsterSkillConfig） | 970（含受击回能 `SPHitBase`：463 条；纯被动/未绑定触发键：29 条） |
| 上游状态定义（MonsterStatusConfig）  | 754                                                               |

## 实现层级归类（供第二步与第三步排期）

| 层级                                | 数量 | 构成                                    | 落地策略                                                                                  |
| ----------------------------------- | ---- | --------------------------------------- | ----------------------------------------------------------------------------------------- |
| `sequence_only`（纯固定序列）       | 36   | 普通 36（无被动白板 16、含被动天赋 20） | 第二步通用执行器直接数据驱动：按 `skillSequence` 循环施放；含被动天赋单位另行标记被动钩子 |
| `generic_rules`（通用周期/CD 规则） | 1    | 普通 1、精英 0、首领 0                  | 第二步通用执行器直接数据驱动：支持 `AI_CD`/`AI_ICD` 与自洽局部周期变量                    |
| `custom_mechanics`（专属机制）      | 122  | 首领 48、精英 43、普通 31               | 第三步按高难出场优先级逐个建立独立 Rust 机制模块并补逐行动回归                            |

### 特征命中统计

| 分类特征                           | 命中单位数 |
| ---------------------------------- | ---------- |
| `contains_special_skill_tag`       | 100        |
| `dynamic_state_machine`            | 71         |
| `has_summon_ids`                   | 58         |
| `modifier_condition_or_mutation`   | 55         |
| `battle_state_predicates`          | 41         |
| `multi_phase`                      | 39         |
| `fixed_skill_sequence`             | 38         |
| `custom_target_selector`           | 37         |
| `empty_ai_decision_ability_driven` | 35         |
| `external_dynamic_state`           | 20         |
| `skill_cd_usability`               | 12         |
| `empty_sequence_ability_driven`    | 2          |
| `unmapped_ai_skill`                | 2          |

### 技能目标类型分布（来自 `ConfigCharacter`）

| `TargetType`    | 技能数 |
| --------------- | ------ |
| `Caster`        | 432    |
| `EnemySelect`   | 286    |
| `AllEnemy`      | 197    |
| `FriendSelect`  | 13     |
| `AllTeamMember` | 13     |

## 第二步可直接数据驱动的单位清单

### 1. `sequence_only`（36 个）

| ID        | 名称                     | 星球           | 序列长度 | 含被动 | 技能列表（触发键 / 目标 / 受击回能）                                                                                                                                        |
| --------- | ------------------------ | -------------- | -------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `8001010` | 炎华造物                 | 空间站「黑塔」 | 1        | 否     | 扰乱(Skill01/EnemySelect/回能10)                                                                                                                                            |
| `8001020` | 霜晶造物                 | 空间站「黑塔」 | 1        | 否     | 扰乱(Skill01/EnemySelect/回能10)                                                                                                                                            |
| `1002040` | 银鬃近卫                 | 雅利洛-VI      | 1        | 否     | 奔袭(Skill01/EnemySelect/回能10)                                                                                                                                            |
| `1002050` | 银鬃射手                 | 雅利洛-VI      | 1        | 否     | 破甲榴弹(Skill02/EnemySelect/回能10)                                                                                                                                        |
| `1012010` | 自动机兵•战犬            | 雅利洛-VI      | 2        | 否     | 垂直打击(Skill01/EnemySelect/回能15)、自愈模块(Skill05/AllTeamMember/回能0)                                                                                                 |
| `1012030` | 自动机兵•甲虫            | 雅利洛-VI      | 1        | 否     | 不稳定力场(Skill01/EnemySelect/回能15)                                                                                                                                      |
| `1022010` | 永冬灾影                 | 雅利洛-VI      | 1        | 否     | 霜冻重击(Skill01/EnemySelect/回能15)                                                                                                                                        |
| `1022020` | 火焚灾影                 | 雅利洛-VI      | 1        | 否     | 灼炎重击(Skill01/EnemySelect/回能15)                                                                                                                                        |
| `8032010` | 基层员工•外勤            | 雅利洛-VI      | 1        | 是     | 打卡(Skill01/EnemySelect/回能10)、提升业绩(PassiveSkill01/Caster/回能0)、803201003(PassiveSkill02/Caster/回能0)                                                             |
| `8032020` | 基层员工•安保            | 雅利洛-VI      | 1        | 是     | 申诉(Skill01/EnemySelect/回能10)、提升业绩(PassiveSkill01/Caster/回能0)、803202003(PassiveSkill02/Caster/回能0)                                                             |
| `2002010` | 云骑巡防士卒             | 仙舟「罗浮」   | 1        | 是     | 云刃(Skill01/EnemySelect/回能15)、200201002(Skill02/AllTeamMember/回能0)、200201003(PassiveSkill01/Caster/回能0)、200201004(Skill03/EnemySelect/回能15)                     |
| `2002030` | 幽府武弁                 | 仙舟「罗浮」   | 1        | 否     | 锯恶(Skill01/EnemySelect/回能15)                                                                                                                                            |
| `2011010` | 入魔机巧 • 率从狻猊      | 仙舟「罗浮」   | 1        | 否     | 狮子吼(Skill01/EnemySelect/回能10)                                                                                                                                          |
| `2011020` | 入魔机巧 • 浓云金蟾      | 仙舟「罗浮」   | 1        | 否     | 霏雾(Skill01/EnemySelect/回能10)                                                                                                                                            |
| `2012010` | 入魔机巧 • 灯昼龙鱼      | 仙舟「罗浮」   | 1        | 是     | 宵明(Skill01/AllEnemy/回能5)、烛花(SkillP01/Caster/回能5)                                                                                                                   |
| `2022040` | 丰饶灵兽•娄金            | 仙舟「罗浮」   | 1        | 是     | 角牴(Skill01/EnemySelect/回能10)、嗥吠(SkillP01/Caster/回能0)                                                                                                               |
| `2022060` | 「药王秘传」器元士       | 仙舟「罗浮」   | 1        | 否     | 犀弩(Skill01/EnemySelect/回能15)                                                                                                                                            |
| `2032010` | 玄爪狼卒                 | 仙舟「罗浮」   | 1        | 是     | 厮杀(Skill01/EnemySelect/回能10)、狼吞(Skill02/Caster/回能0)                                                                                                                |
| `2032020` | 蚀月狼卒                 | 仙舟「罗浮」   | 1        | 是     | 啃噬(Skill01/EnemySelect/回能10)、狼吞(Skill02/Caster/回能0)                                                                                                                |
| `8001040` | 鸣雷造物                 | 仙舟「罗浮」   | 1        | 否     | 雷腾(Skill01/EnemySelect/回能10)                                                                                                                                            |
| `8001050` | 巽风造物                 | 仙舟「罗浮」   | 1        | 否     | 风色(Skill01/EnemySelect/回能10)                                                                                                                                            |
| `3001010` | 惊梦剧团的弹簧荷官       | 匹诺康尼       | 1        | 否     | 掷点(Skill01/EnemySelect/回能10)                                                                                                                                            |
| `3002020` | 惊梦剧团的幸运贩售       | 匹诺康尼       | 1        | 是     | 摇奖热身(Skill04/EnemySelect/回能15)、惊梦蛋池(PassiveSkill02/Caster/回能0)、歪了！(Skill01/AllEnemy/回能0)、保底！(Skill02/AllEnemy/回能0)、出货！(Skill03/AllEnemy/回能0) |
| `3002030` | 惊梦剧团的气泡锂犬       | 匹诺康尼       | 1        | 是     | 邀饮(Skill01/EnemySelect/回能10)、泼洒甜蜜(Skill02/Caster/回能0)、酣畅(SkillP01/Caster/回能0)                                                                               |
| `3011010` | 忆域迷因「视线无处不在」 | 匹诺康尼       | 1        | 否     | 他者凝视(Skill01/EnemySelect/回能10)                                                                                                                                        |
| `3012020` | 忆域迷因「我心支离破碎」 | 匹诺康尼       | 1        | 是     | 排异(Skill03/EnemySelect/回能10)、刚愎盾甲(SkillP01/Caster/回能0)                                                                                                           |
| `4012030` | 天谴猎手                 | 翁法罗斯       | 1        | 是     | 送出哀泣的飞矢(Skill01/EnemySelect/回能8)、城邦守护者(PassiveSkill/Caster/回能0)                                                                                            |
| `4032010` | 黑潮蚀刃                 | 翁法罗斯       | 1        | 是     | 破灭的锋芒(Skill01/EnemySelect/回能10)、末路既定(SkillP01/Caster/回能0)                                                                                                     |
| `4032050` | 黑潮朽弓                 | 翁法罗斯       | 1        | 是     | 恶潮飞矢(Skill01/AllEnemy/回能10)、末路既定(SkillP01/Caster/回能0)                                                                                                          |
| `4052010` | 浪花侍女                 | 翁法罗斯       | 1        | 是     | 海的裙摆飘荡(Skill01/AllEnemy/回能10)、冥河的孤独难耐(Skill02/AllTeamMember/回能0)、请泊舟暂歇(PassiveSkill01/Caster/回能0)、405201004(PassiveSkill02/Caster/回能0)         |
| `4062020` | 金血忆灵•犬形            | 翁法罗斯       | 1        | 是     | 罪血啃噬(Skill01/EnemySelect/回能10)、罪血之污(SkillP01/Caster/回能0)、同享罪血(SkillP02/Caster/回能0)                                                                      |
| `5012030` | 攻击性阅读物             | 二相乐园       | 1        | 是     | 读或死！(Skill01/EnemySelect/回能10)、知识的诅咒(SkillP01/Caster/回能0)                                                                                                     |
| `5012040` | 助手涂鸦•拖稿            | 二相乐园       | 1        | 是     | 拖？再拖啊！(Skill01/EnemySelect/回能10)、热情创作中…(PassiveSkill01/Caster/回能0)                                                                                          |
| `5012050` | 助手涂鸦•赶稿            | 二相乐园       | 1        | 是     | 快！再快点！(Skill01/EnemySelect/回能10)、不怕耳旁风(PassiveSkill01/Caster/回能0)                                                                                           |
| `5012100` | 魔法少女急袭剧团         | 二相乐园       | 1        | 是     | 恋爱❤雷暴天(Skill01/EnemySelect/回能10)、柔雨❤好想你(Skill02/EnemySelect/回能10)、初吻❤天气晴(Skill03/AllEnemy/回能0)、倾心❤雨转晴(PassiveSkill01/Caster/回能0)             |
| `5012110` | 狸猫记者                 | 二相乐园       | 1        | 是     | 抓拍(Skill01/EnemySelect/回能10)、丑照特写(PassiveSkillInitiate/Caster/回能0)                                                                                               |

### 2. `generic_rules`（1 个）

| ID        | 名称     | 分级 | 星球     | 决策数 | 特征                 |
| --------- | -------- | ---- | -------- | ------ | -------------------- |
| `4012010` | 天谴斗士 | 普通 | 翁法罗斯 | 1      | `skill_cd_usability` |

## 第三步需专属机制模块的单位概览（122 个）

| 分级 | 数量 | 代表单位（前 6 个）                                                                                                                                                |
| ---- | ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 首领 | 48   | 虫群•真蛰虫(`8024011`)、碎星王虫•斯喀拉卡巴兹（拟造）(`8025010`)、诛尽杀绝的灭星兽(`8015012`)、遮天蔽日的噬星蠹(`8025011`)、可可利亚(`1004010`)、杰帕德(`1004020`) |
| 精英 | 43   | 外宇宙之冰(`8003010`)、外宇宙之炎(`8003020`)、虚卒•践踏者(`8013010`)、银鬃尉官(`1003010`)、自动机兵•灰熊(`1013010`)、自动机兵•齿狼(`1013020`)                      |
| 普通 | 31   | 虚数织叶者(`8002010`)、重子(`8011010`)、反重子(`8011020`)、虚卒•掠夺者(`8012010`)、虚卒•篡改者(`8012020`)、虚卒•抹消者(`8012030`)                                  |

## 显式标记的待补全项

遵循 [模拟与游戏数据规范](simulation-standards.md)，本阶段不以零值或估算值静默补齐以下两类数据，均在导出结构的 `unresolved` 字段显式声明，留待第二步/第三步按 `Config/ConfigAbility/Monster/*.json` 与实测对照补齐：

- **多段攻击分拆与精确倍率公式**：`MonsterSkillConfig.ParamList` 已完整导出，但部分技能的单段倍率分配与 `SPHitRatio` 需从对应 `ConfigAbility/Monster/*.json` 的 `DamageByAttackProperty` 核对。
- **敌方异常状态的基础概率、持续回合与驱散规则**：状态字典已锁定 `MonsterStatusConfig.json`，具体挂钩时机与数值需在对应敌人规则接入时逐项核对。
