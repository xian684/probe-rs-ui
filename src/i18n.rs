//! 界面语言与文案表：所有界面文案集中在此，按 key 取当前语言文本。
//!
//! 动态文案将占位符写入模板（统一使用匿名 `{}`），调用处用
//! `format!(lang.tr(Msg::X), args)` 填充。

/// 界面语言。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Zh,
    En,
}

/// 界面文案 key。每个变体对应 [`MSGS`] 表中的一条文案。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Msg {
    // ---- 顶栏 / 通用 ----
    AppTitle,
    ConnectedDot,
    NotConnectedDot,
    ThemeSystem,
    ThemeLight,
    ThemeDark,
    Log,
    Clear,
    LogEntries,
    Send,
    All,
    // ---- 设备检测 ----
    DeviceDetection,
    ProbeLabel,
    NotSelected,
    Rescan,
    ScanningProbes,
    Scanning,
    ConnectionMode,
    BootNormal,
    BootUnderReset,
    BootModeHint,
    AutoDetectTarget,
    AutoDetecting,
    Disconnect,
    Disconnected,
    Connecting,
    ManualTargetSel,
    AutoDetectFailedHint,
    ManualTargetHint,
    LoadChipFile,
    LoadingChipFile,
    GenerateFromPack,
    GeneratingFromPack,
    SearchModel,
    SearchHint,
    SelectedChip,
    Brand,
    Family,
    Variant,
    NoMatchingBrand,
    NoMatchingFamily,
    NoMatchingVariant,
    SelectFamilyFirst,
    ConnectByModel,
    ConnectingTo,
    TargetInfo,
    ChipModel,
    Arch,
    CoreCount,
    Core,
    MemoryMap,
    TargetNotConnected,
    BrandOther,
    BrandArm,
    BrandRiscv,
    BrandExternal,
    // ---- 高级芯片配置（左侧 target-gen 面板） ----
    AdvancedChipConfig,
    AdvancedChipConfigHint,
    TgInput,
    TgInputHint,
    TgBrowseFile,
    TgBrowseDir,
    TgOutputDir,
    TgOutputDirHint,
    TgOnlySupported,
    TgOnlySupportedHint,
    TgGenerate,
    TgGenerateConnect,
    TgGenerating,
    TgResult,
    TgVariants,
    TgInputMissing,
    TgNoSupportedFamily,
    TgSerializeFailed,
    TargetsGenerated,
    TargetFileWritten,
    TgLoadedToSelection,
    // ---- 固件烧录 ----
    FirmwareFlashing,
    FirmwareFile,
    FirmwareHint,
    Browse,
    FirmwareImage,
    SelectProjectFolder,
    ScanningProject,
    FileFormat,
    BaseAddress,
    ScanningRoot,
    ProjectFirmware,
    NoProjectFirmware,
    SelectedFirmware,
    ChipEraseBeforeFlash,
    VerifyAfterFlash,
    KeepUnwritten,
    ResetAndRun,
    FlashBtn,
    EraseAllBtn,
    ErasingAll,
    ResetTargetBtn,
    ResettingTarget,
    ReadFirmwareTitle,
    Range,
    Bytes,
    SizeKb,
    ReadFirmwareBtn,
    StartingRead,
    // ---- 内存查看器 ----
    MemoryViewer,
    CustomAddress,
    Region,
    Address,
    BytesCount,
    ReadBtn,
    NotReadYet,
    WriteMemory,
    Data,
    HexHint,
    WriteBtn,
    // ---- RTT 日志 ----
    RttLog,
    StopBtn,
    RttRunning,
    StartBtn,
    StartingRtt,
    StoppingRtt,
    AutoScroll,
    ShowChannel,
    SendChannel,
    SendHint,
    // ---- 应用日志 / 操作反馈 ----
    LoadedChips,
    ScanningDebugProbes,
    NoProbes,
    DetectedProbes,
    ConnectedTo,
    OperationCompleted,
    NoFirmwareFound,
    AutoDetectedFirmware,
    UseOtherFirmware,
    ChipFileLoaded,
    PackGenerated,
    RttStartedSummary,
    MemoryReadDone,
    MemoryWritten,
    UnsupportedFormat,
    FlashingPath,
    ConnectFirst,
    ReadLenClamped,
    ReadingMemory,
    InvalidHexData,
    WritingMemory,
    // ---- 后台线程 ----
    NotConnectedErr,
    RttStoppedReconnect,
    RttStoppedFlash,
    RttStoppedErase,
    RttStoppedRead,
    RttStoppedManual,
    NoProbeIndex,
    OpenProbeFailed,
    ConnectTargetFailed,
    AutoDetectFailed,
    UnderResetFailed,
    ResetFailed,
    ReadFileFailed,
    PackGenFailed,
    PackNoChips,
    UnsupportedFileFormat,
    FlashFailed,
    EraseFailed,
    CreateFileFailed,
    GetCoreFailed,
    ReadFlashFailed,
    WriteFileFailed,
    ReadingFirmware,
    ReadOp,
    ReadMemoryFailed,
    WriteMemoryFailed,
    LayoutParsed,
    StartingOp,
    EraseLabel,
    ProgramLabel,
    VerifyLabel,
    FillLabel,
    // ---- RTT 后台 ----
    RttNotConnected,
    RttCoreFailedStart,
    RttStartedDetected,
    RttStartFailed,
    RttCoreFailedStopped,
    RttReadFailed,
    RttNotStarted,
    RttCoreFailed,
    RttDownWriteFailed,
    RttDownBufferFull,
    RttNoDownChannel,
}

