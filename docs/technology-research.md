# 星穹铁道战斗模拟器：技术选型调研

调研日期：2026-11-15。已确定采用 Rust + Tauri；下文记录其他技术建议、选型依据与待验证事项。

采用 **Rust 战斗与优化核心 + Tauri 2 Windows 桌面壳**，建议用 TypeScript / React / CSS 构建界面、用 SQLite 保存本地档案。优先验证 Windows x64，最终以可离线安装的安装包交付。这个组合可以让 CPU 密集的搜索独立运行，同时用网页技术制作高度定制的配装编辑器、战斗时间轴和分析界面。项目吞吐仍需通过原型实测。

| 方案 | 适用场景 | 本项目的取舍 |
| --- | --- | --- |
| Rust + Tauri 2 + TypeScript / React | 大量后台运算，配合自定义动画、时间轴和图表 | 推荐。多一套工具链，也要控制 WebView 与核心之间的通信量；安装时需处理 WebView2 依赖。[Tauri 进程模型](https://v2.tauri.app/concept/process-model/) · [Windows 安装包](https://v2.tauri.app/distribute/windows-installer/) |
| C# / .NET 10 + WinUI 3 | Windows 原生控件、Fluent 设计，界面和核心可用同一主语言 | 强备选。本机已装 .NET 10 SDK；.NET 有成熟的 CPU 并行工具。如果实测已达标且团队优先降低维护成本，可以选它。高度定制的视觉仍需要专门设计。[WinUI 3](https://learn.microsoft.com/en-us/windows/apps/winui/) · [.NET 并行库](https://learn.microsoft.com/en-us/dotnet/standard/parallel-programming/task-parallel-library-tpl) |
| C++ + Qt Quick / QML | 团队熟悉 C++ / Qt，并考虑原生绘制与跨平台 | 也能胜任；当前没有证据表明它能让本项目的实际搜索快到足以抵消额外的开发与发行成本。用到的 Qt 模块需分别核对许可。[Qt Quick 渲染](https://doc.qt.io/qt-6/qtquick-visualcanvas-scenegraph.html) · [Qt 许可](https://doc.qt.io/qt-6/licensing.html) |

调研时本机已检测到 Node 和 .NET 10 SDK，PATH 未发现 Rust/Cargo。开始原型前按 [Tauri 官方 Windows 前置条件](https://v2.tauri.app/start/prerequisites/)检查 Rust、Microsoft C++ Build Tools 和 WebView2。调研阶段未安装依赖，也未运行语言性能基准。

## 全局角色配置

- 应用内每个可选角色形态有独立的全局档案，以角色 ID 区分。角色等级、行迹、星魂、光锥与叠影、遗器配装集中管理；可以有多套预设和一套当前配置。队伍引用角色 ID，编辑当前配置后，所有引用该角色的队伍在下一次开战时读取新配置。
- 开战或启动搜索时冻结配置快照，记录规则版本、敌人、初始条件和随机种子。改动全局档案不会改变正在运行的战斗或以前的回放。优化只使用临时候选；由用户查看差异并应用，才修改全局档案。
- 真实库存模式按物品实例 ID 建模，同一场战斗中每件实际遗器或光锥只能装备一次，并允许锁定装备。理论配装模式与库存模式分开。SQLite 只负责持久化，模拟开始时加载，搜索过程中不逐次读写数据库。[SQLite 本地应用场景](https://www.sqlite.org/whentouse.html)

## 搜索与性能

手动战斗、脚本回放、打法搜索、配装搜索共用同一个确定性战斗核心。核心接收当前状态和合法指令，推进到下一个决策点；面板和回放可以生成详细事件，批量搜索直接返回评分。优化在后台用 CPU 多核并行，支持取消和分批报告进度；WebView 不接收每个候选的中间状态。[Rayon 并行迭代器](https://docs.rs/rayon/latest/rayon/iter/index.html) · [Tauri 通信指南](https://v2.tauri.app/develop/calling-frontend/)

1. **打法优化：**固定队伍和配装，从合法技能、目标、终结技时机搜索。小场景穷举校验，较大场景可用束搜索保留有限数量的高分分支。对于有随机性的战斗，策略只能根据已观察到的状态选择操作。同一批候选在共同的随机样本下比较，领先方案用额外样本复核。近似搜索的结果是“预算内找到的最佳方案”，不承诺全局最优。[束搜索的最优性限制](https://www.ics.uci.edu/~dechter/publications/r243.pdf)
2. **配装优化：**先固定队伍和战斗条件，按库存占用、锁定装备等约束筛选组合，从整队角度保留有价值的速度档、套装和资源搭配；对入围组合重新搜索打法。避免按单角色伤害先挑满四套，否则可能漏掉让辅助先动、让主输出多行动一次的组合。
3. **评分：**建议默认用固定关卡、敌人配置和前 N 个关卡轮次内的全队有效伤害除以 N；关卡轮次统一计时，不和某个角色的行动次数混用。可约定不计溢出伤害，通关场景同时展示通关所用行动值，方便区分提前击杀的方案。结果展示轮次、随机样本与限制条件。

## 原型验证

- 用同一套四人合成夹具和固定输入，比较 Release 下单线程、多线程的每秒完整模拟次数、峰值内存、搜索到首批结果的时间。分别测无日志批量运行和含事件回放运行。需要客观比较语言时，在 Rust 和 C# 中只复现同一核心切片，核对输出一致后再对比吞吐。
- 用可穷举的小场景验证优化命中率，用更大库存和有随机分支的场景观察结果随时间预算的改善。记录 CPU、线程数、游戏规则版本和样本量。
- 同时做一张可交互的「角色配装 + 行动时间轴 + 全队 DPR 对比」界面，选用自定义视觉；后台搜索时检测滚动、动画和取消响应。在没有开发环境的 Windows 机器上验证离线安装和启动。

参照产品：[StarRailSim](https://starrailsim.com/import) 提供逐回合模拟和配装对比；[Fribbels](https://github.com/fribbels/hsr-optimizer) 提供遗器筛选和配装优化，适合参考操作流程。这些工具能说明需求可实现，不能当作本项目语言或性能的基准。
