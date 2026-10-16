use crate::commands::StorageState;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{
    Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
#[cfg(windows)]
use windows::Win32::{
    Foundation::POINT,
    UI::{Input::KeyboardAndMouse::GetAsyncKeyState, WindowsAndMessaging::GetCursorPos},
};

// The ball has its own fixed-size transparent host. Preview resizing never
// touches its bitmap or native frame; no SetWindowRgn/native-style overrides.
const COLLAPSED_WIDTH: f64 = 108.0;
const COLLAPSED_HEIGHT: f64 = 88.0;
const EXPANDED_WIDTH: f64 = 390.0;
const EXPANDED_HEIGHT: f64 = 456.0;
// Ratios refer to the visible horizontal diameter, excluding transparent margins.
const BALL_DIAMETER: f64 = 64.0;
const DOCK_VISIBLE_RATIO: f64 = 0.70;
const HIDDEN_VISIBLE_RATIO: f64 = 0.30;
// CSS keeps the ball center 62 DIP from the right and 44 DIP from the top.
const BALL_RIGHT: f64 = 62.0;
const BALL_TOP: f64 = 44.0;

#[derive(Default)]
pub struct FloatingState {
    expanded: AtomicBool,
    ball_focused: AtomicBool,
    preview_focused: AtomicBool,
    dragging: AtomicBool,
    dock_monitor_running: AtomicBool,
    display_changed: AtomicBool,
    dock: Mutex<DockState>,
    save: Mutex<SaveState>,
}

#[derive(Default)]
struct SaveState {
    path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default)]
