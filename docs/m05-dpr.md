# DPR 靶机与伤害归因

## 第一版约定

- 「组队」的「计算 DPR」从当前选中的已保存配队读取站位和配装。DPR 场景由后端固定，自选敌方的五个站位可留空。队伍包含暂不支持的配置时，列出对应角色并停止整队计算。
- 固定场景用三台靶机，最大韧性为 `30 / 120 / 30`，按普通普攻基础削韧 `10` 的尺度计。靶机对已接入的属性均有弱点，属性抗性为 `0%`，固定防御 `500`，速度 `100`。
- 内置测试评估前三个关卡轮次 `[0, 350 AV)`。固定种子为 `42`，依次优先考虑伤害型终结技、伤害型强化普攻、伤害型战技和普攻。单体动作优先攻击中间靶机。全体动作结算各目标。
- 靶机永久留场，伤害全额记账，没有击杀回能。靶机按行动条排程，在自己的回合恢复已打空的韧性并推进状态到期，回合内待机。打空韧性的普通命中使用破韧前的伤害乘区，随后独立结算击破伤害与行动延后。
- 逐靶机保留每次伤害的时点、实际来源、触发动作、类型、伤害和角色归属。全队 DPR 等于窗口内所有靶机承受的伤害除以轮数，每位角色的贡献 DPR 采用同一分母。贡献按整数伤害结算，归属之和等于实际伤害。
- 当前合成规则中的跨角色直接乘区是易伤。保留本次命中的目标、暴击结果、击破状态等条件，去掉队友提供的易伤后按同一规则重算一次。将伤害差额记给易伤来源角色，自身提供的易伤归实际造成者。普通和击破伤害分别归因。

## 4.7.0 角色覆盖

- 当前支持目录锁定修订下的 80 级角色配置，包括表中的基础形态，以及卡芙卡、瓦尔特、黑天鹅、银狼、希儿、花火、藿藿、刃、镜流和流萤的强化版。保存只包含受支持配置的配队后点击「计算 DPR」。开战时角色满生命，普通能量以半能量为预设并结算入场效果，飞霄与黄泉分别按自身规则初始化飞黄和残梦。队伍起始有 3 个共用战技点，使用固定打法和三轮窗口。

