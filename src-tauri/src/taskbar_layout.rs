use crate::{settings::TaskbarPosition, taskbar::TaskbarRect};

const EDGE_COMPONENT_ZONE_DIVISOR: i32 = 3;

/// 歌词模式可以占用的一段连续任务栏屏幕区域。
#[derive(Debug, Clone, Copy)]
pub struct AvailableSpan {
    pub(crate) left: i32,
    pub(crate) right: i32,
}

impl AvailableSpan {
    /// 返回可用区域左边界的屏幕横坐标。
    pub fn left(self) -> i32 {
        self.left
    }

    /// 返回可用区域的物理像素宽度。
    pub fn width(self) -> i32 {
        self.right.saturating_sub(self.left).max(1)
    }
}

/// 一个任务栏控件在屏幕物理像素坐标系中的矩形。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OccupiedRect {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
}

impl OccupiedRect {
    /// 返回矩形左边界。
    pub fn left(self) -> i32 {
        self.left
    }

    /// 返回矩形上边界。
    pub fn top(self) -> i32 {
        self.top
    }

    /// 返回矩形右边界。
    pub fn right(self) -> i32 {
        self.right
    }

    /// 返回矩形下边界。
    pub fn bottom(self) -> i32 {
        self.bottom
    }

    /// 返回矩形宽度。
    pub fn width(self) -> i32 {
        self.right - self.left
    }

    /// 返回矩形高度。
    pub fn height(self) -> i32 {
        self.bottom - self.top
    }
}

/// 一个可用于后续避让计算的任务栏控件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OccupiedRegion {
    pub(crate) name: String,
    pub(crate) class_name: String,
    pub(crate) rect: OccupiedRect,
}

impl OccupiedRegion {
    /// 返回 UI Automation 名称或 Win32 回退名称。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 返回控件类名，供诊断不同 Windows 版本的任务栏结构。
    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    /// 返回控件的屏幕物理像素矩形。
    pub fn rect(&self) -> OccupiedRect {
        self.rect
    }
}

/// 按简化规则计算 Bar 的屏幕横坐标：左右只跟随对应边缘组件。
pub fn resolve_bar_screen_x(
    position: TaskbarPosition,
    taskbar_rect: &TaskbarRect,
    regions: &[OccupiedRegion],
    bar_width: i32,
) -> i32 {
    let minimum_x = taskbar_rect.left();
    let maximum_x = taskbar_rect.right().saturating_sub(bar_width);
    let taskbar_width = taskbar_rect.width();
    let left_zone_end = taskbar_rect
        .left()
        .saturating_add(taskbar_width / EDGE_COMPONENT_ZONE_DIVISOR);
    let right_zone_start = taskbar_rect
        .right()
        .saturating_sub(taskbar_width / EDGE_COMPONENT_ZONE_DIVISOR);
    let semantic_central_bounds = central_taskbar_bounds(regions);

    let target_x = match position {
        TaskbarPosition::Left => regions
            .iter()
            .filter(|region| !is_central_taskbar_button(region))
            .filter(|region| {
                // 开始、搜索等中央按钮的内部 XAML 元素类名各不相同，
                // 只要矩形仍与中央按钮组重叠，就不能作为左侧组件参与定位。
                semantic_central_bounds.map_or(true, |(central_left, _)| {
                    region.rect().right() <= central_left
                })
            })
            .filter(|region| region.rect().right() <= left_zone_end)
            .map(|region| region.rect().right())
            .max()
            .unwrap_or(taskbar_rect.left()),
        TaskbarPosition::Right => regions
            .iter()
            .filter(|region| !is_central_taskbar_button(region))
            .filter(|region| {
                semantic_central_bounds.map_or(true, |(_, central_right)| {
                    region.rect().left() >= central_right
                })
            })
            .filter(|region| region.rect().left() >= right_zone_start)
            .map(|region| region.rect().left())
            .min()
            .unwrap_or(taskbar_rect.right())
            .saturating_sub(bar_width),
    };

    target_x.clamp(minimum_x, maximum_x.max(minimum_x))
}

