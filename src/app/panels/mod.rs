//! egui 界面面板渲染：顶栏、设备检测、固件烧录与 RTT 日志面板。
//!
//! 作为 `app` 的子模块，可直接访问 `ProbeUiApp` 的私有字段与方法。

mod device;
mod flash;
mod rtt_panel;
mod top;
