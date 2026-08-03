//! 用户配置持久化：将常用设置保存到本地 config.toml，下次启动自动恢复。

use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    /// 界面语言："zh" / "en"。
    pub lang: String,
    /// 主题模式："system" / "light" / "dark"。
    pub theme: String,
    /// 连接方式："normal" / "under_reset"。
    pub boot_mode: String,
    /// 手动指定的芯片型号。
    pub manual_target: String,
    /// 最近使用的固件文件路径。
    pub file_path: String,
    /// 最近打开的项目文件夹。
    pub firmware_root: String,
    /// 全片擦除后烧录。
    pub chip_erase: bool,
    /// 烧录后校验。
    pub verify: bool,
    /// 保留未写入字节。
    pub keep_unwritten: bool,
    /// 烧录后复位运行。
    pub reset_after: bool,
    /// .bin 烧录基地址。
    pub bin_base: u64,
    /// RTT 上行显示通道（None 表示全部）。
    pub rtt_view_channel: Option<usize>,
    /// RTT 下行发送通道。
    pub rtt_send_channel: usize,
    /// RTT 自动滚动。
    pub rtt_autoscroll: bool,
    /// 中央面板标签："flash" / "memory" / "rtt"。
    pub central_tab: String,
    /// 内存查看器起始地址。
    pub mem_start: u64,
    /// 内存查看器读取长度。
    pub mem_len: usize,
    /// 内存查看器写入起始地址。
    pub mem_write_start: u64,
    /// Target 生成器最近使用的输入路径。
    pub tg_input: String,
    /// Target 生成器最近使用的输出目录。
    pub tg_output_dir: String,
    /// 窗口大小（宽、高）。
    pub window_size: Option<[f32; 2]>,
    /// 窗口位置（x、y）。
    pub window_pos: Option<[f32; 2]>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            lang: "zh".into(),
            theme: "system".into(),
            boot_mode: "normal".into(),
            manual_target: String::new(),
            file_path: String::new(),
            firmware_root: String::new(),
            chip_erase: false,
            verify: true,
            keep_unwritten: true,
            reset_after: true,
            bin_base: 0,
            rtt_view_channel: None,
            rtt_send_channel: 0,
            rtt_autoscroll: true,
            central_tab: "flash".into(),
            mem_start: 0,
            mem_len: 256,
            mem_write_start: 0,
            tg_input: String::new(),
            tg_output_dir: String::new(),
            window_size: None,
            window_pos: None,
        }
    }
}

/// 配置文件路径：优先与可执行文件同目录（便于便携使用），否则使用当前目录。
pub fn config_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.join("config.toml");
        }
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("config.toml")
}

pub fn load() -> AppConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save(cfg: &AppConfig) {
    let path = config_path();
    if let Ok(text) = toml::to_string(cfg) {
        if std::fs::write(&path, text).is_ok() {
            set_hidden(&path);
        }
    }
}

/// Windows 下将配置文件标记为隐藏，避免在可执行文件目录中显眼地出现。
/// 其他平台（Linux/macOS）下为空实现。
#[cfg(windows)]
fn set_hidden(path: &std::path::Path) {
    use std::os::windows::ffi::OsStrExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    extern "system" {
        fn SetFileAttributesW(lpFileName: *const u16, dwFileAttributes: u32) -> i32;
    }
    let wide: Vec<u16> = std::ffi::OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        SetFileAttributesW(wide.as_ptr(), FILE_ATTRIBUTE_HIDDEN);
    }
}

#[cfg(not(windows))]
fn set_hidden(_path: &std::path::Path) {}