| 批次 | 角色机制与回归 |
| --- | --- |
| 第一批 | [阿兰](characters/arlan.md)、[黑塔](characters/herta.md)、[希露瓦](characters/serval.md) |
| 第二批 | [丹恒（巡猎）](characters/dan-heng.md)、[娜塔莎](characters/natasha.md)、[佩拉](characters/pela.md) |
| 第三批 | [三月七（存护）](characters/march-preservation.md)、[杰帕德](characters/gepard.md)、[克拉拉](characters/clara.md) |
| 第四批 | [虎克](characters/hook.md)、[桑博](characters/sampo.md)、[卢卡](characters/luka.md) |
| 第五批 | [姬子](characters/himeko.md)、[素裳](characters/sushang.md)、[彦卿](characters/yanqing.md) |
| 第六批 | [艾丝妲](characters/asta.md)、[布洛妮娅](characters/bronya.md)、[停云](characters/tingyun.md) |
| 第七批 | [桂乃芬](characters/guinaifen.md)、[椒丘](characters/jiaoqiu.md)、[卡芙卡（原版与强化版）](characters/kafka.md) |
| 第八批 | [加拉赫](characters/gallagher.md)、[寒鸦](characters/hanya.md)、[驭空](characters/yukong.md) |
| 第九批 | [白露](characters/bailu.md)、[玲可](characters/lynx.md)、[罗刹](characters/luocha.md) |
| 第十批 | [米沙](characters/misha.md)、[貊泽](characters/moze.md)、[雪衣](characters/xueyi.md) |
| 第十一批 | [真理医生](characters/dr-ratio.md)、[巡猎三月七](characters/march-hunt.md)、[托帕&账账](characters/topaz.md) |
| 第十二批 | [瓦尔特（原版与强化版）](characters/welt.md) |
| 第十三批 | [黑天鹅（原版与强化版）](characters/black-swan.md)、[景元](characters/jing-yuan.md)、[飞霄](characters/feixiao.md) |
| 第十四批 | [希儿（原版与强化版）](characters/seele.md)、[银狼（原版与强化版）](characters/silver-wolf.md)、[符玄](characters/fu-xuan.md) |
| 第十五批 | [黄泉](characters/acheron.md)、[砂金](characters/aventurine.md)、[花火（原版与强化版）](characters/sparkle.md) |
| 第十六批 | [藿藿（原版与强化版）](characters/huohuo.md)、[青雀](characters/qingque.md)、[银枝](characters/argenti.md) |
| 第十七批 | [阮·梅](characters/ruan-mei.md)、[知更鸟](characters/robin.md)、[星期日](characters/sunday.md) |
| 第十八批 | [波提欧](characters/boothill.md)、[刃（原版与强化版）](characters/blade.md)、[镜流（原版与强化版）](characters/jingliu.md) |
| 第十九批 | [丹恒·饮月](characters/dan-heng-il.md)、[同谐开拓者](characters/trailblazer-harmony.md)、[灵砂](characters/lingsha.md) |
| 第二十批 | [翡翠](characters/jade.md)、[忘归人](characters/fugue.md)、[流萤（原版与强化版）](characters/firefly.md) |
| 第二十一批 | [云璃](characters/yunli.md)、[乱破](characters/rappa.md)、[大黑塔](characters/the-herta.md) |
| 第二十二批 | [缇宝](characters/tribbie.md)、[记忆开拓者](characters/trailblazer-remembrance.md)、[那刻夏](characters/anaxa.md) |
| 第二十三批 | [万敌](characters/mydei.md)、[阿格莱雅](characters/aglaea.md)、[赛飞儿](characters/cipher.md) |
| 第二十四批 | [遐蝶](characters/castorice.md)、[白厄](characters/phainon.md)、[爻光](characters/yao-guang.md) |
| 第二十五批 | [风堇](characters/hyacine.md)、[火花](characters/sparxie.md)、[海瑟音](characters/hysilens.md) |
| 第二十六批 | [刻律德菈](characters/cerydra.md)、[长夜月](characters/evernight.md) |
| 第二十七批 | [丹恒·腾荒](characters/dan-heng-permansor.md) |
| 第二十八批 | [昔涟](characters/cyrene.md)、[Saber](characters/saber.md) |
| 第二十九批 | [Archer](characters/archer.md) |
| 第三十批 | [大丽花](characters/dahlia.md) |
| 第三十一批 | [星神★阿哈](characters/aeon-aha.md) |
| 第三十二批 | [远坂凛](characters/rin.md) |
| 第三十三批 | [欢愉开拓者](characters/trailblazer-elation.md)、[毁灭开拓者](characters/trailblazer-destruction.md) |
| 第三十四批 | [存护开拓者](characters/trailblazer-preservation.md) |
| 第三十五批 | [吉尔伽美什](characters/gilgamesh.md) |
| 第三十六批 | [真珠](characters/pearl.md)、[知更鸟·晴歌](characters/robin-sunny.md) |

