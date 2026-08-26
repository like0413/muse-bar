//! Windows 任务栏领域：发现、占用布局、Child 宿主与 Explorer 恢复。

mod bar;
mod explorer;
mod host;
mod layout;
mod occupancy;
mod system;

pub(crate) use bar::apply_content_width;
pub(crate) use explorer::{request_recovery, start as start_explorer_monitor, ExplorerMonitor};
pub(crate) use occupancy::read_occupied_regions;
pub(crate) use system::{
    find_main_taskbar, list_taskbar_monitors, read_taskbar_dpi, read_taskbar_rect, TaskbarDpi,
    TaskbarIdentity, TaskbarMonitor, TaskbarRect,
};

pub(crate) use host::hide_window;