impl Lang {
    pub fn is_en(&self) -> bool {
        matches!(self, Lang::En)
    }

    /// 取当前语言下 `msg` 对应的文案。
    pub fn tr(&self, msg: Msg) -> &'static str {
        let (_, zh, en) = MSGS
            .iter()
            .find(|(k, _, _)| *k == msg)
            .expect("every Msg must have an entry in MSGS");
        if self.is_en() {
            en
        } else {
            zh
        }
    }
}

/// 用参数依次填充模板中的 `{}` 占位符（模板由 [`Lang::tr`] 返回）。
///
/// `format!` 的格式串必须是字面量，无法直接使用运行时模板，故用简单替换实现。
pub fn fill(template: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut it = args.iter();
    let mut rest = template;
    while let Some(p) = rest.find("{}") {
        out.push_str(&rest[..p]);
        match it.next() {
            Some(a) => out.push_str(&a.to_string()),
            None => return out + &rest[p..],
        }
        rest = &rest[p + 2..];
    }
    out.push_str(rest);
    out
}

/// 便捷宏：`t!(lang, Msg::X, a, b)` 等价于用参数填充 `lang.tr(Msg::X)`。
#[macro_export]
macro_rules! t {
    ($lang:expr, $msg:expr $(, $arg:expr)*) => {{
        let _l = $lang;
        let _args = [$(&($arg) as &dyn std::fmt::Display),*];
        $crate::i18n::fill(_l.tr($msg), &_args)
    }};
}

