# M01 · Windows 与离线验收基线

## 首发平台

首发验收以 Windows 11 25H2 x64（Home 或 Pro）和 Microsoft Edge WebView2 为目标，M08 按届时仍受支持的 Windows 11 版本补验。当前不声明支持 Windows 10 或 Arm64。Windows CI 的 Server 镜像可验构建，不能代替在 Windows 11 客户端点一次桌面连接。[Microsoft Windows 11 发布信息](https://learn.microsoft.com/en-us/windows/release-health/windows11-release-information)

| 项目 | M01 基线 |
| --- | --- |
| 性能参考机 | 本机，Intel Core i7-14700KF（28 逻辑处理器），32 GiB RAM，Windows 11 25H2 x64，系统构建 26200.9457 |
| 开发工具 | Rust 1.95.0 / x86_64-pc-windows-msvc；Node.js 24.13.1 / npm 11.14.1；实际依赖见两个锁文件 |
| 桌面技术 | Rust 负责本地业务，Tauri 2 和 WebView2 负责 Windows 窗口与通信，React、TypeScript 和 Vite 负责轻量界面。M01 只包含一条只读诊断命令，没有数据库、后台任务或联网运行要求。 |
| 构建与启动 | 干净源码副本的 Windows x64 Release 构建与真实桌面调用通过，见下文；复现命令见[开发说明](../README.md)。 |

## 离线合成样例

[共享样例](../contracts/battle-session.v1.example.json) 固定关卡 `fixture.two_targets` 和种子 `42`，包含四个角色形态与一波两个具有唯一实例 ID 的敌方目标。`fixture.*` 标识下的规则、数据和随机算法版本用于验证交换格式。样例及版本字段无需官方客户端、账号或网络连接即可验证。

验收时样例文件 SHA-256：`07b7fff5d3b387f1c5d04588f47badb192feec83579bd5a6cabf9ddd1f712829`。

## M05–M07 性能约定

在上述参考机上用同一 Release 构建、版本化关卡、队伍与库存输入及固定种子复跑。用随结果归档的输入文件与哈希标识样本。M05–M06 首次运行前冻结可用的规则与数据、具体配置和评分预算；比较时保持这些条件一致。

| 样本 | 定义 |
| --- | --- |
| M05 小场景 | 以上述四人双目标夹具的初始输入作为样本，固定种子 42，前三个玩家决策点，遍历全部合法指令用于精确模式对照。 |
| M06 小库存 | 同一小场景；四人装备不可共享，除丹恒的鞋和开拓者的鞋外全部锁定。鞋库存三件，其中一件未装备；保存实际攻击和速度数据与锁定表，独立枚举所有可行分配。 |
| M06–M07 中等库存 | 四人六槽，四个隧洞槽位每槽 16 件、两个位面槽位每槽 12 件，共 88 件去重实例；每名角色锁定一件现有装备，并保留未装备件。采用固定的可完整回放关卡，固定前 3 个关卡轮次或更早结束时的总伤害除以固定 3 轮；候选共享 16 个种子（42–57）及同一打法预算。 |

计时走单调时钟：提交搜索至界面首次显示一个已完成评分、且符合库存与锁定约束的候选；发出取消请求至任务结束被确认；对搜索运行时的切换视图或滚动，从输入到第一帧可见反馈，分别记录。每项预热 3 次后独立测 30 次，P95 用最近秩第 29 个值；保留原始毫秒数据和单独记录的冷启动。分别测单线程与 8 个工作线程，记下开启事件回放和关闭详细日志的每秒完成场次、单场用时和进程峰值内存。M07 采用初始门槛：首个候选 P95 ≤ 5 秒、取消 P95 ≤ 500 毫秒、界面可见反馈 P95 ≤ 100 毫秒。M01 固定测量办法，尚未测得吞吐或延迟。

## 本地验证记录

2026-11-15，应用版本 `0.1.0`。首次构建报 `link.exe` 缺失；安装 Visual Studio Build Tools 2022 `17.14.37906.3`（MSVC 链接器文件版本 `14.44.35230.0`，Windows SDK `10.0.26100.0`）后复验。安装器退出码为 `0`。工作目录未接入 Git；用新建的源码副本验证，复制时排除了 `node_modules`、`dist`、`target`、本地工具链缓存和 Tauri 生成目录。依赖按锁文件从本机缓存安装，尚无提交编号或 CI 结果。

| 命令 | 结果 |
| --- | --- |
| `npm.cmd ci --offline --no-audit --no-fund` | 从锁文件与本机缓存重装通过。 |
| `npm.cmd run check` | Prettier、ESLint、TypeScript 类型检查、4 项测试和 Vite 生产构建通过；测试只覆盖前端适配器与共享样例。 |
| `cargo fmt --all -- --check` | 通过。 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过。 |
| `cargo test --workspace --locked` | 通过；此版本尚无 Rust 测试用例。 |
| `npm.cmd run desktop:build` | 生成 `target/x86_64-pc-windows-msvc/release/star-rail-simulator.exe`；已核对 PE 文件头为 x64。 |
| 启动 Release 并点击「检测连接」 | 窗口从「等待检测。」更新为「连接成功 · 应用版本 0.1.0」，确认前端收到 `desktop_status` 命令结果。 |

Windows Release 可执行文件 SHA-256：`9d4ec10dfdf2ee87c533ae00b626aba23b92ec4ddead0496873abe01daafb0b3`。

锁文件 SHA-256：

- `Cargo.lock`：`a18d27804ceab3de51ab6fdb0778b5e32316d602126a7ce87ba291ce914a92a9`
- `package-lock.json`：`5f9536f221ebe7639ba2d9c408a4ced89352abc51d8ac867f5a02836574f6d57`

## 验收结果

2026-11-15，构建 `0.1.0` 按修订后的四项离线条件验收通过。证据来自本地构建、锁文件、合成样例及性能测量约定。

| 路线图验收项 | 结果与证据 |
| --- | --- |
| 1. Windows 构建和桌面通信 | 通过。当前参与复核的 27 份源码、配置和锁定文件与干净构建副本逐字节一致，Release 文件哈希及 x64 PE 标识吻合；真实窗口返回应用版本的结果见上表。 |
| 2. 本地检查与测试入口 | 通过。沿用同一构建输入的格式、lint、类型检查、Clippy 和 `cargo test` 结果，复验 `npm.cmd test` 为 4/4 通过。Windows CI 配置已提供，当前未接入 Git 或实际运行 CI。 |
| 3. M02 / M03 共享契约 | 通过。[契约](battle-contract.md) 与 [同一份样例](../contracts/battle-session.v1.example.json) 包含四个不同角色形态、两个可区分目标、冻结配置和版本字段。带指令的回放样例及从它得到的空指令初始输入都通过同一 JSON Schema；无版本字段、未知指令类型、用档案引用替代快照及越界种子均被拒绝。这里验证的是交换格式；实际接入和逐步指令合法性由 M02、M03 验收。 |
| 4. 离线合成样例和性能约定 | 通过。样例符合 JSON Schema，复核了四个不同角色形态、两个唯一目标及固定种子。参考机、样本规模、计时方法和阈值已登记；性能实测留给后续里程碑。 |