- 正式角色的有效技能等级从档案解析，技能倍率来自冻结的 4.7.0 快照。面板计算纳入配装静态属性，生命、攻击与防御四舍五入为整数，速度保留百万分位，倍率以万分比计算。光锥和遗器战斗触发效果、秘技及客户端逐事件对照待补。
- 固定靶机保持满生命，正常排程、承受击破、冻结和触电。80 级击破基值使用 3767.5533。雷击破触电在目标回合开始结算，冰击破冻结跳过一次行动并将下一次行动提前 50%。这些结算属于当前靶场规则。合成回放仍使用原合成版本。[游戏内冰击破教程](https://hsr.gachabase.net/tutorials/battle/2202/weakness-break-ice/beta?lang=en)
- 工作区回归使用已导入的固定目录，从 SQLite 保存的三人稀疏配队计算两次，核对确定性、逐目标伤害总和及混入未支持角色时的整队拒绝。三位角色分别使用独立分支和 Git worktree，合并前复跑 19 项角色测试。[保存配队回归](../src-tauri/src/character_archive_tests.rs)
- 正式靶机以 100,000 生命上限计算物理击破裂伤，生命本身保持满值。两侧按普通目标计算裂伤与风化，中央按精英目标计算。裂伤与风化随目标回合结算，减速改变靶机的回合时点。受击触发、治疗和净化使用有限目标或手动指令夹具。固定打法仍优先选择输出动作。[游戏内物理击破教程](https://hsr.gachabase.net/tutorials/battle/2201/weakness-break-physical/beta?lang=en)
- 正式靶场记录易伤、减防与属性减抗的来源。重算同一击时保留伤害来源自己的效果，队友的效果按队伍站位依次补回，各次差值归给对应队友。测试核对佩拉的助攻大于零，且角色自身与助攻之和等于全队伤害。[配队和归因回归](../src-tauri/src/character_archive_tests.rs)
- 技能冻结和冰击破冻结区分来源。技能冻结的冰附伤可暴击。护盾按来源独立记录并同时承受攻击，实际吸收按当时最强护盾计算。反击、护盾吸收和单场一次复苏使用有限敌人夹具验证。实际固定靶机保持待机，配队不会凭空触发受击反击。[保存配队回归](../src-tauri/src/character_archive_tests.rs)
- Edge 模拟 IPC 验证保存配队的数值结果、角色名称、逐靶机展开、持续伤害类型、切换队伍后清除旧结果，以及 360 CSS px 宽窗口和未支持队伍提示。
- 角色施加的灼烧、风化与裂伤按来源保留，并与同属性的击破持续伤害分开计时。桑博的易伤仅提高持续伤害，卢卡的引爆沿用裂伤来源，记录触发动作并保留原持续回合。保存配队回归核对三位角色的持续伤害明细及整队归因守恒。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 全队角色可以监听敌方弱点击破。同一次攻击的各段结算完成后，先处理出手角色的动作后效果，再按队伍站位处理追击触发，排队结算。姬子的群体追击和彦卿的单体追击保留各自来源。素裳的击破特攻同时用于物理击破伤害和后续击破裂伤。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 固定打法会按角色策略使用艾丝妲的加速终结技、布洛妮娅的拉条与攻暴增益，以及停云的赐福和定额回能。友方攻击、暴伤和增伤按施加者参与同一次命中的反事实归因。停云触发的雷附伤保留实际出手者及其雷伤面板，基础贡献记给停云。加速、拉条和回能照常影响行动序列，间接归因留待单独建模。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 桂乃芬和卡芙卡引爆既有持续伤害时保留原施加者，记录本次触发动作，并保留持续伤害的剩余回合。吞火与烬煨的增伤按来源归因。保存四人配队核对卡芙卡终结技引爆桑博风化的来源，逐击对账助攻和伤害总量。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 加拉赫的酩酊增加即时击破伤害，寒鸦的负担累计合格指令并返还战技点，驭空的鸣弦号令随普通回合消耗。虚数击破另结算禁锢延后。暴击率光环参与直接伤害归因时，各次重算复用实际命中的同一次随机判定值。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 白露的过量治疗可暂时提高角色生命上限，生命倍率伤害按该增益的来源归因。保存配队验证白露 E4 的增伤贡献。生息受击治疗、护盾吸收与致命攻击救援由有限敌人夹具复验。[角色规则与测试](characters/bailu.md)
- 玲可的求生反应提高生命上限并随受益者的回合计时。量子纠缠在目标回合开始造成追加伤害，保留原击破者。罗刹的自动治疗、净化和结界由有限敌人夹具验证。保存四人配队核对玲可的命中与白露 E4、罗刹 E6 的增益归因。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 米沙按实际消耗的战技点增加终结技段数，雪衣按实际削韧累计恶报，貊泽离场后保留针对猎物的附伤和追击。保存四人配队核对米沙的终结技弹射、貊泽和雪衣的追击来源及整队归因守恒。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 真理医生按目标当前持有的独立负面效果计算天赋追击概率。椒丘的烬煨易伤与灼烧合计为一项，貊泽的猎物和 A6 追击易伤分别计数。保存四人配队核对真理医生的追击来源与貊泽提供的伤害增量。[角色规则](characters/dr-ratio.md) · [配队回归](../src-tauri/src/character_archive_tests.rs)
- 账账以独立速度和行动值入队列，伤害记为托帕的追击。负债证明及叠起的 E1 被执行效果分别计数。巡猎三月七从冻结档案读取师父命途与属性，计算充能、强化普攻、附伤与匹配弱点的削韧。保存四人配队核对账账的追击、真理医生得到的负债证明增伤和三月七的命中。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 瓦尔特两版在同一保存配队中按档案所选配置结算。减速、禁锢和强化版失重独立记录。失重的属性减抗、减防及攻击者获得的增伤按瓦尔特归因。两版终结技暂按总倍率均分两段，等待客户端逐段对照。[角色规则](characters/welt.md) · [配队回归](../src-tauri/src/character_archive_tests.rs)
- 黑天鹅两版的奥迹区分目标回合自然结算和队友引爆，持续伤害保留原施加者。景元的神君按段数改变速度，在账账共用的独立召唤排程中逐段选择目标。飞霄按实际攻击增长飞黄，累积六点即可施放终结技。保存配队验证两种召唤、飞黄终结技及奥迹共存，并核对两版黑天鹅的归因守恒。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 希儿的再现可在当前行动值插入额外回合，结束后恢复原来的排程。银狼的植入可供对应属性的队友击破。符玄阵法按两个承伤目标各自的防御、抗性、减伤与护盾结算分摊后的伤害。保存配队验证希儿、银狼两版可以切换，以及银狼和符玄的增益归因。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 黄泉按成功施加负面效果的行动累计残梦，三次斩击逐段消耗集真赤。砂金按所受攻击和护盾条件累积盲注，使用防御作为伤害与护盾的面板来源。花火提高共用战技点上限，强化版终结技可储存溢出点数。保存配队验证黄泉发动终结技及两位辅助的逐击贡献，并逐一计算所有已支持的冻结档案配置。[配队与档案回归](../src-tauri/src/character_archive_tests.rs)
- 藿藿的禳命可以在冻结跳过回合前净化控制，终结技按队友能量上限返还能量。青雀连续摸牌保留当前回合和行动值，实际消耗通知花火。银枝在 90 至不足 180 能量时可使用低档终结技，满 180 后使用强化档，固定打法优先等待强化档。两档均走终结技相关触发，回放增加了强化终结技指令。保存配队复核不求人追击、银枝强化终结技及藿藿、花火的逐击贡献。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 阮·梅的战技提高队友的削韧效率，残梅绽延后目标恢复韧性并造成冰击破伤害。知更鸟在协奏期间暂停普通行动，按固定暴伤追加物理附伤。星期日可推进角色与可拉条召唤物的行动值，知更鸟的协奏时钟继续独立计时。保存配队复验景元神君、协奏附伤及三位辅助的逐击贡献。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 波提欧对峙时锁定强化普攻目标，并按击破与击败累计优势口袋。刃和镜流共用实际生命消耗的结算入口，分别累计刃的追击充能和镜流强化版的生命事件。保存配队复验刃的追击、镜流的直伤、波提欧的击破和阮·梅的助攻。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 饮月可在同一回合准备三档强化普攻，确认时优先支付逆鳞。同谐开拓者的伴舞把攻击中命中已击破目标的有效削韧段按目标结算为超击破。灵砂的浮元独立排程，醇醉同时增加击破和超击破伤害。保存四人配队以花火补充战技点，复验饮月普攻、浮元追击及伴舞和醇醉的超击破贡献。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 翡翠按实际命中目标数累计追击，收债人的生命消耗使用共用入口。忘归人的云火昭可以造成第二次弱点击破，狐祈按目标区分削韧效率。流萤原版与强化版共用完全燃烧的独立时钟，并按所选版本结算弱点植入、击破和超击破。保存配队复验翡翠追击、流萤超击破，以及忘归人与伴舞各自的辅助贡献。[配队回归](../src-tauri/src/character_archive_tests.rs)
- 云璃的格挡可在下一回合结束时触发反击，有限敌人场景另测实际受击。乱破按真实击破增加充能，三段强化普攻保留目标各自的削韧与超击破。大黑塔的多段伤害按实际扩散范围结算，并支持解读叠加、重排和跨波转移。[配队与档案回归](../src-tauri/src/character_archive_tests.rs)
- 迷迷拥有独立生命、行动值和伤害来源，迷迷自身的伤害计入开拓者，声援真伤保留触发者并将贡献计入开拓者。联合普攻的两次攻击分别触发缇宝结界与一魂真伤，共用一次角色行动。那刻夏逐次命中植入弱点，满足质性揭露时在同一行动值追加战技。保存四人配队复验两种真伤来源、迷迷和那刻夏的输出、固定种子重复计算及逐笔归因守恒。[配队与档案回归](../src-tauri/src/character_archive_tests.rs)
- 万敌按实际生命变化累计血仇，强化战技与额外回合保留终结技插入时机。衣匠拥有自身行动条，并与至高之姿的倒计时分别推进。赛飞儿记录老主顾实际承受的非真伤，在终结技中分摊真伤。保存四人配队复验迷迷与衣匠各自的命中来源、万敌强化战技、赛飞儿真伤和固定种子归因守恒。阿格莱雅的低等级档案另测天赋 2 级与 4 级带小数的衣匠生命。[配队与档案回归](../src-tauri/src/character_archive_tests.rs)
- 遐蝶用失血和治疗累计新蕊，死龙独立行动并按遐蝶生命上限结算伤害。白厄用火种开启境界，暂停暂离队友与其召唤物的行动条，支持两种强化战技指令。爻光引入阿哈独立行动、共享笑点和欢愉伤害。四人保存档案复验死龙在耗血治疗队的出手与伤害归因，低等级保存档案复验爻光的欢愉技。白厄与爻光同队的模拟验证境界期间暂停阿哈时钟，队友回场后恢复。[配队与档案回归](../src-tauri/src/character_archive_tests.rs)
- 小伊卡按风堇及自身的治疗账本出手，保存配队复验小伊卡与死龙的独立来源及风堇的生命上限助攻。火花用爆点和战技点互动，保留原回合直至强化普攻结算，低等级双欢愉档案验证火花和爻光共享阿哈时刻。海瑟音的结界按每个目标及每个伤害批次限制追加笔数，持续伤害配队复验四位原施加者、卡芙卡引爆与海瑟音的逐击贡献。[配队与档案回归](../src-tauri/src/character_archive_tests.rs)
- 刻律德菈的爵位在原战技前结算奇袭，复制段保留出手者并将贡献计入刻律德菈。那刻夏的普通与复制战技各自触发追加战技，白厄境界中的死星天裁只支付一次毁伤。长夜月按实际生命损失与施放条件累计忆质，「长夜」独立排程并提供队伍忆灵增幅。低等级档案复验奇袭、长夜与迷迷的独立来源及归因守恒。[配队与档案回归](../src-tauri/src/character_archive_tests.rs)
- 丹恒·腾荒将龙灵绑定至一名同袍。龙灵独立排程，行动时净化并叠盾，终结技强化后为同袍追加对应属性伤害。星期日选中同袍时可推进龙灵的行动值。低等级档案复验带小数的护盾倍率，分别核对腾荒的追击与同袍的附伤来源及贡献。[配队与档案回归](../src-tauri/src/character_archive_tests.rs)
- 昔涟以追忆触发终结技，德谬歌使用赠诗与故事，并按十三位黄金裔的专属技能结算。保存档案逐一复验十三组配队的独立伤害来源和贡献。Saber 按实际回能计算炉心消耗并储存超出上限的能量，终结技逐段弹射并开启强化普攻。联动回归核对昔涟直接补满 Saber 能量时的溢出余额，另复验长夜月的两笔忆质与龙灵的独立强化次数。[配队与档案回归](../src-tauri/src/character_archive_tests.rs)
- Archer 的回路连接最多连续施放五次战技，每次支付战技点并复用当前行动值。结算后检查下一击是否可支付，支持主动结束回路。追击在实际执行时扣除充能，刻律德菈的战技复制保留原施放计数与一次支付。冻结档案验证低等级、满技能和满星魂的连续战技、终结技、追击及损益归因。[档案回归](../src-tauri/src/character_archive_tests/archer.rs)
- 大丽花的结界允许符合条件的超击破来源在敌方保有韧性时结算，并将由结界增加的伤害记为大丽花的贡献。共舞者触发天赋追击，各弹射段和整次追击分别结算对应超击破。冻结档案复验 E0 和 E6 击破队的弱点植入、固定削韧与伤害归因。[档案回归](../src-tauri/src/character_archive_tests/dahlia.rs)
- 星神★阿哈使用独立愿力，终结技触发欢愉命途升格，改变共享笑点的消费回收和爻光、火花的对应效果。阿哈时刻按实际非真实伤害计算尾段真实伤害，并分摊给在场敌人。低技能等级档案复验三人同队的升格事件和逐击归因，有限敌人夹具另验欢愉负债。[档案回归](../src-tauri/src/character_archive_tests.rs)
- 远坂凛按战技点实际支出和恢复累积宝石，强化战技逐段花费宝石弹射，E1 另有影子宝石。与 Archer 的连携追加攻击分别保留两人的伤害来源，连携恢复的战技点在 Archer 的回路续射判断前到账。冻结档案复验低等级、满技能及满星魂的连携与战技增益归因。[档案回归](../src-tauri/src/character_archive_tests/rin.rs)
- 欢愉开拓者的终结技可为指定队友独立触发欢愉技。升格期间选中星神★阿哈时，按开拓者先出手、阿哈后出手的时序结算，并以开拓者本次欢愉技伤害分摊真伤。毁灭开拓者的单体和扩散终结技共用能量，按存活目标分布选择范围。冻结档案复验两种形态的有效技能等级、星魂和伤害归因。[欢愉档案回归](../src-tauri/src/character_archive_tests/trailblazer_elation.rs) · [毁灭档案回归](../src-tauri/src/character_archive_tests/trailblazer_destruction.rs)
- 存护开拓者在受击后积累灼热意志，强化普攻消耗四层，终结技赠送一次免消耗的强化普攻。有限敌人夹具复验战技嘲讽、效果抵抗、受击减伤及护盾吸收，冻结档案核对双倍率终结技、星魂和逐击归因。[档案回归](../src-tauri/src/character_archive_tests/trailblazer_preservation.rs)
- 吉尔伽美什在兴致达到门槛后，由自动普攻切换至零战技点战技。与 Saber 的连携分别记录两人的伤害，Saber 下一次终结技的倍率增量归吉尔伽美什。冻结档案复验低等级与满星魂配队的兴致、实付能量返还、连携次数和伤害归因。[档案回归](../src-tauri/src/character_archive_tests/gilgamesh.rs)
- 真珠按好活当赏消耗抵挡值，并在四欢愉配队为底本安排独立额外回合，升格时按对应比例回收临时资源。知更鸟·晴歌按气氛值召集乐手，【Fever】期间独立推进忆灵与倒计时，保留各自的伤害来源及助攻。冻结档案复验低等级和满星魂配队、末段真实伤害与逐击归因。[真珠档案](../src-tauri/src/character_archive_tests/pearl.rs) · [晴歌档案](../src-tauri/src/character_archive_tests/robin_sunny.rs)

## 后续归因扩展

- 新增增伤、减防和抗性变化等直接乘区时，按提供者保留效果，复用同一次命中的反事实重算。多位角色同时影响同一命中时，可以按加入顺序平均各角色的边际增量，并确定整数舍入后的分配顺序。
- 拉条、回能和削韧效率暂不归因。这些效果照常影响模拟结算。将来可在固定配装、初始条件、随机样本与可比较的打法策略下重跑整段战斗，评估额外行动、终结技或提前击破带来的差额。整段战斗归因要先定义反事实的策略与随机数对齐方式，并处理与单击直接归因重叠的伤害。