/// 文案表：key → (中文, English)。动态文案使用匿名 `{}` 占位符。
const MSGS: &[(Msg, &str, &str)] = &[
    // ---- 顶栏 / 通用 ----
    (Msg::AppTitle, "Probe-rs 烧录工具", "Probe-rs Flasher"),
    (Msg::ConnectedDot, "● 已连接", "● Connected"),
    (Msg::NotConnectedDot, "○ 未连接", "○ Not connected"),
    (Msg::ThemeSystem, "跟随系统", "System"),
    (Msg::ThemeLight, "浅色", "Light"),
    (Msg::ThemeDark, "深色", "Dark"),
    (Msg::Log, "日志", "Log"),
    (Msg::Clear, "清空", "Clear"),
    (Msg::LogEntries, "共 {} 条", "{} entries"),
    (Msg::Send, "发送", "Send"),
    (Msg::All, "全部", "All"),
    // ---- 设备检测 ----
    (Msg::DeviceDetection, "设备检测", "Device Detection"),
    (Msg::ProbeLabel, "调试探针:", "Probe:"),
    (Msg::NotSelected, "未选择", "Not selected"),
    (Msg::Rescan, "重新扫描", "Rescan"),
    (Msg::ScanningProbes, "正在扫描探针...", "Scanning probes..."),
    (Msg::Scanning, "扫描中...", "Scanning..."),
    (Msg::ConnectionMode, "连接方式:", "Connection mode:"),
    (Msg::BootNormal, "正常连接", "Normal"),
    (Msg::BootUnderReset, "复位期间连接", "Under Reset"),
    (
        Msg::BootModeHint,
        "正常连接：从主 Flash 启动（BOOT0=0）；复位期间连接：保持目标复位直至连接完成（常用于 BOOT0 拉高从系统存储器启动等场景）",
        "Normal: boot from main flash (BOOT0=0); Under Reset: keep the target in reset until connected (e.g. booting from system memory with BOOT0 high)",
    ),
    (Msg::AutoDetectTarget, "自动识别目标", "Auto-detect Target"),
    (
        Msg::AutoDetecting,
        "正在自动识别目标芯片...",
        "Auto-detecting target chip...",
    ),
    (Msg::Disconnect, "断开", "Disconnect"),
    (Msg::Disconnected, "已断开连接", "Disconnected"),
    (Msg::Connecting, "正在连接目标...", "Connecting to target..."),
    (Msg::ManualTargetSel, "手动指定目标芯片", "Manual Target Selection"),
    (
        Msg::AutoDetectFailedHint,
        "（自动识别失败，请手动选择）",
        "(auto-detection failed, select manually)",
    ),
    (
        Msg::ManualTargetHint,
        "DAPLink / CMSIS-DAP 等探针需手动选择芯片型号",
        "DAPLink / CMSIS-DAP probes need manual chip selection",
    ),
    (
        Msg::LoadChipFile,
        "加载芯片描述文件...",
        "Load Chip File...",
    ),
    (
        Msg::LoadingChipFile,
        "正在加载芯片描述文件: {}",
        "Loading chip description: {}",
    ),
    (
        Msg::GenerateFromPack,
        "从 CMSIS 包生成...",
        "Generate from CMSIS Pack...",
    ),
    (
        Msg::GeneratingFromPack,
        "正在从 CMSIS 包生成芯片描述: {}",
        "Generating chip description from CMSIS pack: {}",
    ),
    (Msg::SearchModel, "搜索型号:", "Search:"),
    (Msg::SearchHint, "如 stm32f103 / nrf52840", "e.g. stm32f103 / nrf52840"),
    (Msg::SelectedChip, "已选型号: {}", "Selected: {}"),
    (Msg::Brand, "品牌", "Brand"),
    (Msg::Family, "系列", "Family"),
    (Msg::Variant, "具体型号", "Variant"),
    (Msg::NoMatchingBrand, "未找到匹配的品牌", "No matching brand"),
    (Msg::NoMatchingFamily, "无匹配系列", "No matching family"),
    (
        Msg::NoMatchingVariant,
        "该系列下无匹配型号",
        "No matching variant in this family",
    ),
    (
        Msg::SelectFamilyFirst,
        "请先在左侧选择芯片系列",
        "Select a chip family on the left first",
    ),
    (Msg::ConnectByModel, "按型号连接", "Connect by Model"),
    (Msg::ConnectingTo, "正在连接 {} ...", "Connecting to {} ..."),
    (Msg::TargetInfo, "目标信息", "Target Info"),
    (Msg::ChipModel, "芯片型号: {}", "Chip: {}"),
    (Msg::Arch, "架构: {}", "Architecture: {}"),
    (Msg::CoreCount, "核心数量: {}", "Cores: {}"),
    (Msg::Core, "  核心 {}: {}", "  Core {}: {}"),
    (Msg::MemoryMap, "内存映射:", "Memory Map:"),
    (Msg::TargetNotConnected, "尚未连接目标", "Not connected"),
    (Msg::BrandOther, "其他", "Other"),
    (Msg::BrandArm, "ARM 通用", "ARM Generic"),
    (Msg::BrandRiscv, "RISC-V 通用", "RISC-V Generic"),
    (Msg::BrandExternal, "外部芯片包", "External Pack"),
    // ---- 高级芯片配置（左侧 target-gen 面板） ----
    (Msg::AdvancedChipConfig, "高级芯片配置", "Advanced Chip Config"),
    (
        Msg::AdvancedChipConfigHint,
        "加载本地 CMSIS Pack（.pack / .pdsc / .zip 或解压目录），自动生成芯片描述，可直接生成后连接目标",
        "Load a local CMSIS Pack (.pack / .pdsc / .zip or unzipped dir), auto-generate chip descriptions, and optionally connect to the target",
    ),
    (Msg::TgInput, "输入:", "Input:"),
    (
        Msg::TgInputHint,
        "选择 .pack / .pdsc / .zip 文件，或包含 .pdsc 的解压目录",
        "Pick a .pack / .pdsc / .zip file, or an unzipped directory containing .pdsc",
    ),
    (Msg::TgBrowseFile, "文件...", "File..."),
    (Msg::TgBrowseDir, "目录...", "Dir..."),
    (Msg::TgOutputDir, "输出目录:", "Output dir:"),
    (
        Msg::TgOutputDirHint,
        "可选：生成的 <family>.yaml 将写入此目录",
        "Optional: generated <family>.yaml files will be written here",
    ),
    (Msg::TgOnlySupported, "仅生成已支持的芯片族", "Only supported families"),
    (
        Msg::TgOnlySupportedHint,
        "勾选后仅保留 probe-rs 内置支持芯片族的 target 定义，减少无关文件",
        "When checked, only targets for probe-rs built-in supported families are kept",
    ),
    (Msg::TgGenerate, "生成芯片描述", "Generate"),
    (Msg::TgGenerateConnect, "生成并连接", "Generate & Connect"),
    (
        Msg::TgGenerating,
        "正在从 CMSIS Pack 生成 target 定义，请稍候...",
        "Generating target definitions from the CMSIS pack...",
    ),
    (Msg::TgResult, "已生成的芯片族:", "Generated families:"),
    (Msg::TgVariants, "个型号", "variants"),
    (
        Msg::TgInputMissing,
        "输入路径不存在: {}",
        "Input path does not exist: {}",
    ),
    (
        Msg::TgNoSupportedFamily,
        "该包中没有 probe-rs 已支持的芯片族（可取消勾选『仅生成已支持的芯片族』后重试）",
        "No probe-rs supported family found in the pack (uncheck 'Only supported families' and retry)",
    ),
    (
        Msg::TgSerializeFailed,
        "序列化芯片族 {} 失败: {}",
        "Failed to serialize family {}: {}",
    ),
    (
        Msg::TargetsGenerated,
        "已生成 {} 个 target 定义文件",
        "Generated {} target definition file(s)",
    ),
    (
        Msg::TargetFileWritten,
        "已写入: {}（{} 个型号）",
        "Written: {} ({} variant(s))",
    ),
    (
        Msg::TgLoadedToSelection,
        "已将 {} 个芯片族加载到手动选型列表",
        "Loaded {} chip family(ies) into the manual selection list",
    ),
    // ---- 固件烧录 ----
    (Msg::FirmwareFlashing, "固件烧录", "Firmware Flashing"),
    (Msg::FirmwareFile, "固件文件:", "Firmware file:"),
    (
        Msg::FirmwareHint,
        "选择 .elf / .hex / .bin / .uf2 文件",
        "Select .elf / .hex / .bin / .uf2 file",
    ),
    (Msg::Browse, "浏览...", "Browse..."),
    (Msg::FirmwareImage, "固件镜像", "Firmware image"),
    (Msg::SelectProjectFolder, "选择项目文件夹...", "Select Project Folder..."),
    (
        Msg::ScanningProject,
        "正在扫描项目文件夹并自动识别固件: {}",
        "Scanning project folder and auto-detecting firmware: {}",
    ),
    (Msg::FileFormat, "文件格式: {}", "File format: {}"),
    (Msg::BaseAddress, "基地址:", "Base address:"),
    (Msg::ScanningRoot, "扫描中: {}", "Scanning: {}"),
    (Msg::ProjectFirmware, "项目固件:", "Project firmware:"),
    (Msg::NoProjectFirmware, "未选择项目固件", "No project firmware"),
    (Msg::SelectedFirmware, "已选择固件: {}", "Selected firmware: {}"),
    (Msg::ChipEraseBeforeFlash, "全片擦除后烧录", "Chip erase before flash"),
    (Msg::VerifyAfterFlash, "烧录后校验", "Verify after flash"),
    (Msg::KeepUnwritten, "保留未写入字节", "Keep unwritten bytes"),
    (Msg::ResetAndRun, "烧录后复位运行", "Reset and run after flash"),
    (Msg::FlashBtn, "开始烧录", "Flash"),
    (Msg::EraseAllBtn, "全片擦除", "Erase All"),
    (Msg::ErasingAll, "开始全片擦除...", "Erasing all flash..."),
    (Msg::ResetTargetBtn, "复位目标", "Reset Target"),
    (Msg::ResettingTarget, "正在复位目标...", "Resetting target..."),
    (Msg::ReadFirmwareTitle, "读取固件", "Read Firmware"),
    (Msg::Range, "范围:", "Range:"),
    (Msg::Bytes, "字节", "bytes"),
    (Msg::SizeKb, "大小: {} KB", "Size: {} KB"),
    (Msg::ReadFirmwareBtn, "读取固件...", "Read Firmware..."),
    (
        Msg::StartingRead,
        "开始读取: 0x{} - 0x{}",
        "Starting read: 0x{} - 0x{}",
    ),
    // ---- 内存查看器 ----
    (Msg::MemoryViewer, "内存查看器", "Memory Viewer"),
    (Msg::CustomAddress, "自定义地址", "Custom address"),
    (Msg::Region, "区域:", "Region:"),
    (Msg::Address, "地址:", "Address:"),
    (Msg::BytesCount, "字节数:", "Bytes:"),
    (Msg::ReadBtn, "读取", "Read"),
    (Msg::NotReadYet, "尚未读取，点击上方『读取』", "Not read yet; click 'Read' above"),
    (Msg::WriteMemory, "写入内存", "Write Memory"),
    (Msg::Data, "数据:", "Data:"),
    (Msg::HexHint, "十六进制字节，如 DE AD BE EF", "Hex bytes, e.g. DE AD BE EF"),
    (Msg::WriteBtn, "写入", "Write"),
    // ---- RTT 日志 ----
    (Msg::RttLog, "RTT 日志", "RTT Log"),
    (Msg::StopBtn, "停止", "Stop"),
    (Msg::RttRunning, "● 运行中", "● Running"),
    (Msg::StartBtn, "启动", "Start"),
    (
        Msg::StartingRtt,
        "正在启动 RTT（在目标 RAM 中扫描控制块）...",
        "Starting RTT (scanning target RAM for the control block)...",
    ),
    (Msg::StoppingRtt, "正在停止 RTT...", "Stopping RTT..."),
    (Msg::AutoScroll, "自动滚动", "Auto-scroll"),
    (Msg::ShowChannel, "显示通道:", "Show channel:"),
    (Msg::SendChannel, "发送通道:", "Send channel:"),
    (
        Msg::SendHint,
        "输入内容后回车或点击发送，写入目标下行通道",
        "Type and press Enter or click Send to write to a down channel",
    ),
    // ---- 应用日志 / 操作反馈 ----
    (
        Msg::LoadedChips,
        "已加载 {} 个内置芯片系列（{} 个品牌），可手动指定目标",
        "Loaded {} built-in chip families ({} brands); manual target selection is available",
    ),
    (Msg::ScanningDebugProbes, "正在扫描调试探针...", "Scanning debug probes..."),
    (
        Msg::NoProbes,
        "未检测到任何调试探针，请检查 USB 连接与驱动",
        "No debug probes detected. Check USB connection and drivers",
    ),
    (Msg::DetectedProbes, "检测到 {} 个调试探针", "Detected {} debug probe(s)"),
    (Msg::ConnectedTo, "已连接目标: {}", "Connected to target: {}"),
    (Msg::OperationCompleted, "操作成功完成", "Operation completed successfully"),
    (
        Msg::NoFirmwareFound,
        "在 {} 中未找到固件文件 (.elf / .hex / .bin / .uf2)",
        "No firmware file (.elf / .hex / .bin / .uf2) found in {}",
    ),
    (
        Msg::AutoDetectedFirmware,
        "自动识别到固件: {}（共 {} 个候选）",
        "Auto-detected firmware: {} ({} candidate(s))",
    ),
    (
        Msg::UseOtherFirmware,
        "如需使用其它固件，请在下方下拉列表中选择",
        "To use another firmware, pick one from the dropdown below",
    ),
    (
        Msg::ChipFileLoaded,
        "已加载芯片包: {}（{} 个型号），可在左侧手动选择",
        "Chip pack loaded: {} ({} variant(s)); select it manually on the left",
    ),
    (
        Msg::PackGenerated,
        "已从 CMSIS 包生成 {} 个芯片族，可在左侧手动选择",
        "Generated {} chip family(ies) from CMSIS pack; select them manually on the left",
    ),
    (
        Msg::RttStartedSummary,
        "RTT 已启动（上行 {}，下行 {}）",
        "RTT started ({} up, {} down)",
    ),
    (Msg::MemoryReadDone, "读取内存完成: {} 字节", "Memory read: {} bytes"),
    (Msg::MemoryWritten, "内存写入完成", "Memory written"),
    (
        Msg::UnsupportedFormat,
        "不支持的文件格式，请选择 .elf / .hex / .bin / .uf2 文件",
        "Unsupported file format. Choose a .elf / .hex / .bin / .uf2 file",
    ),
    (Msg::FlashingPath, "开始烧录: {}", "Flashing: {}"),
    (Msg::ConnectFirst, "请先连接目标芯片", "Connect to a target first"),
    (
        Msg::ReadLenClamped,
        "读取长度已限制为 {} 字节",
        "Read length clamped to {} bytes",
    ),
    (
        Msg::ReadingMemory,
        "正在读取内存: 0x{}，{} 字节",
        "Reading memory: 0x{}, {} bytes",
    ),
    (
        Msg::InvalidHexData,
        "数据格式错误：请输入十六进制字节（如 DE AD BE EF）",
        "Invalid data: enter hex bytes (e.g. DE AD BE EF)",
    ),
    (
        Msg::WritingMemory,
        "正在写入内存: 0x{}，{} 字节",
        "Writing memory: 0x{}, {} bytes",
    ),
    // ---- 后台线程 ----
    (
        Msg::NotConnectedErr,
        "尚未连接到目标芯片，请先自动识别目标",
        "Not connected to a target. Auto-detect or select the target first",
    ),
    (Msg::RttStoppedReconnect, "重新连接前已停止 RTT", "RTT stopped before reconnecting"),
    (Msg::RttStoppedFlash, "烧录期间已停止 RTT", "RTT stopped during flashing"),
    (Msg::RttStoppedErase, "擦除期间已停止 RTT", "RTT stopped during erase"),
    (Msg::RttStoppedRead, "读取期间已停止 RTT", "RTT stopped during read"),
    (Msg::RttStoppedManual, "RTT 已停止", "RTT stopped"),
    (Msg::NoProbeIndex, "未找到编号为 {} 的调试探针", "No debug probe with index {} found"),
    (Msg::OpenProbeFailed, "打开探针失败: {}", "Failed to open probe: {}"),
    (Msg::ConnectTargetFailed, "连接目标 {} 失败: {}", "Failed to connect to target {}: {}"),
    (
        Msg::AutoDetectFailed,
        "自动识别目标失败: {}。该探针可能不支持自动识别芯片（如 DAPLink/CMSIS-DAP），请在左侧『手动指定目标芯片』中搜索并选择芯片型号后重试",
        "Auto-detection failed: {}. The probe may not support auto-identification (e.g. DAPLink/CMSIS-DAP). Please search and select the chip model under 'Manual Target Selection' on the left and retry",
    ),
    (
        Msg::UnderResetFailed,
        "复位期间连接目标失败: {}。该探针可能不支持自动识别芯片（如 DAPLink/CMSIS-DAP），请在左侧『手动指定目标芯片』中搜索并选择芯片型号后重试",
        "Failed to attach under reset: {}. The probe may not support auto-identification (e.g. DAPLink/CMSIS-DAP). Please search and select the chip model under 'Manual Target Selection' on the left and retry",
    ),
    (Msg::ResetFailed, "复位失败: {}", "Reset failed: {}"),
    (Msg::ReadFileFailed, "读取文件失败: {}: {}", "Failed to read file: {}: {}"),
    (
        Msg::PackGenFailed,
        "生成芯片描述失败: {}",
        "Failed to generate chip description: {}",
    ),
    (Msg::PackNoChips, "未在包中找到可用芯片", "No usable chips found in the pack"),
    (
        Msg::UnsupportedFileFormat,
        "不支持的文件格式: .{}，请选择 .elf / .hex / .bin / .uf2 文件",
        "Unsupported file format: .{}. Choose a .elf / .hex / .bin / .uf2 file",
    ),
    (Msg::FlashFailed, "烧录失败: {}", "Flashing failed: {}"),
    (Msg::EraseFailed, "全片擦除失败: {}", "Chip erase failed: {}"),
    (Msg::CreateFileFailed, "创建文件失败: {}", "Failed to create file: {}"),
    (Msg::GetCoreFailed, "获取核心失败: {}", "Failed to get core: {}"),
    (
        Msg::ReadFlashFailed,
        "读取 Flash 失败 (0x{}): {}",
        "Failed to read flash (0x{}): {}",
    ),
    (Msg::WriteFileFailed, "写入文件失败: {}", "Failed to write file: {}"),
    (
        Msg::ReadingFirmware,
        "开始读取固件: 0x{} - 0x{}（{} KB）",
        "Reading firmware: 0x{} - 0x{} ({} KB)",
    ),
    (Msg::ReadOp, "读取", "Read"),
    (
        Msg::ReadMemoryFailed,
        "读取内存失败 (0x{}): {}",
        "Failed to read memory (0x{}): {}",
    ),
    (
        Msg::WriteMemoryFailed,
        "写入内存失败 (0x{}): {}",
        "Failed to write memory (0x{}): {}",
    ),
    (
        Msg::LayoutParsed,
        "已解析固件布局，准备烧录...",
        "Firmware layout parsed, ready to flash...",
    ),
    (Msg::StartingOp, "开始{}...", "Starting {}..."),
    (Msg::EraseLabel, "擦除", "Erase"),
    (Msg::ProgramLabel, "编程", "Program"),
    (Msg::VerifyLabel, "校验", "Verify"),
    (Msg::FillLabel, "填充", "Fill"),
    // ---- RTT 后台 ----
    (
        Msg::RttNotConnected,
        "尚未连接目标，无法启动 RTT",
        "Not connected to a target. Cannot start RTT",
    ),
    (
        Msg::RttCoreFailedStart,
        "获取核心失败，无法启动 RTT: {}",
        "Failed to get core. Cannot start RTT: {}",
    ),
    (
        Msg::RttStartedDetected,
        "RTT 已启动，检测到 {} 个上行、{} 个下行通道",
        "RTT started: {} up, {} down channel(s)",
    ),
    (
        Msg::RttStartFailed,
        "启动 RTT 失败: {}。请确认固件已初始化 RTT 且目标程序正在运行",
        "Failed to start RTT: {}. Make sure the firmware has initialized RTT and is running",
    ),
    (
        Msg::RttCoreFailedStopped,
        "获取核心失败，RTT 已停止: {}",
        "Failed to get core; RTT stopped: {}",
    ),
    (
        Msg::RttReadFailed,
        "RTT 读取失败，已停止: {}",
        "RTT read failed, stopped: {}",
    ),
    (Msg::RttNotStarted, "RTT 未启动或未连接目标", "RTT not started or not connected"),
    (Msg::RttCoreFailed, "获取核心失败: {}", "Failed to get core: {}"),
    (Msg::RttDownWriteFailed, "RTT 下行写入失败: {}", "RTT down write failed: {}"),
    (
        Msg::RttDownBufferFull,
        "目标下行缓冲区已满，部分数据未发送",
        "Target down buffer full, some data was not sent",
    ),
    (
        Msg::RttNoDownChannel,
        "目标未配置 RTT 下行通道",
        "Target has no RTT down channel",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msg_table_keys_unique_and_texts_nonempty() {
        let mut seen = std::collections::HashSet::new();
        for (k, zh, en) in MSGS {
            assert!(!zh.is_empty(), "empty zh text for {k:?}");
            assert!(!en.is_empty(), "empty en text for {k:?}");
            assert!(seen.insert(*k), "duplicate Msg key: {k:?}");
        }
    }

    #[test]
    fn tr_resolves_every_entry() {
        for (k, zh, en) in MSGS {
            assert_eq!(Lang::Zh.tr(*k), *zh);
            assert_eq!(Lang::En.tr(*k), *en);
        }
    }
}
