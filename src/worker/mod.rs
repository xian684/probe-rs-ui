//! 后台工作线程：接收 UI 命令、持有 probe-rs 会话、回传事件。
//!
//! 职责划分（本文件只做模块组织与对外接口，具体逻辑在各子模块）：
//! - [`run`]：线程主循环与命令分发
//! - [`probe`]：探针扫描、目标连接、复位
//! - [`flash`]：烧录、全片擦除、读取 Flash
//! - [`memory`]：内存读写
//! - [`progress`]：烧录进度事件映射

mod flash;
mod memory;
mod probe;
mod progress;
mod run;
mod target_gen;

use std::path::PathBuf;
use std::sync::mpsc;

use crate::firmware::FirmwareCandidate;
use crate::i18n::Lang;

/// 一个可展示给界面的探针描述。
#[derive(Clone)]
pub struct ProbeInfo {
    pub identifier: String,
    pub serial_number: Option<String>,
    pub probe_type: String,
    pub index: usize,
}

/// 一段内存区域摘要。
#[derive(Clone)]
pub struct MemRegionInfo {
    pub kind: &'static str,
    pub start: u64,
    pub end: u64,
}

/// 连接成功后展示给界面的目标摘要。
#[derive(Clone)]
pub struct TargetSummary {
    pub name: String,
    pub architecture: String,
    pub cores: Vec<(usize, String)>,
    pub memory: Vec<MemRegionInfo>,
}

/// 外部加载的芯片描述文件（CMSIS 包生成的 YAML target）解析结果。
#[derive(Clone)]
pub struct ChipFileInfo {
    pub family_name: String,
    pub chips: Vec<String>,
}

/// target-gen 生成的单个芯片族摘要（用于左侧面板结果展示）。
#[derive(Clone)]
pub struct TargetGenFamilyInfo {
    pub name: String,
    pub variant_count: usize,
    pub output_file: String,
}

/// target-gen 生成结果：芯片族摘要列表 + 落盘文件路径。
#[derive(Clone)]
pub struct TargetGenResult {
    pub families: Vec<TargetGenFamilyInfo>,
    /// 生成后已加载到 registry 的芯片族（供手动选型/自动连接）。
    pub loaded: Vec<ChipFileInfo>,
}

/// 进度条状态。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OpState {
    Active,
    Done,
    Failed,
}

/// 连接方式（对应 STM32 BOOT0/BOOT1 启动配置场景）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    /// 正常连接，直接从主 Flash 启动（BOOT0 = 0）。
    Normal,
    /// 复位期间连接，通过探针保持目标复位直至协议初始化完成，
    /// 常用于目标程序干扰 SWD 或 BOOT0 拉高从系统存储器启动的场景。
    UnderReset,
}

/// 发送给后台工作线程的命令。
pub enum WorkerCommand {
    Scan,
    ConnectAuto {
        probe: usize,
        boot_mode: BootMode,
    },
    ConnectManual {
        probe: usize,
        target: String,
        boot_mode: BootMode,
    },
    Flash {
        path: PathBuf,
        do_chip_erase: bool,
        verify: bool,
        keep_unwritten_bytes: bool,
        reset_after: bool,
        bin_base: u64,
    },
    EraseAll,
    ReadFlash {
        path: PathBuf,
        start: u64,
        end: u64,
    },
    MemoryRead {
        start: u64,
        len: usize,
    },
    MemoryWrite {
        start: u64,
        data: Vec<u8>,
    },
    Reset,
    Disconnect,
    Shutdown,
    ScanFirmware {
        root: PathBuf,
    },
    LoadChipFile {
        path: PathBuf,
    },
    GeneratePack {
        path: PathBuf,
    },
    TargetGenGenerate {
        /// 输入：.pack 文件或解压后的目录。
        input: PathBuf,
        /// 输出目录：生成的 YAML target 文件将写入这里（留空则不落盘）。
        output_dir: PathBuf,
        /// 是否只生成 probe-rs 已支持芯片族的 target（对应 target-gen 的 only_supported_families）。
        only_supported: bool,
        /// 生成后是否加载到 registry（供手动选型/自动连接）。
        auto_load: bool,
    },
    SetLang(Lang),
    RttStart,
    RttStop,
    RttWrite {
        channel: usize,
        data: Vec<u8>,
    },
}

/// 后台工作线程回传给界面的事件。
pub enum WorkerEvent {
    Probes(Result<Vec<ProbeInfo>, String>),
    Connected(Result<TargetSummary, String>),
    Status(String),
    Diagnostic(String),
    Progress {
        operation: &'static str,
        done: u64,
        total: Option<u64>,
        state: OpState,
    },
    OperationDone(Result<(), String>),
    FirmwareScanned {
        root: String,
        candidates: Vec<FirmwareCandidate>,
        best: Option<usize>,
    },
    ChipFileLoaded(Result<ChipFileInfo, String>),
    PackGenerated(Result<Vec<ChipFileInfo>, String>),
    TargetGenDone(Result<TargetGenResult, String>),
    RttData {
        channel: usize,
        data: Vec<u8>,
    },
    RttStarted {
        up_channels: usize,
        down_channels: usize,
    },
    RttStopped,
    MemoryRead(Result<Vec<u8>, String>),
    MemoryWrite(Result<(), String>),
}

pub struct Worker {
    pub sender: mpsc::Sender<WorkerCommand>,
    pub receiver: mpsc::Receiver<WorkerEvent>,
}

pub fn spawn(lang: Lang) -> Worker {
    let (tx, rx) = mpsc::channel::<WorkerCommand>();
    let (etx, erx) = mpsc::channel::<WorkerEvent>();
    std::thread::Builder::new()
        .name("probe-rs-worker".to_owned())
        .spawn(move || run::run(rx, etx, lang))
        .expect("无法创建后台工作线程");
    Worker {
        sender: tx,
        receiver: erx,
    }
}