struct DockState {
    side: Option<DockSide>,
    hidden: bool,
    anchor: Option<BallPosition>,
    outside_since: Option<Instant>,
    area: Option<WorkArea>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DockSide {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct BallPosition {
    x: i32,
    y: i32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
struct SavedPosition {
    x: i32,
    y: i32,
    dock: Option<DockSide>,
    hidden: bool,
}

impl Default for SavedPosition {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            dock: None,
            hidden: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WorkArea {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f64,
}

#[derive(Debug, PartialEq)]
struct Layout {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

fn layout(center: BallPosition, area: WorkArea, _expanded: bool) -> Layout {
    let scale = area.scale;
    let width = (COLLAPSED_WIDTH * scale)
        .round()
        .max(1.0)
        .min(area.width.max(1) as f64) as u32;
    let height = (COLLAPSED_HEIGHT * scale)
        .round()
        .max(1.0)
        .min(area.height.max(1) as f64) as u32;
    let x = center.x as i64 - width as i64 + (BALL_RIGHT * scale).round() as i64;
    let y = center.y as i64 - (BALL_TOP * scale).round() as i64;
    Layout {
        x: x.clamp(
            area.x as i64,
            area.x as i64 + area.width.saturating_sub(width) as i64,
        ) as i32,
        y: y.clamp(
            area.y as i64,
            area.y as i64 + area.height.saturating_sub(height) as i64,
        ) as i32,
        width,
        height,
    }
}

fn dock_layout(center: BallPosition, area: WorkArea, side: DockSide, hidden: bool) -> Layout {
    let width = (BALL_DIAMETER * if hidden { HIDDEN_VISIBLE_RATIO } else { 1.0 } * area.scale)
        .round()
        .max(1.0)
        .min(area.width.max(1) as f64) as u32;
    let height = (BALL_DIAMETER * area.scale)
        .round()
        .max(1.0)
        .min(area.height.max(1) as f64) as u32;
    let y = (center.y as i64 - (height as f64 / 2.0).round() as i64).clamp(
        area.y as i64,
        area.y as i64 + area.height.saturating_sub(height) as i64,
    );
    let x = match side {
        DockSide::Left => area.x,
        DockSide::Right => area.x + area.width.saturating_sub(width) as i32,
    };
    Layout {
        x,
        y: y as i32,
        width,
        height,
    }
}

fn ball_position(
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    scale: f64,
) -> BallPosition {
    BallPosition {
        x: position.x + size.width as i32 - (BALL_RIGHT * scale).round() as i32,
        y: position.y + (BALL_TOP * scale).round() as i32,
    }
}

fn current_ball(window: &WebviewWindow) -> Result<BallPosition, String> {
    let state = window.app_handle().state::<FloatingState>();
    let dock = *state.dock.lock().map_err(|_| "浮球状态不可用")?;
    if dock.hidden {
        return Err("请先唤出完整浮球".into());
    }
    if dock.side.is_some() {
        let position = window.outer_position().map_err(|_| "无法读取浮球位置")?;
        let size = window.inner_size().map_err(|_| "无法读取浮球大小")?;
        return Ok(BallPosition {
            x: position.x + (size.width as f64 / 2.0).round() as i32,
            y: position.y + (size.height as f64 / 2.0).round() as i32,
        });
    }
    Ok(ball_position(
        window.outer_position().map_err(|_| "无法读取浮球位置")?,
        window.inner_size().map_err(|_| "无法读取浮球大小")?,
        window.scale_factor().map_err(|_| "无法读取屏幕缩放")?,
    ))
}

fn work_area(window: &WebviewWindow, center: Option<BallPosition>) -> Result<WorkArea, String> {
    let monitor = center
        .and_then(|p| {
            window
                .monitor_from_point(p.x as f64, p.y as f64)
                .ok()
                .flatten()
        })
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or("无法读取显示器工作区")?;
    let area = monitor.work_area();
    Ok(WorkArea {
        x: area.position.x,
        y: area.position.y,
        width: area.size.width,
        height: area.size.height,
        scale: monitor.scale_factor(),
    })
}

#[cfg(windows)]
fn global_cursor() -> Option<(i32, i32)> {
    unsafe {
        let mut point = POINT::default();
        GetCursorPos(&mut point).ok()?;
        Some((point.x, point.y))
    }
}

#[cfg(not(windows))]
fn global_cursor() -> Option<(i32, i32)> {
    None
}

#[cfg(windows)]
fn left_button_down() -> bool {
    unsafe { GetAsyncKeyState(0x01) < 0 }
}

#[cfg(not(windows))]
fn left_button_down() -> bool {
    false
}

fn point_inside_window(window: &WebviewWindow, point: (i32, i32)) -> bool {
    let Ok(position) = window.outer_position() else {
        return false;
    };
    let Ok(size) = window.inner_size() else {
        return false;
    };
    point.0 >= position.x
        && point.1 >= position.y
        && point.0 < position.x + size.width as i32
        && point.1 < position.y + size.height as i32
}

fn apply_window_layout(
    window: &WebviewWindow,
    center: BallPosition,
    expanded: bool,
    dock: Option<DockSide>,
    hidden: bool,
) -> Result<(), String> {
    let area = work_area(window, Some(center))?;
    let next = match dock {
        Some(side) if !expanded => dock_layout(center, area, side, hidden),
        _ => layout(center, area, expanded),
    };
    let size = PhysicalSize::new(next.width, next.height);
    if window.inner_size().map_err(|_| "无法读取浮球大小")? != size {
        window.set_size(size).map_err(|_| "浮球未能调整大小")?;
    }
    let position = PhysicalPosition::new(next.x, next.y);
    if window.outer_position().map_err(|_| "无法读取浮球位置")? != position {
        window.set_position(position).map_err(|_| "浮球未能定位")?;
    }
    Ok(())
}

fn dock_status(state: DockState) -> DockStatus {
    DockStatus {
        side: state.side,
        hidden: state.hidden,
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockStatus {
    side: Option<DockSide>,
    hidden: bool,
}

fn emit_dock_status(app: &tauri::AppHandle) {
    let snapshot = app
        .state::<FloatingState>()
        .dock
        .lock()
        .ok()
        .map(|dock| *dock);
    if let Some(dock) = snapshot {
        let _ = app.emit_to(
            "floating",
            "pometodo-floating-dock-state",
            dock_status(dock),
        );
    }
}

fn detect_dock(center: BallPosition, area: WorkArea) -> Option<DockSide> {
    let diameter = BALL_DIAMETER * area.scale;
    detect_dock_edges(center.x as f64 - diameter / 2.0, diameter, area)
}

fn detect_dock_edges(left: f64, diameter: f64, area: WorkArea) -> Option<DockSide> {
    let right = left + diameter;
    let screen_left = area.x as f64;
    let screen_right = screen_left + area.width as f64;
    // Strictly less than 70%. Vertical clipping must never trigger docking.
    let missing = diameter * (1.0 - DOCK_VISIBLE_RATIO);
    if left < screen_left - missing - 1e-7 {
        Some(DockSide::Left)
    } else if right > screen_right + missing + 1e-7 {
        Some(DockSide::Right)
    } else {
        None
    }
}

fn settled_side(
    center: BallPosition,
    area: WorkArea,
    previous: Option<DockSide>,
) -> Option<DockSide> {
    let half = (BALL_DIAMETER * area.scale / 2.0).round() as i32;
    // A revealed docked ball is fully inside the screen. Moving it vertically
    // along that same edge must retain docking, without docking a free ball there.
    match previous {
        Some(DockSide::Left) if (center.x - area.x - half).abs() <= 1 => previous,
        Some(DockSide::Right) if (center.x - area.x - area.width as i32 + half).abs() <= 1 => {
            previous
        }
        _ => detect_dock(center, area),
    }
}

fn preview_layout(center: BallPosition, area: WorkArea) -> (Layout, bool) {
    let width = (EXPANDED_WIDTH * area.scale).round().min(area.width as f64) as u32;
    let height = ((EXPANDED_HEIGHT - COLLAPSED_HEIGHT) * area.scale)
        .round()
        .min(area.height as f64) as u32;
    let gap = (32.0 * area.scale).round() as i32;
    let above = center.y + gap + height as i32 > area.y + area.height as i32;
    let x = center.x - width as i32 + (BALL_RIGHT * area.scale).round() as i32;
    let y = if above {
        center.y - gap - height as i32
    } else {
        center.y + gap
    };
    (
        Layout {
            x: x.clamp(area.x, area.x + area.width.saturating_sub(width) as i32),
            y: y.clamp(area.y, area.y + area.height.saturating_sub(height) as i32),
            width,
            height,
        },
        above,
    )
}

fn collapse_if_unfocused(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Focus can transfer from ball to preview in separate Windows messages.
        std::thread::sleep(Duration::from_millis(80));
        let state = app.state::<FloatingState>();
        if !state.ball_focused.load(Ordering::SeqCst)
            && !state.preview_focused.load(Ordering::SeqCst)
        {
            let _ = app.emit("pometodo-floating-collapse", ());
        }
    });
}

fn read_position(path: &std::path::Path) -> Option<SavedPosition> {
    let file = fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(257).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 256 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

fn write_position(path: &std::path::Path, position: SavedPosition) -> Result<(), String> {
    let bytes = serde_json::to_vec(&position).map_err(|_| "浮球位置未能保存")?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|_| "浮球位置未能保存")?;
    fs::rename(&temporary, path).map_err(|_| "浮球位置未能保存".into())
}

fn report(app: &tauri::AppHandle, error: String) {
    let _ = app.emit_to("floating", "pometodo-floating-error", &error);
    let _ = app.emit("pometodo-system-error", error);
}

fn persist_position(app: &tauri::AppHandle) {
    let floating_state = app.state::<FloatingState>();
    let Some(state) = floating_state.dock.lock().ok().map(|dock| *dock) else {
        return;
    };
    let Some(center) = state.anchor else {
        return;
    };
    let saved = SavedPosition {
        x: center.x,
        y: center.y,
        dock: state.side,
        hidden: false,
    };
    // Serialize writes without holding a lock across any native window call.
    let save = floating_state.save.lock().ok();
    if let Some(path) = save.as_ref().and_then(|save| save.path.as_ref()) {
        if let Err(error) = write_position(path, saved) {
            report(app, error);
        }
    }
}

fn set_dock_hidden(app: &tauri::AppHandle, hidden: bool) -> Result<bool, String> {
    let window = app.get_webview_window("floating").ok_or("浮球窗口不可用")?;
    let state = app.state::<FloatingState>();
    if hidden
        && (state.expanded.load(Ordering::SeqCst)
            || state.dragging.load(Ordering::SeqCst)
            || crate::context_menu::floating_menu_open(app))
    {
        return Ok(false);
    }
    let (center, side) = {
        let dock = state.dock.lock().map_err(|_| "浮球状态不可用")?;
        let Some(side) = dock.side else {
            return Ok(false);
        };
        if dock.hidden == hidden {
            return Ok(false);
        }
        (dock.anchor.ok_or("浮球位置不可用")?, side)
    };
    apply_window_layout(&window, center, false, Some(side), hidden)?;
    {
        let mut dock = state.dock.lock().map_err(|_| "浮球状态不可用")?;
        dock.hidden = hidden;
        dock.outside_since = None;
    }
    emit_dock_status(app);
    Ok(true)
}

#[derive(Debug, PartialEq)]
enum DockAction {
    None,
    Reveal,
    Hide,
}

// Shared by the native monitor and behavioral tests; no web pointer-up is needed.
fn hover_action(dock: &mut DockState, now: Instant, inside: bool, blocked: bool) -> DockAction {
    if dock.side.is_none() || blocked {
        dock.outside_since = None;
        return DockAction::None;
    }
    if inside {
        dock.outside_since = None;
        return if dock.hidden {
            DockAction::Reveal
        } else {
            DockAction::None
        };
    }
    if dock.hidden {
        return DockAction::None;
    }
    let since = dock.outside_since.get_or_insert(now);
    if now.duration_since(*since) >= Duration::from_millis(500) {
        DockAction::Hide
    } else {
        DockAction::None
    }
}

fn monitor_tick(app: &tauri::AppHandle) -> Result<bool, String> {
    let state = app.state::<FloatingState>();
    if state.dragging.load(Ordering::SeqCst) {
        if left_button_down() {
            return Ok(true);
        }
        finish_drag(app)?;
    }
    let window = app.get_webview_window("floating").ok_or("浮球窗口不可用")?;
    let snapshot = *state.dock.lock().map_err(|_| "浮球状态不可用")?;
    let center = snapshot.anchor.ok_or("浮球位置不可用")?;
    let area = work_area(&window, Some(center))?;
    if state.display_changed.swap(false, Ordering::SeqCst) || snapshot.area != Some(area) {
        apply_window_layout(&window, center, false, snapshot.side, snapshot.hidden)?;
        let complete = match snapshot.side {
            Some(side) => {
                let rect = dock_layout(center, area, side, false);
                BallPosition {
                    x: rect.x + (rect.width as f64 / 2.0).round() as i32,
                    y: rect.y + (rect.height as f64 / 2.0).round() as i32,
                }
            }
            None => current_ball(&window)?,
        };
        {
            let mut dock = state.dock.lock().map_err(|_| "浮球状态不可用")?;
            dock.anchor = Some(complete);
            dock.area = Some(area);
        }
        persist_position(app);
        if state.expanded.load(Ordering::SeqCst) {
            pometodo_float_expand(app.clone(), true)?;
        }
    }
    if snapshot.side.is_none() {
        return Ok(false);
    }
    if !window.is_visible().map_err(|_| "无法读取浮球显示状态")? {
        return Ok(true);
    }
    let inside = global_cursor()
        .map(|point| point_inside_window(&window, point))
        .unwrap_or(true);
    let blocked = state.expanded.load(Ordering::SeqCst)
        || left_button_down()
        || crate::context_menu::floating_menu_open(app);
    let action = {
        let mut dock = state.dock.lock().map_err(|_| "浮球状态不可用")?;
        hover_action(&mut dock, Instant::now(), inside, blocked)
    };
    match action {
        DockAction::Reveal => {
            set_dock_hidden(app, false)?;
        }
        DockAction::Hide => {
            set_dock_hidden(app, true)?;
        }
        DockAction::None => {}
    }
    Ok(true)
}

fn start_dock_monitor(app: &tauri::AppHandle) {
    let state = app.state::<FloatingState>();
    if state.dock_monitor_running.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        loop {
            std::thread::sleep(Duration::from_millis(50));
            // All transitions and window changes are serialized with native commands.
            // Never hold a state mutex while dispatching to the UI thread.
            let (send, receive) = std::sync::mpsc::channel();
            let tick_app = app.clone();
            if app
                .run_on_main_thread(move || {
                    let keep_running = match monitor_tick(&tick_app) {
                        Ok(value) => value,
                        Err(error) => {
                            tick_app
                                .state::<FloatingState>()
                                .dragging
                                .store(false, Ordering::SeqCst);
                            report(&tick_app, error);
                            false
                        }
                    };
                    if !keep_running {
                        tick_app
                            .state::<FloatingState>()
                            .dock_monitor_running
                            .store(false, Ordering::SeqCst);
                    }
                    let _ = send.send(keep_running);
                })
                .is_err()
            {
                break;
            }
            if receive.recv() != Ok(true) {
                break;
            }
        }
    });
}

fn finish_drag(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<FloatingState>();
    let window = app.get_webview_window("floating").ok_or("浮球窗口不可用")?;
    let center = current_ball(&window)?;
    let area = work_area(&window, Some(center))?;
    let previous = state.dock.lock().map_err(|_| "浮球状态不可用")?.side;
    let side = settled_side(center, area, previous);
    apply_window_layout(&window, center, false, side, false)?;
    {
        let mut dock = state.dock.lock().map_err(|_| "浮球状态不可用")?;
        dock.side = side;
        dock.hidden = false;
        dock.outside_since = None;
        dock.area = Some(area);
    }
    let actual = current_ball(&window)?;
    state.dock.lock().map_err(|_| "浮球状态不可用")?.anchor = Some(actual);
    state.dragging.store(false, Ordering::SeqCst);
    emit_dock_status(app);
    persist_position(app);
    Ok(())
}

pub fn create(app: &tauri::AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(
        app,
        "floating",
        WebviewUrl::App("index.html?window=floating".into()),
    )
    .title("PomeTodo 浮球")
    .inner_size(COLLAPSED_WIDTH, COLLAPSED_HEIGHT)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .visible(false)
    .focused(false)
    .shadow(false)
    .build()?;
    let preview = WebviewWindowBuilder::new(
        app,
        "floating-preview",
        WebviewUrl::App("index.html?window=floating-preview".into()),
    )
    .title("PomeTodo 浮球预览")
    .inner_size(EXPANDED_WIDTH, EXPANDED_HEIGHT - COLLAPSED_HEIGHT)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .visible(false)
    .focused(false)
    .shadow(false)
    .build()?;
    let preview_app = app.clone();
    preview.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(focused) = event {
            preview_app
                .state::<FloatingState>()
                .preview_focused
                .store(*focused, Ordering::SeqCst);
            if !focused {
                collapse_if_unfocused(&preview_app);
            }
        }
    });
    // This independent profile path never follows a moved task data directory.
    let path = {
        let state = app.state::<StorageState>();
        state.inner.lock().ok().and_then(|storage| {
            storage
                .as_ref()
                .ok()
                .map(|storage| storage.profile_directory.join("floating-position.json"))
        })
    };
    if let Ok(area) = work_area(&window, None) {
        let saved = path.as_deref().and_then(read_position);
        let initial = saved
            .map(|position| BallPosition {
                x: position.x,
                y: position.y,
            })
            .unwrap_or(BallPosition {
                x: area.x + area.width as i32 - (80.0 * area.scale).round() as i32,
                y: area.y + (50.0 * area.scale).round() as i32,
            });
        if let Ok(mut dock) = app.state::<FloatingState>().dock.lock() {
            dock.side = saved.and_then(|position| position.dock);
            // Hidden is transient; restart with the complete ball visible at the saved edge.
            dock.hidden = false;
            dock.anchor = Some(initial);
        }
        let (side, hidden) = app
            .state::<FloatingState>()
            .dock
            .lock()
            .ok()
            .map(|dock| (dock.side, dock.hidden))
            .unwrap_or((None, false));
        if let Err(error) = apply_window_layout(&window, initial, false, side, hidden) {
            report(app, error);
        }
    }
    if let Ok(mut save) = app.state::<FloatingState>().save.lock() {
        save.path = path;
    }
    emit_dock_status(app);
    if app
        .state::<FloatingState>()
        .dock
        .lock()
        .ok()
        .is_some_and(|dock| dock.side.is_some())
    {
        start_dock_monitor(app);
    }
    let events_app = app.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::Focused(focused) => {
            events_app
                .state::<FloatingState>()
                .ball_focused
                .store(*focused, Ordering::SeqCst);
            if !focused {
                collapse_if_unfocused(&events_app);
            }
        }
        tauri::WindowEvent::ScaleFactorChanged { .. } => {
            events_app
                .state::<FloatingState>()
                .display_changed
                .store(true, Ordering::SeqCst);
            start_dock_monitor(&events_app);
        }
        tauri::WindowEvent::Moved(_) => {
            // Programmatic hide/reveal moves must not save a new anchor or collapse
            // the preview. Only an explicitly started drag is interpreted as a drag.
            start_dock_monitor(&events_app);
        }
        _ => {}
    });
    Ok(())
}

