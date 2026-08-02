//! egui 界面面板渲染：顶栏、设备检测、固件烧录与 RTT 日志面板。
//!
//! 通过 `ProbeUiApp` 的 `pub(crate)` 接口访问应用状态与事件处理。

mod central;
mod device;
mod flash;
mod mem_panel;
mod rtt_panel;
mod top;
