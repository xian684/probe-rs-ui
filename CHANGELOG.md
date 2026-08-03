# 更新日志

本项目的所有重要变更均记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [v0.9.0] - 2026-08-04

### 新增
- **ARM 在线索引**（中央面板新标签页）：搜索 [Keil.pidx](https://www.keil.com/pack/Keil.pidx) 公共索引，按关键字检索芯片包并直接下载、生成芯片描述
  - 三个操作按钮：**下载**（仅保存 `.pack`）、**添加到外部芯片包**（生成并注册）、**生成芯片描述文件**（生成 YAML 落盘）
  - 通过内置 target-gen 库实现，无需安装命令行工具
- **外部芯片包独立视图**：导入的芯片族不再混入内置三级菜单，在「外部芯片包」视图中按家族下拉 + 型号列表选择，可直接按型号连接
- **外部芯片包来源持久化**：历史导入的 YAML / CMSIS Pack 来源自动保存，下次启动自动恢复
- **删除外部芯片包**：家族下拉旁新增删除按钮，删除后启动恢复时自动跳过（手动重新导入则恢复）
- 高级芯片配置两个生成模式：**生成芯片描述** / **生成芯片描述并自动导入**
- 错误提示优化：高频失败（连接/烧录/擦除/复位）附第二行「提示:」操作建议

### 变更
- target-gen 从中央标签页迁移至左侧「高级芯片配置」面板，与「手动指定目标」互斥切换
- 外部芯片包来源去重：同名家族并集合并新型号，避免重复添加
- 手动选型区内置/外部芯片包互斥切换；连接成功后清除「自动识别失败」提示
- 错误提示文案规范化（访问目标内核、读取文件格式等）

### 重构
- 大文件拆分：`app.rs` → `src/app/`（mod/settings/events/actions），`device.rs` → `src/panels/device/`（mod/manual/target_gen/info）
- `handle_event`（300+ 行单函数）拆为 16 个 `on_xxx` 方法；worker 命令分发拆为 15 个 handler（引入 `Ctx`/`FlashRequest`）
- 烧录面板拆出进度条渲染（`flash_progress.rs`）与区块方法；外部芯片包视图拆至 `external.rs`
- 项目无 300+ 行单函数

### 修复
- egui 默认字体缺失的图标码位（`⚙️ 🧩 ⬇ ➕`）改为 emoji 主区字符
- 左侧面板输入框挤出同行按钮：改用 `horizontal_wrapped` + 固定输入框宽度
- 三级菜单 / 型号列表高度压缩，使「按型号连接」按钮保持在可视区内

## [v0.8.0] - 2026-07

### 变更
- 配置文件隐藏：写入后通过 Windows API 将 `config.toml` 标记为隐藏

## [v0.7.0] - 2026-07

### 变更
- 启动时自动选中上次使用的芯片型号（按已保存配置反查品牌与系列）

## [v0.6.0] - 2026-07

### 新增
- 配置持久化：常用设置与窗口位置保存到 `config.toml`，下次启动自动恢复

### 变更
- 默认窗口尺寸缩小至 1280×760，修复窗口尺寸钳制导致的界面过大/滚动条问题
- 左栏目标信息框固定显示并收窄设备检测区，日志移入底部全局面板

## [v0.5.0] - 2026-06

### 新增
- RTT 通道切换：上行显示 / 下行发送通道可选
- 深色 / 浅色主题切换（跟随系统）

### 变更
- RTT 日志移入中央面板标签，移除独立的启用按钮

## [v0.4.0] - 2026-06

### 新增
- **内存查看器**：任意地址读取/写入与十六进制转储，中央面板在固件烧录与内存视图间切换

## [v0.3.0] - 2026-05

### 新增
- **读取固件**：按地址范围读取 Flash 导出为 `.bin`
- `.bin` 烧录可配置基地址
- Windows 版本信息与图标嵌入

### 重构
- `panels` 提升为 crate 根级模块，拆分 app 为 panels 子模块

## [v0.2.0] - 2026-05

### 新增
- 品牌 → 系列 → 型号三级联动选型
- RTT 日志监控（开关控制、跨平台字体）
- 复位期间连接（Under Reset）
- 中英文界面切换
- 识别并烧录无扩展名的 Rust ELF 编译产物
- macOS / Linux 构建工作流（GitHub Actions）
- 按钮图标与窗口图标

### 变更
- 以 GUI 子系统编译，启动时不再弹出终端窗口
- 芯片选择改为双列列表、面板固定宽度

## [v0.1.0] - 2026-04

### 新增
- 初始版本：探针扫描、目标识别（自动/手动）、固件烧录（ELF/HEX/BIN/UF2）
- 烧录选项（整片擦除、校验、保留未写字节、复位运行）与实时进度条

[Unreleased]: https://github.com/xian684/probe-rs-ui/compare/v0.9.0...HEAD
[v0.9.0]: https://github.com/xian684/probe-rs-ui/releases/tag/v0.9.0
[v0.8.0]: https://github.com/xian684/probe-rs-ui/releases/tag/v0.8.0
[v0.7.0]: https://github.com/xian684/probe-rs-ui/releases/tag/v0.7.0
[v0.6.0]: https://github.com/xian684/probe-rs-ui/releases/tag/v0.6.0
[v0.5.0]: https://github.com/xian684/probe-rs-ui/releases/tag/v0.5.0
[v0.4.0]: https://github.com/xian684/probe-rs-ui/releases/tag/v0.4.0
[v0.3.0]: https://github.com/xian684/probe-rs-ui/releases/tag/v0.3.0
[v0.2.0]: https://github.com/xian684/probe-rs-ui/releases/tag/v0.2.0
[v0.1.0]: https://github.com/xian684/probe-rs-ui/releases/tag/v0.1.0
