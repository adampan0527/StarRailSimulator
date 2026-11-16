# StarRailSimulator

用 Rust 和 Tauri 开发的 Windows《崩坏：星穹铁道》离线战斗模拟器。桌面训练支持本地配装、组队、逐回合操作和历史回放，当前战斗范围为 M02 合成规则。开发进度见 [路线图](html_report/01_开发路线图/index.html)。

## Windows x64 开发

1. 安装 [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)，选择「使用 C++ 的桌面开发」，保留 x64 MSVC 和 Windows SDK；确认已安装 [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)。（见 [Tauri 前置条件](https://v2.tauri.app/start/prerequisites/#windows)。）
2. 安装 [Rustup](https://rustup.rs/)，使用 `x86_64-pc-windows-msvc`。进入项目目录运行 `rustup show`；`rust-toolchain.toml` 选择 Rust 1.95.0 和所需组件。
3. 安装 [Node.js 24.13.1](https://nodejs.org/en/download/archive/v24.13.1) 和 npm 11.14.1（必要时运行 `npm.cmd install --global --no-audit npm@11.14.1`）。在新 PowerShell 窗口中执行：

```powershell
npm.cmd ci --no-audit --no-fund
npm.cmd run check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm.cmd run roadmap:check
```

如在当前开发机复用项目内的 Rustup，不必改系统 PATH。在项目目录执行以下命令后再运行 Cargo 或桌面脚本：

```powershell
$m01Toolchain = Join-Path (Get-Location) '.local-toolchain'
$env:CARGO_HOME = Join-Path $m01Toolchain 'cargo'
$env:RUSTUP_HOME = Join-Path $m01Toolchain 'rustup'
$env:Path = "$(Join-Path $env:CARGO_HOME 'bin');$env:Path"
```

## 构建和验证桌面连接

开发时运行 `npm.cmd run desktop:dev`。只在浏览器打开前端无法调用桌面后端。

```powershell
npm.cmd run desktop:build
& .\target\x86_64-pc-windows-msvc\release\star-rail-simulator.exe
```

Release 构建产出 `.exe`，安装包留待 M08。需要检查前后端连接时，展开窗口底部的「桌面连接诊断」，用鼠标或 Tab、Enter 激活「检测连接」，应显示「连接成功 · 应用版本 0.1.0」。若报 `link.exe` 找不到，核对 C++ 工作负载和 Windows SDK，再打开新的终端重试。

Windows CI 配置使用相同检查命令。工程设计边界见 [战斗输入契约](docs/battle-contract.md) 和 [M01 基线](docs/m01-baseline.md)。

## 桌面训练

启动桌面应用后，编辑并保存训练配装，从四个角色中组队并选择靶机。战斗中的每条已接受指令自动保存，重启后可在「历史回放」查看或继续未结束的战斗。具体流程、存档路径和验证记录见 [M04 桌面训练与回放](docs/m04-desktop.md)。

## 运行离线战斗核心

```powershell
cargo run --locked -p star-rail-battle-core --example m02 -- replay
cargo test --locked -p star-rail-battle-core
```

回放输出合成战斗的状态和事件 JSON。规则、完整夹具和测量方法见 [M02 验收记录](docs/m02-core.md)，公式来源见 [战斗机制调研](docs/m02-combat-mechanics.md)。

## 导入 4.7.0 数据与验证档案

```powershell
node scripts/m03/download.mjs
node scripts/m03/import.mjs
cargo run --offline --locked -p star-rail-profiles --example m03 -- audit .m03-data/4.7.0/catalog.json
```

首次下载获取固定修订的源表。导入、角色查询和 SQLite 档案操作均可离线执行。跨进程重启演示、数据覆盖和接口说明见 [M03 验收记录](docs/m03-profiles.md)。
