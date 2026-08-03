# Probe-rs 烧录工具

基于 [probe-rs](https://probe.rs/) 与 [egui](https://github.com/emilk/egui) 开发的嵌入式固件图形化烧录工具。

支持自动检测调试探针、识别目标芯片、手动指定芯片型号，以及一键烧录、全片擦除、复位等操作，无需记忆命令行参数。

> English docs: [README-en.md](README-en.md)

## 功能特性

- 🔍 **探针自动扫描**：自动枚举已连接的调试探针（ST-Link、J-Link、DAPLink / CMSIS-DAP 等）。
- 🎯 **目标自动识别**：支持自动识别芯片型号（ST-Link / J-Link 等可自报型号的探针）。
- 🔌 **连接方式可选**：支持「正常连接」与「复位期间连接」（Under Reset），对应 STM32 BOOT0/BOOT1 启动配置，便于在目标程序干扰 SWD 或需从系统存储器启动时连接。
- 🧩 **手动指定芯片**：内置 probe-rs 全部芯片型号库，按品牌 → 系列 → 具体型号三级联动选择，支持关键字实时搜索；可在「内置芯片包 / 外部芯片包」两个视图间切换（适用于 DAPLink / CMSIS-DAP 等无法自动识别的探针）。
- 📦 **高级芯片配置**：左侧内置 [target-gen](https://probe.rs/docs/tools/target-gen/) 库，选择本地 CMSIS 包（`.pack` / `.pdsc` / `.zip`）即可生成芯片描述；支持「生成芯片描述」与「生成芯片描述并自动导入」两种模式，同名芯片族自动去重（并集合并新型号），也可加载现成的 YAML 描述文件。
- 📦 **外部芯片包**：通过加载 YAML 或从 CMSIS 包生成导入的芯片**独立于内置三级菜单**，在手动选型区的「外部芯片包」视图中按家族下拉 + 型号列表选择，并可一键按型号连接。
- 📁 **固件自动定位**：选择项目文件夹后自动扫描常见构建产物（cargo `target/`、Keil `Objects/`、CubeIDE `Debug/`、CMake `build/` 等），自动选中最佳固件，多候选可下拉切换。
- ⚡ **固件烧录**：支持 `.elf` / `.axf` / `.hex` / `.bin` / `.uf2` 格式，并可识别无扩展名的 Rust ELF 编译产物。
- ⚙️ **可配置烧录选项**：全片擦除、烧录后校验、保留未写入字节、烧录后复位运行。
- 🗑️ **全片擦除 / 目标复位**：一键操作。
- 💾 **读取固件**：按地址范围读取 Flash 导出为 `.bin`（连接后自动填充芯片 Flash 区域）。
- 📊 **进度显示**：擦除、编程、校验等操作进度条实时更新；总大小未知的操作（如全片擦除）显示旋转指示。
- 📡 **RTT 日志监控**：默认关闭；启用后底部面板实时显示目标 RTT 上行通道输出（多通道自动标注），并支持向目标下行通道 0 发送数据（回车或按钮）。
- 🌐 **中英文界面**：自动加载 Windows、macOS 与 Linux 上可用的 CJK 系统字体，顶栏可切换中文 / English。
- 🖼️ **图形化图标**：操作按钮带图标，程序窗口使用芯片样式图标。

## 系统要求

- Windows / Linux / macOS
- Rust 1.97 或更高版本（构建时）
- 支持 SWD / JTAG 的调试探针（ST-Link、J-Link、DAPLink、CMSIS-DAP 等）

## 构建

```bash
cargo build --release
```

生成的可执行文件位于 `target/release/probe-rs-ui`（Windows 为 `probe-rs-ui.exe`），可直接双击运行。

### Windows 构建包

推送至 `master` 后，GitHub Actions 会自动构建 Windows x86_64 的 `.zip` 包（可在 [Actions](https://github.com/xian684/probe-rs-ui/actions/workflows/build-packages.yml) 页面下载 artifacts）。推送形如 `v1.0.0` 的 tag 时，还会自动创建 GitHub Release 并附带该 zip。

## 使用方法

1. 连接调试探针与目标芯片，启动程序后自动扫描探针。
2. 点击 **自动识别目标**；若探针不支持自动识别（如 DAPLink），在左侧 **手动指定目标芯片** 中按品牌、系列选择具体型号后点击 **按型号连接**。
3. 若目标芯片不在内置库中，可在左侧切换到 **高级芯片配置**，选择厂商 CMSIS Pack（`.pack` / `.pdsc` / `.zip`）点击 **生成芯片描述并自动导入**（或先加载 YAML 描述文件）；随后在手动选型区切到 **外部芯片包** 视图，按家族下拉选择具体型号并点击 **按型号连接**。
4. 选择固件文件，或点击 **选择项目文件夹...** 让程序自动定位编译产物。
5. 按需勾选烧录选项，点击 **开始烧录**。

## 依赖版本

| 依赖 | 版本 |
| ---- | ---- |
| eframe / egui | 0.31 |
| probe-rs | 0.32 |
| target-gen | 0.32 |
| rfd | 0.17 |

## 项目结构

```
probe-rs-ui/
├── build.rs                 构建脚本：Windows 资源嵌入（图标、版本信息）
├── assets/
│   └── icon.ico             Windows 程序图标
└── src/
    ├── main.rs              程序入口：窗口配置、图标生成、字体加载与 eframe 启动
    │
    ├── app/                 应用状态中枢（ProbeUiApp，UI 线程持有）
    │   ├── mod.rs           状态结构、new()、日志方法、eframe::App 主循环（update）
    │   ├── settings.rs      配置持久化：apply_config / collect_config（config.toml 双向）
    │   ├── events.rs        后台事件分发：handle_event 将 WorkerEvent 应用到界面状态
    │   └── actions.rs       操作入口：烧录 / 内存读写 / 文件格式识别（含 parse_hex_bytes）
    │
    ├── panels/              界面面板渲染（egui，均为 ProbeUiApp 方法）
    │   ├── mod.rs           面板模块声明
    │   ├── top.rs           顶栏：标题、连接状态、主题 / 语言切换
    │   ├── device/          左侧设备检测面板
    │   │   ├── mod.rs       面板入口：探针选择、连接方式、自动识别、子面板切换
    │   │   ├── manual.rs    手动指定目标：内置/外部芯片包切换、搜索 + 三级联动 + 按型号连接
    │   │   ├── target_gen.rs 高级芯片配置：CMSIS Pack 生成芯片描述（生成/生成并导入）
    │   │   └── info.rs      目标信息框：连接后的芯片与内存映射展示
    │   ├── central.rs       中央面板：烧录 / 内存查看器 / RTT 标签切换
    │   ├── flash.rs         固件烧录视图：文件选择、烧录选项、进度条、读取固件
    │   ├── mem_panel.rs     内存查看器：任意地址读写与十六进制转储
    │   ├── rtt_panel.rs     RTT 日志视图：通道选择、收发、自动滚动
    │   └── log.rs           底部日志面板：全局面板，记录操作日志
    │
    ├── worker/              后台工作线程（probe-rs 会话仅在此线程持有，UI 永不阻塞）
    │   ├── mod.rs           公共类型（WorkerCommand/WorkerEvent 等）与 spawn 入口
    │   ├── run.rs           线程主循环：命令分发、RTT 轮询、连接结果回写
    │   ├── probe.rs         探针扫描、目标连接（自动/手动/复位期间）、复位
    │   ├── flash.rs         烧录、全片擦除、读取 Flash 导出 bin
    │   ├── memory.rs        内存读写
    │   ├── progress.rs      烧录进度回调 → WorkerEvent 进度事件映射
    │   └── target_gen.rs    target-gen 集成：CMSIS Pack → 芯片族 → YAML/注册
    │
    ├── chips.rs             内置芯片库枚举与品牌分组（BRAND_RULES 前缀表）
    ├── config.rs            便携式 config.toml 持久化（与 exe 同目录，隐藏属性）
    ├── firmware.rs          固件扫描与格式识别（ELF/HEX/BIN/UF2，ELF 魔数识别）
    ├── rtt.rs               RTT 会话生命周期与通道读写（worker 线程调用）
    ├── fonts.rs             CJK 字体加载（Windows/macOS/Linux 候选路径）
    └── i18n.rs              语言支持：Msg 枚举 + MSGS 文案表 + 占位符填充（中文/English）
```

**UI 与后端通信模型**：UI 线程（app/panels）与后台线程（worker）通过两条 mpsc 通道解耦——`WorkerCommand` 下行发送操作指令，`WorkerEvent` 上行回传结果；probe-rs 会话（`Session`）只在 worker 线程存活，任何耗时的烧录/擦除/读写都不会阻塞界面。

## 常见问题

**DAPLink 无法自动识别芯片？**
DAPLink / CMSIS-DAP 为通用调试协议，不向主机上报芯片型号，因此无法自动识别。请在左侧 **手动指定目标芯片** 中搜索并选择系列与具体型号后连接。

**找不到编译产物？**
点击 **选择项目文件夹...** 后，程序会递归扫描常见构建输出目录；若仍未找到，请确认固件为 `.elf` / `.hex` / `.bin` / `.uf2` 格式。
