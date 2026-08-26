//! 系统媒体领域：会话观察、选择、控制、封面处理与前端 DTO。

mod activity;
mod artwork;
mod control;
mod model;
mod runtime;
mod selection;

pub(crate) use activity::{MediaSelectionReason, MediaSessionActivity};
pub(crate) use control::{ControlAction, MediaControlError};
pub(crate) use model::{MediaSessionIdentity, MediaSnapshot};
pub(crate) use runtime::SystemMediaManager;