pub fn is_enabled(app: &tauri::AppHandle) -> Result<bool, String> {
    let state = app.state::<StorageState>();
    let guard = state.inner.lock().map_err(|_| "浮球无法读取设置")?;
    Ok(guard
        .as_ref()
        .map_err(Clone::clone)?
        .repo
        .settings()?
        .floating_ball_enabled)
}

pub fn set_enabled(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    {
        let state = app.state::<StorageState>();
        let mut guard = state.inner.lock().map_err(|_| "浮球无法保存设置")?;
        guard
            .as_mut()
            .map_err(|e| e.clone())?
            .repo
            .update_settings(serde_json::json!({ "floatingBallEnabled": enabled }))?;
    }
    // Let the tray and settings host refresh their switch from the persisted value.
    let _ = app.emit("pometodo-data-changed", ());
    let _ = app.emit("pometodo-floating-enabled", enabled);
    refresh(app)
}

pub fn refresh(app: &tauri::AppHandle) -> Result<(), String> {
    let should_show = {
        let state = app.state::<StorageState>();
        let guard = state.inner.lock().map_err(|_| "浮球无法读取任务")?;
        let repo = &guard.as_ref().map_err(Clone::clone)?.repo;
        repo.settings()?.floating_ball_enabled
            && repo
                .list()?
                .iter()
                .any(|task| task.status != pometodo_core::models::TodoStatus::Completed)
    };
    if let Some(window) = app.get_webview_window("floating") {
        if should_show {
            if !window.is_visible().map_err(|_| "无法读取浮球显示状态")? {
                window.show().map_err(|_| "浮球未能显示")?;
            }
            app.emit_to("floating", "pometodo-floating-refresh", ())
                .map_err(|_| "浮球未能刷新")?;
            let _ = app.emit_to("floating-preview", "pometodo-floating-refresh", ());
        } else {
            let _ = app.emit("pometodo-floating-collapse", ());
            window.hide().map_err(|_| "浮球未能收起")?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn pometodo_float_expand(app: tauri::AppHandle, expanded: bool) -> Result<(), String> {
    let window = app.get_webview_window("floating").ok_or("浮球窗口不可用")?;
    let state = app.state::<FloatingState>();
    let was_expanded = state.expanded.load(Ordering::SeqCst);
    if was_expanded == expanded && !expanded {
        return Ok(());
    }
    let preview = app
        .get_webview_window("floating-preview")
        .ok_or("浮球预览不可用")?;
    if expanded {
        set_dock_hidden(&app, false)?;
        let center = current_ball(&window)?;
        let (next, above) = preview_layout(center, work_area(&window, Some(center))?);
        preview
            .set_size(PhysicalSize::new(next.width, next.height))
            .map_err(|_| "预览大小未能更新")?;
        preview
            .set_position(PhysicalPosition::new(next.x, next.y))
            .map_err(|_| "预览未能定位")?;
        state.expanded.store(true, Ordering::SeqCst);
        if app
            .emit_to("floating-preview", "pometodo-floating-present", above)
            .is_err()
        {
            state.expanded.store(false, Ordering::SeqCst);
            return Err("预览未能打开".into());
        }
    } else {
        preview.hide().map_err(|_| "预览未能收起")?;
        state.expanded.store(false, Ordering::SeqCst);
        let _ = app.emit("pometodo-floating-closed", ());
        start_dock_monitor(&app);
    }
    Ok(())
}

#[tauri::command]
pub fn pometodo_float_drag_start(app: tauri::AppHandle) -> Result<(), String> {
    set_dock_hidden(&app, false)?;
    pometodo_float_expand(app.clone(), false)?;
    let state = app.state::<FloatingState>();
    if state.dragging.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    state
        .dock
        .lock()
        .map_err(|_| "浮球状态不可用")?
        .outside_since = None;
    start_dock_monitor(&app);
    let window = app.get_webview_window("floating").ok_or("浮球窗口不可用")?;
    if let Err(error) = window.start_dragging() {
        state.dragging.store(false, Ordering::SeqCst);
        return Err(format!("浮球未能移动：{error}"));
    }
    Ok(())
}

#[tauri::command]
pub fn pometodo_float_dock_state(app: tauri::AppHandle) -> Result<DockStatus, String> {
    app.state::<FloatingState>()
        .dock
        .lock()
        .map(|dock| dock_status(*dock))
        .map_err(|_| "浮球状态不可用".into())
}

#[tauri::command]
pub fn pometodo_float_dock_reveal(app: tauri::AppHandle) -> Result<bool, String> {
    let changed = set_dock_hidden(&app, false)?;
    start_dock_monitor(&app);
    Ok(changed)
}

#[tauri::command]
pub fn pometodo_float_preview_state(app: tauri::AppHandle) -> Result<Option<bool>, String> {
    if !app.state::<FloatingState>().expanded.load(Ordering::SeqCst) {
        return Ok(None);
    }
    let ball = app.get_webview_window("floating").ok_or("浮球不可用")?;
    let center = current_ball(&ball)?;
    Ok(Some(
        preview_layout(center, work_area(&ball, Some(center))?).1,
    ))
}

#[tauri::command]
pub fn pometodo_float_present(app: tauri::AppHandle) -> Result<bool, String> {
    if !app.state::<FloatingState>().expanded.load(Ordering::SeqCst) {
        return Ok(false);
    }
    let preview = app
        .get_webview_window("floating-preview")
        .ok_or("预览不可用")?;
    preview.show().map_err(|_| "预览未能显示")?;
    preview.set_focus().map_err(|_| "预览未能获取焦点")?;
    Ok(true)
}

#[tauri::command]
pub fn pometodo_float_menu(app: tauri::AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("floating").ok_or("浮球窗口不可用")?;
    let position = window.cursor_position().map_err(|_| "无法读取菜单位置")?;
    crate::context_menu::open(&app, true, position)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenRequest {
    task_id: Option<String>,
    settings: bool,
    pending: bool,
    request_id: u64,
}

#[tauri::command]
pub fn pometodo_open_main(
    app: tauri::AppHandle,
    task_id: Option<String>,
    settings: Option<bool>,
) -> Result<(), String> {
    let settings = settings.unwrap_or(false);
    let pending = task_id.is_none() && !settings;
    // Collapse through the frontend so the old 120ms fade can finish before resizing.
    let _ = app.emit("pometodo-floating-collapse", ());
    let main = app.get_webview_window("main").ok_or("待办窗口不可用")?;
    main.unminimize().map_err(|_| "未能打开待办窗口")?;
    main.show().map_err(|_| "未能打开待办窗口")?;
    main.set_focus().map_err(|_| "未能打开待办窗口")?;
    let request_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "系统时间异常")?
        .as_millis() as u64;
    app.emit_to(
        "main",
        "pometodo-open",
        OpenRequest {
            task_id,
            settings,
            pending,
            request_id,
        },
    )
    .map_err(|_| "窗口未能定位任务".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn primary(scale: f64) -> WorkArea {
        WorkArea {
            x: 0,
            y: 0,
            width: 1920,
            height: 1040,
            scale,
        }
    }

    #[test]
    fn popup_expansion_does_not_move_or_resize_the_ball_horizontally() {
        let center = BallPosition { x: 1500, y: 100 };
        let collapsed = layout(center, primary(1.0), false);
        let expanded = layout(center, primary(1.0), true);
        assert_eq!(collapsed.x, expanded.x);
        assert_eq!(collapsed.width, expanded.width);
        assert_eq!(collapsed, expanded);
        let (_, above) = preview_layout(center, primary(1.0));
        assert!(!above);
        let (panel, above) = preview_layout(BallPosition { x: 1500, y: 990 }, primary(1.0));
        assert!(above);
        assert!(panel.y + panel.height as i32 <= 990);
    }

    #[test]
    fn expanding_and_collapsing_preserves_ball_center() {
        let center = BallPosition { x: 1500, y: 100 };
        let area = primary(1.0);
        for expanded in [false, true, false] {
            let position = layout(center, area, expanded);
            assert_eq!(
                ball_position(
                    PhysicalPosition::new(position.x, position.y),
                    PhysicalSize::new(position.width, position.height),
                    area.scale
                ),
                center
            );
        }
    }

    #[test]
    fn bottom_right_expansion_stays_above_taskbar() {
        let result = layout(BallPosition { x: 1910, y: 1030 }, primary(1.0), true);
        assert_eq!(result.x + result.width as i32, 1920);
        assert_eq!(result.y + result.height as i32, 1040);
    }

    #[test]
    fn negative_monitor_coordinates_are_preserved_at_high_dpi() {
        let area = WorkArea {
            x: -2560,
            y: -200,
            width: 2560,
            height: 1400,
            scale: 1.5,
        };
        let center = BallPosition { x: -600, y: 50 };
        let result = layout(center, area, true);
        assert_eq!(result.width, 162);
        assert_eq!(result.height, 132);
        assert_eq!(
            ball_position(
                PhysicalPosition::new(result.x, result.y),
                PhysicalSize::new(result.width, result.height),
                area.scale
            ),
            center
        );
    }

    #[test]
    fn disconnected_monitor_position_is_recovered_inside_available_work_area() {
        let result = layout(BallPosition { x: -5000, y: 6000 }, primary(1.0), false);
        assert_eq!(result.x, 0);
        assert_eq!(result.y + result.height as i32, 1040);
    }

    #[test]
    fn small_work_area_never_creates_a_window_larger_than_available_space() {
        let area = WorkArea {
            x: 0,
            y: 40,
            width: 360,
            height: 400,
            scale: 1.25,
        };
        let (result, _) = preview_layout(BallPosition { x: 100, y: 100 }, area);
        assert_eq!(
            result,
            Layout {
                x: 0,
                y: 40,
                width: 360,
                height: 400
            }
        );
    }

    #[test]
    fn position_file_survives_overwrite_and_accepts_negative_coordinates() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("floating-position.json");
        write_position(
            &path,
            SavedPosition {
                x: 100,
                y: 50,
                dock: None,
                hidden: false,
            },
        )
        .unwrap();
        let expected = SavedPosition {
            x: -1200,
            y: -80,
            dock: Some(DockSide::Right),
            hidden: true,
        };
        write_position(&path, expected).unwrap();
        assert_eq!(read_position(&path), Some(expected));
        fs::write(&path, br#"{"x":12,"y":34}"#).unwrap();
        assert_eq!(
            read_position(&path),
            Some(SavedPosition {
                x: 12,
                y: 34,
                dock: None,
                hidden: false
            })
        );
        fs::write(&path, vec![b' '; 257]).unwrap();
        assert_eq!(read_position(&path), None);
    }

    #[test]
    fn hidden_window_exposes_thirty_percent_of_the_ball_and_keeps_its_height() {
        let area = primary(1.0);
        let center = BallPosition { x: 1858, y: 500 };
        let right = dock_layout(center, area, DockSide::Right, true);
        assert_eq!(right.width, 19); // round(64 * 30%)
        assert_eq!(right.height, 64);
        assert_eq!(right.x + right.width as i32, 1920);
        assert_eq!(right.y, 468);
        let left = dock_layout(center, area, DockSide::Left, true);
        assert_eq!(left.x, 0);
    }

    #[test]
    fn docking_requires_strictly_less_than_seventy_percent_visible() {
        let area = primary(1.0);
        for visible in [0.0, 0.30, 0.69, 0.699] {
            assert_eq!(
                detect_dock_edges(-64.0 * (1.0 - visible), 64.0, area),
                Some(DockSide::Left)
            );
            assert_eq!(
                detect_dock_edges(1920.0 - 64.0 * visible, 64.0, area),
                Some(DockSide::Right)
            );
        }
        for visible in [0.70, 0.701, 0.71, 1.0] {
            assert_eq!(detect_dock_edges(-64.0 * (1.0 - visible), 64.0, area), None);
            assert_eq!(detect_dock_edges(1920.0 - 64.0 * visible, 64.0, area), None);
        }
    }

    #[test]
    fn transparent_window_margins_and_top_bottom_do_not_trigger_docking() {
        let area = primary(1.0);
        for x in [0, -14, -33] {
            let center = ball_position(
                PhysicalPosition::new(x, 100),
                PhysicalSize::new(108, 88),
                1.0,
            );
            assert_eq!(detect_dock(center, area), None);
        }
        let center = ball_position(
            PhysicalPosition::new(-34, 100),
            PhysicalSize::new(108, 88),
            1.0,
        );
        assert_eq!(detect_dock(center, area), Some(DockSide::Left));
        for y in [-500, 2000] {
            assert_eq!(detect_dock(BallPosition { x: 800, y }, area), None);
        }
    }

    #[test]
    fn ratios_and_reveal_work_on_both_edges_at_different_scales() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let area = WorkArea {
                x: -2560,
                y: -200,
                width: 2560,
                height: 1440,
                scale,
            };
            let diameter = BALL_DIAMETER * scale;
            assert_eq!(
                detect_dock_edges(-2560.0 - diameter * 0.31, diameter, area),
                Some(DockSide::Left)
            );
            assert_eq!(
                detect_dock_edges(-diameter * 0.69, diameter, area),
                Some(DockSide::Right)
            );
            for side in [DockSide::Left, DockSide::Right] {
                let center = BallPosition { x: -100, y: 500 };
                let hidden = dock_layout(center, area, side, true);
                let visible = dock_layout(center, area, side, false);
                assert!((hidden.width as f64 - diameter * 0.30).abs() <= 0.5);
                assert_eq!(visible.width, diameter.round() as u32);
                assert_eq!(hidden.height, visible.height);
                assert_eq!(hidden.y, visible.y);
                // The narrow strip is contained in the revealed ball, with no gap.
                assert!(hidden.x >= visible.x);
                assert!(hidden.x + hidden.width as i32 <= visible.x + visible.width as i32);
                assert!(visible.x >= area.x && visible.x + visible.width as i32 <= 0);
            }
        }
    }

    #[test]
    fn vertical_docked_drag_retains_side_but_dragging_inward_restores_free_mode() {
        let area = primary(1.0);
        assert_eq!(
            settled_side(BallPosition { x: 32, y: 800 }, area, Some(DockSide::Left)),
            Some(DockSide::Left)
        );
        assert_eq!(
            settled_side(
                BallPosition { x: 1888, y: 200 },
                area,
                Some(DockSide::Right)
            ),
            Some(DockSide::Right)
        );
        assert_eq!(
            settled_side(BallPosition { x: 80, y: 800 }, area, Some(DockSide::Left)),
            None
        );
        assert_eq!(
            settled_side(
                BallPosition { x: 1800, y: 200 },
                area,
                Some(DockSide::Right)
            ),
            None
        );
        assert_eq!(
            settled_side(BallPosition { x: 32, y: 800 }, area, None),
            None
        );
    }

    #[test]
    fn native_hover_reveals_and_leave_waits_five_hundred_milliseconds() {
        let now = Instant::now();
        let mut dock = DockState {
            side: Some(DockSide::Left),
            ..Default::default()
        };
        assert_eq!(hover_action(&mut dock, now, false, false), DockAction::None);
        assert_eq!(
            hover_action(&mut dock, now + Duration::from_millis(499), false, false),
            DockAction::None
        );
        assert_eq!(
            hover_action(&mut dock, now + Duration::from_millis(500), false, false),
            DockAction::Hide
        );
        dock.hidden = true;
        assert_eq!(
            hover_action(&mut dock, now + Duration::from_secs(1), true, false),
            DockAction::Reveal
        );
        dock.hidden = false;
        assert_eq!(
            hover_action(&mut dock, now + Duration::from_secs(2), true, false),
            DockAction::None
        );
        assert_eq!(dock.outside_since, None);
    }

    #[test]
    fn preview_menu_and_drag_protection_restart_the_leave_delay() {
        let now = Instant::now();
        let mut dock = DockState {
            side: Some(DockSide::Right),
            ..Default::default()
        };
        hover_action(&mut dock, now, false, false);
        assert_eq!(
            hover_action(&mut dock, now + Duration::from_secs(1), false, true),
            DockAction::None
        );
        assert_eq!(dock.outside_since, None);
        assert_eq!(
            hover_action(&mut dock, now + Duration::from_secs(2), false, false),
            DockAction::None
        );
        dock.side = None;
        assert_eq!(
            hover_action(&mut dock, now + Duration::from_secs(3), false, false),
            DockAction::None
        );
    }

    #[test]
    fn revealed_ball_preview_tracks_current_side_and_vertical_position() {
        let area = primary(1.0);
        for side in [DockSide::Left, DockSide::Right] {
            for y in [150, 900] {
                let full = dock_layout(BallPosition { x: 500, y }, area, side, false);
                let actual = BallPosition {
                    x: full.x + 32,
                    y: full.y + 32,
                };
                let (preview, above) = preview_layout(actual, area);
                assert_eq!(above, y == 900);
                assert!(preview.x >= area.x && preview.x + preview.width as i32 <= 1920);
                assert!(preview.y >= area.y && preview.y + preview.height as i32 <= 1040);
                if side == DockSide::Left {
                    assert_eq!(preview.x, 0);
                } else {
                    assert_eq!(preview.x + preview.width as i32, 1920);
                }
            }
        }
    }
}
