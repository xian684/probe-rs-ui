//! 烧录进度事件映射：probe-rs 的 `FlashProgress` 回调 → 界面可展示的 `WorkerEvent`。

use probe_rs::flashing::{ProgressEvent, ProgressOperation};

use crate::i18n::{Lang, Msg};
use crate::t;

use super::{OpState, WorkerEvent};

/// 将 probe-rs 进度回调映射为界面事件（无对应展示的事件返回 `None`）。
pub(super) fn map(event: ProgressEvent, lang: Lang) -> Option<WorkerEvent> {
    match event {
        ProgressEvent::FlashLayoutReady { .. } => {
            Some(WorkerEvent::Status(lang.tr(Msg::LayoutParsed).to_owned()))
        }
        ProgressEvent::AddProgressBar { operation, total } => Some(WorkerEvent::Progress {
            operation: op_label(operation, lang),
            done: 0,
            total,
            state: OpState::Active,
        }),
        ProgressEvent::Started(operation) => Some(WorkerEvent::Status(t!(
            lang,
            Msg::StartingOp,
            op_label(operation, lang)
        ))),
        ProgressEvent::Progress {
            operation, size, ..
        } => Some(WorkerEvent::Progress {
            operation: op_label(operation, lang),
            done: size,
            total: None,
            state: OpState::Active,
        }),
        ProgressEvent::Failed(operation) => Some(WorkerEvent::Progress {
            operation: op_label(operation, lang),
            done: 0,
            total: None,
            state: OpState::Failed,
        }),
        ProgressEvent::Finished(operation) => Some(WorkerEvent::Progress {
            operation: op_label(operation, lang),
            done: 0,
            total: None,
            state: OpState::Done,
        }),
        ProgressEvent::DiagnosticMessage { message } => Some(WorkerEvent::Diagnostic(message)),
    }
}

/// 烧录子操作（擦除/编程/校验/填充）的本地化标签。
fn op_label(op: ProgressOperation, lang: Lang) -> &'static str {
    match op {
        ProgressOperation::Erase => lang.tr(Msg::EraseLabel),
        ProgressOperation::Program => lang.tr(Msg::ProgramLabel),
        ProgressOperation::Verify => lang.tr(Msg::VerifyLabel),
        ProgressOperation::Fill => lang.tr(Msg::FillLabel),
    }
}