/// 按 Bar 位置返回歌词模式可占满的任务栏连续空白区域。
pub fn resolve_available_span(
    position: TaskbarPosition,
    taskbar_rect: &TaskbarRect,
    regions: &[OccupiedRegion],
) -> AvailableSpan {
    let taskbar_width = taskbar_rect.width();
    let left_zone_end = taskbar_rect
        .left()
        .saturating_add(taskbar_width / EDGE_COMPONENT_ZONE_DIVISOR);
    let right_zone_start = taskbar_rect
        .right()
        .saturating_sub(taskbar_width / EDGE_COMPONENT_ZONE_DIVISOR);
    let center_x = taskbar_rect.left().saturating_add(taskbar_width / 2);

    let semantic_central_bounds = central_taskbar_bounds(regions);
    let left_edge = regions
        .iter()
        .filter(|region| !is_central_taskbar_button(region))
        .filter(|region| {
            // 搜索按钮等 XAML 控件会暴露 AnimatedVisualPlayer 之类的内部子元素。
            // 这些子元素虽然类名不同，却仍位于中央按钮组内，不能误判为左侧组件。
            semantic_central_bounds.map_or(true, |(central_left, _)| {
                region.rect().right() <= central_left
            })
        })
        .filter(|region| region.rect().right() <= left_zone_end)
        .map(|region| region.rect().right())
        .max()
        .unwrap_or(taskbar_rect.left());
    let right_edge = regions
        .iter()
        .filter(|region| !is_central_taskbar_button(region))
        .filter(|region| {
            semantic_central_bounds.map_or(true, |(_, central_right)| {
                region.rect().left() >= central_right
            })
        })
        .filter(|region| region.rect().left() >= right_zone_start)
        .map(|region| region.rect().left())
        .min()
        .unwrap_or(taskbar_rect.right());

    // 过滤掉横跨大部分任务栏的框架容器，只保留中部按钮组及其实际子项。
    let (central_left, central_right) = regions
        .iter()
        .filter_map(|region| {
            let rect = region.rect();
            (rect.width() < taskbar_width * 2 / 3
                && rect.right() > left_zone_end
                && rect.left() < right_zone_start)
                .then_some((rect.left(), rect.right()))
        })
        .fold((None::<i32>, None::<i32>), |(left, right), rect| {
            (
                Some(left.map_or(rect.0, |value| value.min(rect.0))),
                Some(right.map_or(rect.1, |value| value.max(rect.1))),
            )
        });
    let (central_left, central_right) = semantic_central_bounds.unwrap_or((
        central_left.unwrap_or(center_x),
        central_right.unwrap_or(center_x),
    ));

    let (left, right) = match position {
        TaskbarPosition::Left => (left_edge, central_left),
        TaskbarPosition::Right => (central_right, right_edge),
    };
    // XAML 正在重新布局时可能短暂只返回一部分元素。遇到反向边界时使用任务栏
    // 外沿作为保守回退，绝不能把 Bar 折叠成 1 像素。
    let (left, right) = if right > left {
        (left, right)
    } else {
        match position {
            TaskbarPosition::Left => (taskbar_rect.left(), central_left),
            TaskbarPosition::Right => (central_right, taskbar_rect.right()),
        }
    };
    let left = left.clamp(taskbar_rect.left(), taskbar_rect.right().saturating_sub(1));
    let right = right.clamp(left.saturating_add(1), taskbar_rect.right());

    AvailableSpan { left, right }
}

/// 识别 Windows 11 中央开始按钮、系统按钮和任务按钮的 UI Automation 类型。
pub(crate) fn is_central_taskbar_button(region: &OccupiedRegion) -> bool {
    matches!(
        region.class_name(),
        "ToggleButton" | "Taskbar.TaskListButtonAutomationPeer"
    )
}

/// 返回开始、搜索、任务视图和任务按钮共同覆盖的横向范围。
fn central_taskbar_bounds(regions: &[OccupiedRegion]) -> Option<(i32, i32)> {
    regions
        .iter()
        .filter(|region| is_central_taskbar_button(region))
        .map(|region| (region.rect().left(), region.rect().right()))
        .fold(None, |bounds, rect| {
            Some(match bounds {
                Some((left, right)) => (left.min(rect.0), right.max(rect.1)),
                None => rect,
            })
        })
}

/// 将已验证任务栏矩形转换为本模块的通用占用矩形。
pub(crate) fn occupied_rect_from_taskbar(taskbar_rect: &TaskbarRect) -> OccupiedRect {
    OccupiedRect {
        left: taskbar_rect.left(),
        top: taskbar_rect.top(),
        right: taskbar_rect.right(),
        bottom: taskbar_rect.bottom(),
    }
}
