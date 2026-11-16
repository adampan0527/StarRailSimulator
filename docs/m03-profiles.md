# M03 · 4.7.0 数据与角色档案

基线为 2026-11-16 国服 4.7.0。正式服发布表和中文索引逐 ID 对账得到 99 条角色源记录，五个开拓者命途合并男女数据后得到 **94 个角色形态**。角色强化前后共 **104 套技能配置**，包含 769 条技能记录、1,354 条可选或固有行迹记录、三条奇缘星魂和七个忆灵。另有 **172 张光锥、860 档叠影、790 条遗器记录、64 个遗器套装**。可升级技能的行迹节点折叠为逐级前置条件。 [逐 ID 对账](m03-coverage.json)

## 数据来源与离线重建

- [StarRailRes 4.7.0 固定修订](https://github.com/Mar-7th/StarRailRes/tree/03907e798b46f9eb109a1f882055cdf49ce3d374) 提供简中名称、说明和角色、光锥、遗器的索引。该仓库标注 AGPL-3.0。[源文件清单及 SHA-256](../scripts/m03/source-lock.json)
- [TurnBasedGameData 正式服修订 17180467](https://github.com/DimbreathBot/TurnBasedGameData/tree/dad9155eb00fdeda31fb8456507c53590d49071f) 提供正式服常规与联动角色清单、角色强化、忆灵、技能及星魂参数、精确词条档位。奇缘星魂说明从同版简中文本表按键解析，黄泉、丹恒·饮月和景元均可在普通 0 星魂时选用。[4.7 官方公告](https://www.hoyolab.com/article/47394097?reply=1)
- 角色与光锥的七个突破阶段按两份源表核对，上限、基础属性和成长率由正式服表给定。内部拆分技能保留各自的参数，并指向共享的可见说明。遗器按稀有度、槽位、等级、词条类型和可达档位校验。两处缺失的属性显示名和拆分技能的说明对应关系记录在独立补充表。 [补充表](../scripts/m03/patches.mjs) · [遗器稀有度与词条规则](https://www.prydwen.gg/star-rail/guides/relics)

```powershell
node scripts/m03/download.mjs
node scripts/m03/import.mjs
cargo run --offline --locked -p star-rail-profiles --example m03 -- audit .m03-data/4.7.0/catalog.json
cargo run --offline --locked -p star-rail-profiles --example m03 -- show .m03-data/4.7.0/catalog.json character.1308
```

下载脚本校验每个文件的 SHA-256。已下载的两份目录也可以直接传给 `import.mjs`，默认在 `.m03-data/4.7.0` 生成 `catalog.json` 和 `audit.json`。Rust 读取时再按实际内容校验 `data_version`。简中文本与数值随本地数据包保存，版本库保留对账 ID、源锁和合成夹具。M08 处理最终分发范围。

## Rust 接口和存档

- `Catalog` 提供形态、技能配置、基础属性、光锥和遗器查询。角色默认 80 级、六阶突破、0 星魂、常规行迹按前置解锁，可训练技能默认取当前突破的基础上限。光锥装备时初始化为 80 级、S1。`effective_skill_level` 从保存的基础等级和当前星魂计算有效等级，附条件的技能等级保留适用条件供后续调用。
- `Archive` 管理角色档案、命名预设、当前方案和队伍。队伍只保存有序角色形态 ID。预设和当前选择在同一个 SQLite 事务内提交。保存出错时调用方可重试原草稿，重开时校验数据库结构和数据包版本。
- `FrozenTeam` v1 逐值保存队伍配装，同时记录游戏版本和数据版本。合成队伍快照可用现有契约夹具生成战斗会话 v1，供 M02 加载和重放。真实队伍快照交给后续战斗规则和 M04 界面接入。

## 验收命令

```powershell
$catalog = '.m03-data/4.7.0/catalog.json'
$demo = Join-Path '.m03-data' ("m03-demo-" + [guid]::NewGuid().ToString('N'))
cargo run --offline --locked -p star-rail-profiles --example m03 -- demo-write $catalog "$demo.sqlite" "$demo.json"
cargo run --offline --locked -p star-rail-profiles --example m03 -- demo-reopen $catalog "$demo.sqlite" "$demo.json"
cargo test --workspace --locked --offline
```

示例分别启动两个 Windows 进程。第一进程创建两队和黄泉 S1 快照，并把两个队伍共用的当前配装改为 S5、开启奇缘星魂。第二进程读取两队当前值及旧快照。集成测试在保存预设后、切换当前方案时注入 SQLite 错误，重开数据库后对比旧值，再提交同一份草稿。[应用层测试](../crates/profiles/tests/archive.rs)

## 2026-11-16 验收结果

| 验收项 | Windows x64 结果 |
| --- | --- |
| 数据重建与对账 | 从两份下载目录离线重建两次，目录与对账文件逐字节一致。提交的 JSON 对账与生成结果一致。数据版本为 `sr.4.7.0.m03.dce40f8d031ea2b3272537b2dd4041510e65e163db773a9d1a7cda7a509b1ff9`。 |
| 全量只读查询 | Rust 审计通过 104 套技能配置的等级与突破组合，逐光锥读取五档参数。抽查存护三月七 80 级基础生命 1058.4、黄泉终结技由 10 级到 12 级且基础等级仍为 10，以及三条奇缘星魂在普通 0 星魂时的配置。 |
| 重启与旧快照 | 第二个进程的两个队伍均读取黄泉 E0/S5 和启用的奇缘星魂，旧快照保持 E0/S1 与原设置。旧快照前后 SHA-256 均为 `46043eca2a61ceb37fe6f19cef5eb9cd5624124f2ac4c87736be484d0900a35e`。 |
| 失败恢复与回归 | 6 项 M03 集成测试、9 项 M02 集成测试、Rust 格式、Clippy 和 `npm.cmd run check` 均通过。存档版本不兼容、篡改数据包、非法槽位与词条、事务中途失败和重试均有断言。M03 生成的 v1 合成输入能从 M02 载入并执行共享样例的指令。 |
