mod codex;
mod models;

use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

use models::{ProviderSnapshot, WidgetPreferences};
#[cfg(debug_assertions)]
use models::UsageWindow;
use serde::Deserialize;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_window_state::Builder as WindowStateBuilder;

// These are the light-theme visual dimensions. They deliberately do not vary
// by appearance: a theme changes colours only, while the native window keeps
// the same footprint as its CSS content.
const COLLAPSED_LOGICAL_SIZE: f64 = 72.0;
const EXPANDED_LOGICAL_SIZE: f64 = 306.0;
const EDGE_SAFE_INSET_LOGICAL: f64 = 4.0;
const SNAP_THRESHOLD_LOGICAL: f64 = 24.0;
const POSITION_EPSILON: u32 = 2;

#[derive(Clone, Copy)]
enum HorizontalDock {
    Left,
    Right,
}

#[derive(Clone, Copy)]
enum VerticalDock {
    Top,
    Bottom,
}

#[derive(Clone, Copy, Default)]
struct DockState {
    horizontal: Option<HorizontalDock>,
    vertical: Option<VerticalDock>,
}

impl DockState {
    fn is_docked(self) -> bool {
        self.horizontal.is_some() || self.vertical.is_some()
    }
}

#[derive(Clone, Copy)]
struct WidgetRect {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

#[derive(Clone, Copy, Deserialize)]
struct WorkAreaPoint {
    x: i32,
    y: i32,
}

#[derive(Clone, Copy, Deserialize)]
struct WorkAreaSize {
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, Deserialize)]
struct WorkAreaPayload {
    position: WorkAreaPoint,
    size: WorkAreaSize,
}

#[derive(Clone, Copy)]
enum WidgetMode {
    Collapsed,
    Expanded,
}

#[derive(Clone, Copy)]
struct WidgetGeometryState {
    mode: WidgetMode,
    dock: DockState,
    collapsed_rect: WidgetRect,
    expanded_rect: Option<WidgetRect>,
}

struct AppState {
    client: reqwest::Client,
    preferences: Mutex<WidgetPreferences>,
    preferences_path: PathBuf,
    fetch_lock: tokio::sync::Mutex<()>,
    snapshot_cache: Mutex<Option<(Instant, Vec<ProviderSnapshot>)>>,
    #[cfg(debug_assertions)]
    simulate_short_window_for_testing: Mutex<bool>,
    geometry: Mutex<Option<WidgetGeometryState>>,
    drag_mode: Mutex<Option<WidgetMode>>,
    tray_menu: Mutex<Option<TrayMenuState>>,
}

#[derive(Clone)]
struct TrayMenuState {
    show: CheckMenuItem<tauri::Wry>,
    refresh: MenuItem<tauri::Wry>,
    always_on_top: CheckMenuItem<tauri::Wry>,
    locked: CheckMenuItem<tauri::Wry>,
    pinned: CheckMenuItem<tauri::Wry>,
    autostart: CheckMenuItem<tauri::Wry>,
    settings: Submenu<tauri::Wry>,
    language: Submenu<tauri::Wry>,
    language_zh_cn: CheckMenuItem<tauri::Wry>,
    language_zh_tw: CheckMenuItem<tauri::Wry>,
    language_en: CheckMenuItem<tauri::Wry>,
    appearance: Submenu<tauri::Wry>,
    theme_system: CheckMenuItem<tauri::Wry>,
    theme_dark: CheckMenuItem<tauri::Wry>,
    theme_light: CheckMenuItem<tauri::Wry>,
    skin_blur: CheckMenuItem<tauri::Wry>,
    skin_computer: CheckMenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
    #[cfg(debug_assertions)]
    test_short_window: CheckMenuItem<tauri::Wry>,
}

struct TrayLabels {
    show: &'static str,
    refresh: &'static str,
    settings: &'static str,
    always_on_top: &'static str,
    locked: &'static str,
    pinned: &'static str,
    autostart: &'static str,
    language: &'static str,
    zh_cn: &'static str,
    zh_tw: &'static str,
    english: &'static str,
    appearance: &'static str,
    system: &'static str,
    dark: &'static str,
    light: &'static str,
    blur: &'static str,
    computer: &'static str,
    quit: &'static str,
    #[cfg(debug_assertions)]
    debug_short_window: &'static str,
}

fn tray_labels(language: &str) -> TrayLabels {
    match language {
        "zh-TW" => TrayLabels {
            show: "顯示", refresh: "立即更新", settings: "功能設定",
            always_on_top: "置頂", locked: "鎖定", pinned: "固定", autostart: "開機自動啟動",
            language: "切換語言 / Swtich Language", zh_cn: "簡體中文", zh_tw: "繁體中文", english: "English",
            appearance: "外觀", system: "跟隨系統", dark: "深色", light: "淺色",
            blur: "模糊", computer: "電腦", quit: "退出",
            #[cfg(debug_assertions)]
            debug_short_window: "測試：模擬 5 小時額度",
        },
        "en" => TrayLabels {
            show: "Show", refresh: "Refresh Now", settings: "Feature Settings",
            always_on_top: "Always on Top", locked: "Lock", pinned: "Pin", autostart: "Launch at Startup",
            language: "Switch Language", zh_cn: "Simplified Chinese", zh_tw: "Traditional Chinese", english: "English",
            appearance: "Appearance", system: "Follow System", dark: "Dark", light: "Light",
            blur: "Blur", computer: "Computer", quit: "Quit",
            #[cfg(debug_assertions)]
            debug_short_window: "Test: Simulate 5-hour Quota",
        },
        _ => TrayLabels {
            show: "显示", refresh: "立即刷新", settings: "功能设置",
            always_on_top: "置顶", locked: "锁定", pinned: "固定", autostart: "开机自启",
            language: "切换语言 / Swtich Language", zh_cn: "简体中文", zh_tw: "繁体中文", english: "English",
            appearance: "外观", system: "跟随系统", dark: "深色", light: "浅色",
            blur: "模糊", computer: "电脑", quit: "退出",
            #[cfg(debug_assertions)]
            debug_short_window: "测试：模拟 5 小时额度",
        },
    }
}

fn update_tray_menu(menu: &TrayMenuState, language: &str, _visible: bool) {
    let labels = tray_labels(language);
    let _ = menu.show.set_text(labels.show);
    let _ = menu.refresh.set_text(labels.refresh);
    let _ = menu.settings.set_text(labels.settings);
    let _ = menu.always_on_top.set_text(labels.always_on_top);
    let _ = menu.locked.set_text(labels.locked);
    let _ = menu.pinned.set_text(labels.pinned);
    let _ = menu.autostart.set_text(labels.autostart);
    let _ = menu.language.set_text(labels.language);
    let _ = menu.language_zh_cn.set_text(labels.zh_cn);
    let _ = menu.language_zh_tw.set_text(labels.zh_tw);
    let _ = menu.language_en.set_text(labels.english);
    let _ = menu.language_zh_cn.set_checked(language == "zh-CN");
    let _ = menu.language_zh_tw.set_checked(language == "zh-TW");
    let _ = menu.language_en.set_checked(language == "en");
    let _ = menu.appearance.set_text(labels.appearance);
    let _ = menu.theme_system.set_text(labels.system);
    let _ = menu.theme_dark.set_text(labels.dark);
    let _ = menu.theme_light.set_text(labels.light);
    let _ = menu.skin_blur.set_text(labels.blur);
    let _ = menu.skin_computer.set_text(labels.computer);
    let _ = menu.quit.set_text(labels.quit);
    #[cfg(debug_assertions)]
    let _ = menu.test_short_window.set_text(labels.debug_short_window);
}

fn sync_show_menu_state(app: &AppHandle, visible: bool) {
    if let Some(state) = app.try_state::<AppState>() {
        let language = preferences_lock(state.inner()).language.clone();
        if let Ok(tray_menu) = state.tray_menu.lock() {
            if let Some(menu) = tray_menu.as_ref() {
                let _ = menu.show.set_checked(visible);
                update_tray_menu(menu, &language, visible);
            }
        }
    }
}

fn apply_short_window_test_override(
    _state: &AppState,
    #[allow(unused_mut)]
    mut snapshots: Vec<ProviderSnapshot>,
) -> Vec<ProviderSnapshot> {
    #[cfg(debug_assertions)]
    if _state
        .simulate_short_window_for_testing
        .lock()
        .map(|value| *value)
        .unwrap_or(false)
    {
        for snapshot in &mut snapshots {
            if snapshot.status == "ok" {
                snapshot.short_window = Some(UsageWindow {
                    remaining_percent: 88.0,
                    resets_at: Some((chrono::Utc::now() + chrono::Duration::hours(3)).to_rfc3339()),
                    window_seconds: 18_000,
                });
            }
        }
    }
    snapshots
}

async fn fetch_snapshots_uncached(state: &State<'_, AppState>) -> Vec<ProviderSnapshot> {
    let _guard = state.fetch_lock.lock().await;
    let values = vec![codex::fetch_snapshot(&state.client).await];
    if let Ok(mut cache) = state.snapshot_cache.lock() {
        *cache = Some((Instant::now(), values.clone()));
    }
    apply_short_window_test_override(state.inner(), values)
}

fn load_preferences(path: &PathBuf) -> WidgetPreferences {
    let parse = |candidate: &PathBuf| {
        fs::read_to_string(candidate)
            .ok()
            .and_then(|raw| serde_json::from_str::<WidgetPreferences>(&raw).ok())
    };
    if let Some(value) = parse(path) {
        return value.normalized();
    }
    let backup = path.with_extension("json.bak");
    if let Some(value) = parse(&backup) {
        eprintln!("preferences recovered from backup");
        return value.normalized();
    }
    WidgetPreferences::default()
}

fn preferences_lock(state: &AppState) -> MutexGuard<'_, WidgetPreferences> {
    state.preferences.lock().unwrap_or_else(|poisoned| {
        eprintln!("preferences lock was poisoned; recovering the last in-memory settings");
        poisoned.into_inner()
    })
}

fn persist_preferences(path: &PathBuf, value: &WidgetPreferences) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|_| "failed to create settings directory".to_string())?;
    }
    let serialized =
        serde_json::to_vec_pretty(value).map_err(|_| "failed to serialize settings".to_string())?;
    let temporary = path.with_extension("json.tmp");
    let backup = path.with_extension("json.bak");
    let mut file = fs::File::create(&temporary)
        .map_err(|_| "failed to create temporary settings file".to_string())?;
    file.write_all(&serialized)
        .and_then(|_| file.sync_all())
        .map_err(|_| "failed to write settings".to_string())?;
    if path.exists() {
        let _ = fs::remove_file(&backup);
        fs::rename(path, &backup).map_err(|_| "failed to back up settings".to_string())?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::rename(&backup, path);
        return Err(format!("failed to commit settings: {error}"));
    }
    Ok(())
}

#[tauri::command]
async fn get_snapshots(state: State<'_, AppState>) -> Result<Vec<ProviderSnapshot>, String> {
    const CACHE_TTL: Duration = Duration::from_secs(30);
    if let Ok(cache) = state.snapshot_cache.lock() {
        if let Some((time, values)) = &*cache {
            if time.elapsed() < CACHE_TTL {
                return Ok(apply_short_window_test_override(&state, values.clone()));
            }
        }
    }
    let _guard = match state.fetch_lock.try_lock() {
        Ok(guard) => guard,
        Err(_) => {
            if let Ok(cache) = state.snapshot_cache.lock() {
                if let Some((_, values)) = &*cache {
                    return Ok(apply_short_window_test_override(&state, values.clone()));
                }
            }
            return Ok(vec![ProviderSnapshot::failure(
                "unavailable",
                "Quota refresh is already running.",
            )]);
        }
    };
    if let Ok(cache) = state.snapshot_cache.lock() {
        if let Some((time, values)) = &*cache {
            if time.elapsed() < CACHE_TTL {
                return Ok(apply_short_window_test_override(&state, values.clone()));
            }
        }
    }
    let values = vec![codex::fetch_snapshot(&state.client).await];
    if let Ok(mut cache) = state.snapshot_cache.lock() {
        *cache = Some((Instant::now(), values.clone()));
    }
    Ok(apply_short_window_test_override(&state, values))
}

#[tauri::command]
async fn refresh_snapshots(state: State<'_, AppState>) -> Result<Vec<ProviderSnapshot>, String> {
    Ok(fetch_snapshots_uncached(&state).await)
}

fn clamp_position_to_monitor(
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    monitor: &tauri::Monitor,
    safe_inset: i32,
) -> PhysicalPosition<i32> {
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let left = monitor_position.x;
    let top = monitor_position.y;
    let right = left + monitor_size.width as i32;
    let bottom = top + monitor_size.height as i32;
    PhysicalPosition::new(
        position
            .x
            .clamp(left - safe_inset, right - size.width as i32 + safe_inset),
        position
            .y
            .clamp(top - safe_inset, bottom - size.height as i32 + safe_inset),
    )
}

fn logical_to_physical(value: f64, scale_factor: f64) -> u32 {
    (value * scale_factor).round().max(1.0) as u32
}

fn safe_inset_for_current_appearance(state: &AppState, scale_factor: f64) -> u32 {
    let _ = state;
    logical_to_physical(EDGE_SAFE_INSET_LOGICAL, scale_factor)
}

fn window_size_for_visual_size(visual_size: u32, safe_inset: u32) -> u32 {
    visual_size + safe_inset * 2
}

fn widget_window_size(logical_visual_size: f64, scale_factor: f64, safe_inset: u32) -> u32 {
    window_size_for_visual_size(
        logical_to_physical(logical_visual_size, scale_factor),
        safe_inset,
    )
}

fn detect_dock(
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    monitor: &tauri::Monitor,
    threshold: i32,
    safe_inset: i32,
) -> DockState {
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let visible_left = position.x + safe_inset;
    let visible_top = position.y + safe_inset;
    let visible_right = position.x + size.width as i32 - safe_inset;
    let visible_bottom = position.y + size.height as i32 - safe_inset;
    let left_distance = (visible_left - monitor_position.x).abs();
    let top_distance = (visible_top - monitor_position.y).abs();
    let right_distance = (monitor_position.x + monitor_size.width as i32 - visible_right).abs();
    let bottom_distance = (monitor_position.y + monitor_size.height as i32 - visible_bottom).abs();
    let horizontal = if left_distance <= threshold || right_distance <= threshold {
        if left_distance <= right_distance {
            Some(HorizontalDock::Left)
        } else {
            Some(HorizontalDock::Right)
        }
    } else {
        None
    };
    let vertical = if top_distance <= threshold || bottom_distance <= threshold {
        if top_distance <= bottom_distance {
            Some(VerticalDock::Top)
        } else {
            Some(VerticalDock::Bottom)
        }
    } else {
        None
    };
    DockState {
        horizontal,
        vertical,
    }
}

fn snap_position(
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    dock: DockState,
    monitor: &tauri::Monitor,
    safe_inset: i32,
) -> PhysicalPosition<i32> {
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let mut next = clamp_position_to_monitor(position, size, monitor, safe_inset);
    match dock.horizontal {
        Some(HorizontalDock::Left) => next.x = monitor_position.x - safe_inset,
        Some(HorizontalDock::Right) => {
            next.x = monitor_position.x + monitor_size.width as i32 - size.width as i32 + safe_inset
        }
        None => {}
    }
    match dock.vertical {
        Some(VerticalDock::Top) => next.y = monitor_position.y - safe_inset,
        Some(VerticalDock::Bottom) => {
            next.y =
                monitor_position.y + monitor_size.height as i32 - size.height as i32 + safe_inset
        }
        None => {}
    }
    next
}

fn expanded_position_in_bounds(
    collapsed: WidgetRect,
    expanded_size: PhysicalSize<u32>,
    dock: DockState,
    bounds_position: PhysicalPosition<i32>,
    bounds_size: PhysicalSize<u32>,
    safe_inset: i32,
) -> PhysicalPosition<i32> {
    let monitor_right = bounds_position.x + bounds_size.width as i32;
    let monitor_bottom = bounds_position.y + bounds_size.height as i32;
    let collapsed_left = collapsed.position.x + safe_inset;
    let collapsed_top = collapsed.position.y + safe_inset;
    let collapsed_right = collapsed.position.x + collapsed.size.width as i32 - safe_inset;
    let collapsed_bottom = collapsed.position.y + collapsed.size.height as i32 - safe_inset;
    let x = match dock.horizontal {
        Some(HorizontalDock::Left) => collapsed_left - safe_inset,
        Some(HorizontalDock::Right) => collapsed_right - expanded_size.width as i32 + safe_inset,
        None if collapsed_left + expanded_size.width as i32 - safe_inset > monitor_right => {
            collapsed_right - expanded_size.width as i32 + safe_inset
        }
        None => collapsed_left - safe_inset,
    };
    let y = match dock.vertical {
        Some(VerticalDock::Top) => collapsed_top - safe_inset,
        Some(VerticalDock::Bottom) => collapsed_bottom - expanded_size.height as i32 + safe_inset,
        None if collapsed_top + expanded_size.height as i32 - safe_inset > monitor_bottom => {
            collapsed_bottom - expanded_size.height as i32 + safe_inset
        }
        None => collapsed_top - safe_inset,
    };
    let min_x = bounds_position.x - safe_inset;
    let min_y = bounds_position.y - safe_inset;
    let max_x = (monitor_right - expanded_size.width as i32 + safe_inset).max(min_x);
    let max_y = (monitor_bottom - expanded_size.height as i32 + safe_inset).max(min_y);
    PhysicalPosition::new(x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}

fn expanded_position(
    collapsed: WidgetRect,
    expanded_size: PhysicalSize<u32>,
    dock: DockState,
    monitor: &tauri::Monitor,
    work_area: Option<WorkAreaPayload>,
    safe_inset: i32,
) -> PhysicalPosition<i32> {
    let (bounds_position, bounds_size) = work_area
        .map(|area| {
            (
                PhysicalPosition::new(area.position.x, area.position.y),
                PhysicalSize::new(area.size.width, area.size.height),
            )
        })
        .unwrap_or_else(|| (*monitor.position(), *monitor.size()));
    expanded_position_in_bounds(
        collapsed,
        expanded_size,
        dock,
        bounds_position,
        bounds_size,
        safe_inset,
    )
}

fn collapsed_geometry_for_expand(
    current_position: PhysicalPosition<i32>,
    collapsed_size: PhysicalSize<u32>,
    monitor: &tauri::Monitor,
    threshold: i32,
    safe_inset: i32,
    previous: Option<WidgetGeometryState>,
) -> (WidgetRect, DockState) {
    if let Some(previous) = previous {
        let position = if previous.dock.is_docked() {
            snap_position(
                previous.collapsed_rect.position,
                collapsed_size,
                previous.dock,
                monitor,
                safe_inset,
            )
        } else {
            clamp_position_to_monitor(
                previous.collapsed_rect.position,
                collapsed_size,
                monitor,
                safe_inset,
            )
        };
        return (
            WidgetRect {
                position,
                size: collapsed_size,
            },
            previous.dock,
        );
    }

    let current_collapsed = WidgetRect {
        position: clamp_position_to_monitor(current_position, collapsed_size, monitor, safe_inset),
        size: collapsed_size,
    };
    let dock = detect_dock(
        current_collapsed.position,
        collapsed_size,
        monitor,
        threshold,
        safe_inset,
    );
    let position = if dock.is_docked() {
        snap_position(
            current_collapsed.position,
            collapsed_size,
            dock,
            monitor,
            safe_inset,
        )
    } else {
        current_collapsed.position
    };
    (
        WidgetRect {
            position,
            size: collapsed_size,
        },
        dock,
    )
}

fn current_widget_rect(window: &tauri::WebviewWindow) -> Result<WidgetRect, String> {
    Ok(WidgetRect {
        position: window
            .outer_position()
            .map_err(|_| "failed to read widget position".to_string())?,
        size: window
            .outer_size()
            .map_err(|_| "failed to read widget size".to_string())?,
    })
}

fn monitor_and_scale(
    window: &tauri::WebviewWindow,
) -> Result<(Option<tauri::Monitor>, f64), String> {
    let monitor = window
        .current_monitor()
        .map_err(|_| "failed to read monitor".to_string())?;
    let scale_factor = monitor
        .as_ref()
        .map(|item| item.scale_factor())
        .unwrap_or(1.0);
    Ok((monitor, scale_factor))
}

fn infer_mode(rect: WidgetRect, collapsed_size: PhysicalSize<u32>) -> WidgetMode {
    if rect.size.width <= collapsed_size.width + POSITION_EPSILON
        && rect.size.height <= collapsed_size.height + POSITION_EPSILON
    {
        WidgetMode::Collapsed
    } else {
        WidgetMode::Expanded
    }
}

fn collapsed_restore_candidate(
    current_position: PhysicalPosition<i32>,
    previous: Option<WidgetGeometryState>,
) -> PhysicalPosition<i32> {
    previous
        .map(|value| value.collapsed_rect.position)
        .unwrap_or(current_position)
}

#[tauri::command]
fn expand_widget(
    work_area: Option<WorkAreaPayload>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if preferences_lock(state.inner()).locked {
        return Ok(());
    }
    let window = app
        .get_webview_window("widget")
        .ok_or_else(|| "widget window missing".to_string())?;
    let current = current_widget_rect(&window)?;
    let (monitor, scale_factor) = monitor_and_scale(&window)?;
    let safe_inset = safe_inset_for_current_appearance(state.inner(), scale_factor);
    let collapsed_size = PhysicalSize::new(
        widget_window_size(COLLAPSED_LOGICAL_SIZE, scale_factor, safe_inset),
        widget_window_size(COLLAPSED_LOGICAL_SIZE, scale_factor, safe_inset),
    );
    let expanded_size = PhysicalSize::new(
        widget_window_size(EXPANDED_LOGICAL_SIZE, scale_factor, safe_inset),
        widget_window_size(EXPANDED_LOGICAL_SIZE, scale_factor, safe_inset),
    );
    let Some(monitor) = monitor else {
        window
            .set_size(expanded_size)
            .map_err(|_| "failed to resize widget".to_string())?;
        return Ok(());
    };
    let threshold = logical_to_physical(SNAP_THRESHOLD_LOGICAL, scale_factor) as i32;
    let previous = state.geometry.lock().ok().and_then(|value| *value);
    let (collapsed_rect, dock) = collapsed_geometry_for_expand(
        current.position,
        collapsed_size,
        &monitor,
        threshold,
        safe_inset as i32,
        previous,
    );
    let expanded_rect = WidgetRect {
        position: expanded_position(
            collapsed_rect,
            expanded_size,
            dock,
            &monitor,
            work_area,
            safe_inset as i32,
        ),
        size: expanded_size,
    };

    if let Ok(mut geometry) = state.geometry.lock() {
        *geometry = Some(WidgetGeometryState {
            mode: WidgetMode::Expanded,
            dock,
            collapsed_rect,
            expanded_rect: Some(expanded_rect),
        });
    }

    window
        .set_position(expanded_rect.position)
        .map_err(|_| "failed to position widget".to_string())?;
    window
        .set_size(expanded_size)
        .map_err(|_| "failed to resize widget".to_string())
}

#[cfg(test)]
mod geometry_tests {
    use super::*;

    #[test]
    fn visibility_action_is_always_labeled_show() {
        assert_eq!(tray_labels("zh-CN").show, "显示");
        assert_eq!(tray_labels("zh-TW").show, "顯示");
        assert_eq!(tray_labels("en").show, "Show");
    }

    fn rect(x: i32, y: i32, size: u32) -> WidgetRect {
        WidgetRect {
            position: PhysicalPosition::new(x, y),
            size: PhysicalSize::new(size, size),
        }
    }

    #[test]
    fn window_size_includes_the_transparent_safe_inset() {
        assert_eq!(window_size_for_visual_size(72, 4), 80);
        assert_eq!(widget_window_size(306.0, 1.5, 6), 471);
    }

    #[test]
    fn expansion_stays_above_a_bottom_taskbar() {
        let position = expanded_position_in_bounds(
            rect(1844, 964, 80),
            PhysicalSize::new(314, 314),
            DockState {
                horizontal: Some(HorizontalDock::Right),
                vertical: Some(VerticalDock::Bottom),
            },
            PhysicalPosition::new(0, 0),
            PhysicalSize::new(1920, 1040),
            4,
        );
        assert_eq!(position, PhysicalPosition::new(1610, 730));
    }

    #[test]
    fn expansion_handles_negative_origin_work_areas() {
        let position = expanded_position_in_bounds(
            rect(-1284, -4, 80),
            PhysicalSize::new(314, 314),
            DockState {
                horizontal: Some(HorizontalDock::Left),
                vertical: Some(VerticalDock::Top),
            },
            PhysicalPosition::new(-1280, 0),
            PhysicalSize::new(1280, 984),
            4,
        );
        assert_eq!(position, PhysicalPosition::new(-1284, -4));
    }

    #[test]
    fn undocked_expansion_flips_inward_near_work_area_edges() {
        let position = expanded_position_in_bounds(
            rect(1750, 900, 80),
            PhysicalSize::new(314, 314),
            DockState::default(),
            PhysicalPosition::new(0, 0),
            PhysicalSize::new(1920, 1040),
            4,
        );
        assert_eq!(position, PhysicalPosition::new(1516, 666));
    }

    #[test]
    fn collapse_restores_the_original_anchor_after_expanded_window_moves() {
        let previous = WidgetGeometryState {
            mode: WidgetMode::Expanded,
            dock: DockState::default(),
            collapsed_rect: rect(420, 260, 80),
            expanded_rect: Some(rect(650, 500, 314)),
        };
        let candidate = collapsed_restore_candidate(
            PhysicalPosition::new(650, 500),
            Some(previous),
        );
        assert_eq!(candidate, PhysicalPosition::new(420, 260));
    }
}

#[tauri::command]
fn collapse_widget(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if preferences_lock(state.inner()).locked {
        return Ok(());
    }
    let window = app
        .get_webview_window("widget")
        .ok_or_else(|| "widget window missing".to_string())?;
    let current = current_widget_rect(&window)?;
    let (monitor, scale_factor) = monitor_and_scale(&window)?;
    let safe_inset = safe_inset_for_current_appearance(state.inner(), scale_factor);
    let collapsed_size = PhysicalSize::new(
        widget_window_size(COLLAPSED_LOGICAL_SIZE, scale_factor, safe_inset),
        widget_window_size(COLLAPSED_LOGICAL_SIZE, scale_factor, safe_inset),
    );
    let Some(monitor) = monitor else {
        window
            .set_size(collapsed_size)
            .map_err(|_| "failed to resize widget".to_string())?;
        return Ok(());
    };
    let threshold = logical_to_physical(SNAP_THRESHOLD_LOGICAL, scale_factor) as i32;
    let previous = state.geometry.lock().ok().and_then(|value| *value);
    let candidate = collapsed_restore_candidate(current.position, previous);
    let dock = detect_dock(
        candidate,
        collapsed_size,
        &monitor,
        threshold,
        safe_inset as i32,
    );
    let next_position = if dock.is_docked() {
        snap_position(candidate, collapsed_size, dock, &monitor, safe_inset as i32)
    } else {
        clamp_position_to_monitor(candidate, collapsed_size, &monitor, safe_inset as i32)
    };
    let collapsed_rect = WidgetRect {
        position: next_position,
        size: collapsed_size,
    };
    if let Ok(mut geometry) = state.geometry.lock() {
        *geometry = Some(WidgetGeometryState {
            mode: WidgetMode::Collapsed,
            dock,
            collapsed_rect,
            expanded_rect: None,
        });
    }
    window
        .set_size(collapsed_size)
        .map_err(|_| "failed to resize widget".to_string())?;
    window
        .set_position(next_position)
        .map_err(|_| "failed to position widget".to_string())
}

#[tauri::command]
fn begin_widget_drag(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if preferences_lock(state.inner()).locked {
        return Ok(());
    }
    let window = app
        .get_webview_window("widget")
        .ok_or_else(|| "widget window missing".to_string())?;
    let current = current_widget_rect(&window)?;
    let (_, scale_factor) = monitor_and_scale(&window)?;
    let safe_inset = safe_inset_for_current_appearance(state.inner(), scale_factor);
    let collapsed_size = PhysicalSize::new(
        widget_window_size(COLLAPSED_LOGICAL_SIZE, scale_factor, safe_inset),
        widget_window_size(COLLAPSED_LOGICAL_SIZE, scale_factor, safe_inset),
    );
    let mode = state
        .geometry
        .lock()
        .ok()
        .and_then(|value| *value)
        .map(|value| value.mode)
        .unwrap_or_else(|| infer_mode(current, collapsed_size));
    if let Ok(mut drag_mode) = state.drag_mode.lock() {
        *drag_mode = Some(mode);
    }
    Ok(())
}

#[tauri::command]
fn finish_widget_drag(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if preferences_lock(state.inner()).locked {
        return Ok(());
    }
    let window = app
        .get_webview_window("widget")
        .ok_or_else(|| "widget window missing".to_string())?;
    let current = current_widget_rect(&window)?;
    let (monitor, scale_factor) = monitor_and_scale(&window)?;
    let Some(monitor) = monitor else {
        return Ok(());
    };
    let threshold = logical_to_physical(SNAP_THRESHOLD_LOGICAL, scale_factor) as i32;
    let safe_inset = safe_inset_for_current_appearance(state.inner(), scale_factor);
    let collapsed_size = PhysicalSize::new(
        widget_window_size(COLLAPSED_LOGICAL_SIZE, scale_factor, safe_inset),
        widget_window_size(COLLAPSED_LOGICAL_SIZE, scale_factor, safe_inset),
    );
    let expanded_size = PhysicalSize::new(
        widget_window_size(EXPANDED_LOGICAL_SIZE, scale_factor, safe_inset),
        widget_window_size(EXPANDED_LOGICAL_SIZE, scale_factor, safe_inset),
    );
    let mode = state
        .drag_mode
        .lock()
        .ok()
        .and_then(|mut value| value.take())
        .or_else(|| {
            state
                .geometry
                .lock()
                .ok()
                .and_then(|value| *value)
                .map(|value| value.mode)
        })
        .unwrap_or_else(|| infer_mode(current, collapsed_size));

    match mode {
        WidgetMode::Collapsed => {
            let dock = detect_dock(
                current.position,
                collapsed_size,
                &monitor,
                threshold,
                safe_inset as i32,
            );
            let next_position = if dock.is_docked() {
                snap_position(
                    current.position,
                    collapsed_size,
                    dock,
                    &monitor,
                    safe_inset as i32,
                )
            } else {
                clamp_position_to_monitor(
                    current.position,
                    collapsed_size,
                    &monitor,
                    safe_inset as i32,
                )
            };
            let collapsed_rect = WidgetRect {
                position: next_position,
                size: collapsed_size,
            };
            window
                .set_position(next_position)
                .map_err(|_| "failed to position widget".to_string())?;
            if let Ok(mut geometry) = state.geometry.lock() {
                *geometry = Some(WidgetGeometryState {
                    mode: WidgetMode::Collapsed,
                    dock,
                    collapsed_rect,
                    expanded_rect: None,
                });
            }
        }
        WidgetMode::Expanded => {
            let current_position = clamp_position_to_monitor(
                current.position,
                expanded_size,
                &monitor,
                safe_inset as i32,
            );
            let updated_rect = WidgetRect {
                position: current_position,
                size: expanded_size,
            };
            window
                .set_position(current_position)
                .map_err(|_| "failed to position widget".to_string())?;
            if let Ok(mut geometry) = state.geometry.lock() {
                if let Some(mut value) = *geometry {
                    value.mode = WidgetMode::Expanded;
                    value.expanded_rect = Some(updated_rect);
                    *geometry = Some(value);
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn get_preferences(state: State<'_, AppState>) -> WidgetPreferences {
    // Preferences are always recoverable: an empty or invalid on-disk file
    // was normalized at startup, and a poisoned mutex is recovered above.
    // Do not turn a safe default into a user-facing startup error.
    preferences_lock(&state).clone()
}

#[tauri::command]
fn set_preferences(
    preferences: WidgetPreferences,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let preferences = preferences.normalized();
    persist_preferences(&state.preferences_path, &preferences)?;
    *preferences_lock(&state) = preferences.clone();
    if let Ok(tray_menu) = state.tray_menu.lock() {
        if let Some(menu) = tray_menu.as_ref() {
            let _ = menu.always_on_top.set_checked(preferences.always_on_top);
            let _ = menu.locked.set_checked(preferences.locked);
            let _ = menu.pinned.set_checked(preferences.pinned_provider.is_some());
            let visible = app.get_webview_window("widget")
                .and_then(|window| window.is_visible().ok())
                .unwrap_or(true);
            update_tray_menu(menu, &preferences.language, visible);
        }
    }
    Ok(())
}

fn apply_lock(app: &AppHandle, locked: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("widget")
        .ok_or_else(|| "widget window missing".to_string())?;
    window
        .set_ignore_cursor_events(locked)
        .map_err(|_| "failed to toggle click-through".to_string())
}

fn apply_always_on_top(app: &AppHandle, always_on_top: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("widget")
        .ok_or_else(|| "widget window missing".to_string())?;
    window
        .set_always_on_top(always_on_top)
        .map_err(|error| format!("failed to toggle always-on-top: {error}"))?;
    #[cfg(target_os = "macos")]
    if always_on_top {
        let ns_window = window
            .ns_window()
            .map_err(|error| format!("failed to access native widget window: {error}"))?;
        unsafe {
            use objc2_app_kit::NSWindow;
            (&*ns_window.cast::<NSWindow>()).orderFrontRegardless();
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn register_space_change_fronting(app: &tauri::App) {
    use block2::RcBlock;
    use objc2_app_kit::{
        NSWindow, NSWorkspace, NSWorkspaceActiveSpaceDidChangeNotification,
    };
    use objc2_foundation::NSNotification;
    use std::ptr::NonNull;

    let app_handle = app.handle().clone();
    let callback = RcBlock::new(move |_notification: NonNull<NSNotification>| {
        let app_handle = app_handle.clone();
        let main_thread_handle = app_handle.clone();
        let _ = app_handle.run_on_main_thread(move || {
            let should_front = main_thread_handle
                .try_state::<AppState>()
                .and_then(|state| state.preferences.lock().ok().map(|prefs| prefs.always_on_top))
                .unwrap_or(false);
            if !should_front {
                return;
            }
            let Some(window) = main_thread_handle.get_webview_window("widget") else {
                return;
            };
            if !window.is_visible().unwrap_or(false) {
                return;
            }
            let Ok(ns_window) = window.ns_window() else {
                return;
            };
            unsafe {
                (&*ns_window.cast::<NSWindow>()).orderFrontRegardless();
            }
        });
    });
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    let observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceActiveSpaceDidChangeNotification),
            None,
            None,
            &callback,
        )
    };
    // The observer belongs to the application lifetime.
    std::mem::forget(observer);
}

#[tauri::command]
fn set_widget_locked(
    locked: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WidgetPreferences, String> {
    let previous = state
        .preferences
        .lock()
        .map_err(|_| "settings unavailable".to_string())?
        .clone();
    let mut next = previous.clone();
    next.locked = locked;
    persist_preferences(&state.preferences_path, &next)?;
    if let Err(error) = apply_lock(&app, locked) {
        let _ = persist_preferences(&state.preferences_path, &previous);
        return Err(error);
    }
    *state
        .preferences
        .lock()
        .map_err(|_| "settings unavailable".to_string())? = next.clone();
    if let Ok(tray_menu) = state.tray_menu.lock() {
        if let Some(menu) = tray_menu.as_ref() {
            let _ = menu.locked.set_checked(next.locked);
        }
    }
    Ok(next)
}

#[tauri::command]
fn set_widget_always_on_top(
    always_on_top: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WidgetPreferences, String> {
    let previous = state
        .preferences
        .lock()
        .map_err(|_| "settings unavailable".to_string())?
        .clone();
    let mut next = previous.clone();
    next.always_on_top = always_on_top;
    persist_preferences(&state.preferences_path, &next)?;
    if let Err(error) = apply_always_on_top(&app, always_on_top) {
        let _ = persist_preferences(&state.preferences_path, &previous);
        return Err(error);
    }
    *state
        .preferences
        .lock()
        .map_err(|_| "settings unavailable".to_string())? = next.clone();
    if let Ok(tray_menu) = state.tray_menu.lock() {
        if let Some(menu) = tray_menu.as_ref() {
            let _ = menu.always_on_top.set_checked(next.always_on_top);
        }
    }
    let _ = app.emit_to("widget", "preferences-changed", next.clone());
    Ok(next)
}

#[tauri::command]
fn sync_widget_appearance(_appearance: String, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let window = app
        .get_webview_window("widget")
        .ok_or_else(|| "widget window missing".to_string())?;
    let current = current_widget_rect(&window)?;
    let (_, scale_factor) = monitor_and_scale(&window)?;
    let safe_inset = safe_inset_for_current_appearance(state.inner(), scale_factor);
    let expanded_threshold = logical_to_physical((COLLAPSED_LOGICAL_SIZE + EXPANDED_LOGICAL_SIZE) / 2.0, scale_factor);
    let visual_size = if current.size.width > expanded_threshold {
        EXPANDED_LOGICAL_SIZE
    } else {
        COLLAPSED_LOGICAL_SIZE
    };
    let side = widget_window_size(visual_size, scale_factor, safe_inset);
    window
        .set_size(PhysicalSize::new(side, side))
        .map_err(|_| "failed to resize widget for appearance".to_string())
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let initially_visible = app.get_webview_window("widget")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(true);
    let labels = tray_labels("zh-CN");
    let show = CheckMenuItem::with_id(app, "show", labels.show, true, initially_visible, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", labels.refresh, true, None::<&str>)?;
    let pin = CheckMenuItem::with_id(app, "pin", labels.pinned, true, false, None::<&str>)?;
    let always_on_top = CheckMenuItem::with_id(app, "always-on-top", labels.always_on_top, true, false, None::<&str>)?;
    let locked = CheckMenuItem::with_id(app, "locked", labels.locked, true, false, None::<&str>)?;
    let language_zh_cn = CheckMenuItem::with_id(app, "language-zh-CN", labels.zh_cn, true, true, None::<&str>)?;
    let language_zh_tw = CheckMenuItem::with_id(app, "language-zh-TW", labels.zh_tw, true, false, None::<&str>)?;
    let language_en = CheckMenuItem::with_id(app, "language-en", labels.english, true, false, None::<&str>)?;
    let language = Submenu::with_items(app, labels.language, true, &[&language_zh_cn, &language_zh_tw, &language_en])?;
    let theme_system = CheckMenuItem::with_id(app, "theme-system", labels.system, true, false, None::<&str>)?;
    let theme_dark = CheckMenuItem::with_id(app, "theme-dark", labels.dark, true, false, None::<&str>)?;
    let theme_light = CheckMenuItem::with_id(app, "theme-light", labels.light, true, false, None::<&str>)?;
    let skin_blur = CheckMenuItem::with_id(app, "skin-blur", labels.blur, true, false, None::<&str>)?;
    let skin_computer = CheckMenuItem::with_id(app, "skin-computer", labels.computer, true, false, None::<&str>)?;
    let appearance = Submenu::with_items(app, labels.appearance, true, &[&theme_system, &theme_dark, &theme_light, &skin_blur, &skin_computer])?;
    let autostart = CheckMenuItem::with_id(
        app, "autostart", labels.autostart, true,
        app.autolaunch().is_enabled().unwrap_or(false), None::<&str>,
    )?;
    #[cfg(debug_assertions)]
    let test_short_window = CheckMenuItem::with_id(app, "debug-short-window", labels.debug_short_window, true, false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
    let settings = Submenu::with_items(app, labels.settings, true, &[&always_on_top, &locked, &pin, &autostart])?;

    let preferences = app.try_state::<AppState>()
        .and_then(|state| state.preferences.lock().ok().map(|prefs| prefs.clone()))
        .unwrap_or_default();
    let _ = always_on_top.set_checked(preferences.always_on_top);
    let _ = locked.set_checked(preferences.locked);
    let _ = pin.set_checked(preferences.pinned_provider.is_some());
    let _ = language_zh_cn.set_checked(preferences.language == "zh-CN");
    let _ = language_zh_tw.set_checked(preferences.language == "zh-TW");
    let _ = language_en.set_checked(preferences.language == "en");
    let _ = theme_system.set_checked(preferences.selected_skin == "default" && preferences.appearance == "system");
    let _ = theme_dark.set_checked(preferences.selected_skin == "default" && preferences.appearance == "dark");
    let _ = theme_light.set_checked(preferences.selected_skin == "default" && preferences.appearance == "light");
    let _ = skin_blur.set_checked(preferences.selected_skin == "blur");
    let _ = skin_computer.set_checked(preferences.selected_skin == "computer");

    #[cfg(debug_assertions)]
    let menu = Menu::with_items(app, &[&show, &refresh, &settings, &language, &appearance, &test_short_window, &quit])?;
    #[cfg(not(debug_assertions))]
    let menu = Menu::with_items(app, &[&show, &refresh, &settings, &language, &appearance, &quit])?;
    let mut builder = TrayIconBuilder::with_id("main").menu(&menu).tooltip("Quota Float");
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let autostart_menu = autostart.clone();
    let always_on_top_state = always_on_top.clone();
    let locked_state = locked.clone();
    let theme_system_state = theme_system.clone();
    let theme_dark_state = theme_dark.clone();
    let theme_light_state = theme_light.clone();
    let skin_blur_state = skin_blur.clone();
    let skin_computer_state = skin_computer.clone();
    let pin_state = pin.clone();
    #[cfg(debug_assertions)]
    let test_short_window_menu = test_short_window.clone();
    let tray_menu_state = TrayMenuState {
        show: show.clone(),
        refresh: refresh.clone(),
        always_on_top: always_on_top.clone(),
        locked: locked.clone(),
        pinned: pin.clone(),
        autostart: autostart.clone(),
        settings: settings.clone(),
        language: language.clone(),
        language_zh_cn: language_zh_cn.clone(),
        language_zh_tw: language_zh_tw.clone(),
        language_en: language_en.clone(),
        appearance: appearance.clone(),
        theme_system: theme_system.clone(),
        theme_dark: theme_dark.clone(),
        theme_light: theme_light.clone(),
        skin_blur: skin_blur.clone(),
        skin_computer: skin_computer.clone(),
        quit: quit.clone(),
        #[cfg(debug_assertions)]
        test_short_window: test_short_window.clone(),
    };
    update_tray_menu(&tray_menu_state, &preferences.language, initially_visible);
    let tray_menu_for_events = tray_menu_state.clone();
    builder.on_menu_event(move |app, event| match event.id.as_ref() {
        "show" => if let Some(window) = app.get_webview_window("widget") {
            if window.is_visible().unwrap_or(false) {
                let _ = window.hide();
                sync_show_menu_state(app, false);
            } else {
                let _ = window.show();
                let _ = window.set_focus();
                sync_show_menu_state(app, true);
            }
        },
        "refresh" => { let _ = app.emit_to("widget", "refresh-requested", ()); }
        "debug-short-window" => {
            #[cfg(debug_assertions)]
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(mut enabled) = state.simulate_short_window_for_testing.lock() {
                    *enabled = !*enabled;
                    let _ = test_short_window_menu.set_checked(*enabled);
                    let _ = app.emit_to("widget", "refresh-requested", ());
                }
            }
        }
        "locked" => if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut prefs) = state.preferences.lock() {
                let previous = prefs.clone();
                prefs.locked = !prefs.locked;
                if persist_preferences(&state.preferences_path, &prefs).is_ok()
                    && apply_lock(app, prefs.locked).is_ok()
                {
                    let _ = locked_state.set_checked(prefs.locked);
                    let _ = app.emit_to("widget", "preferences-changed", prefs.clone());
                } else {
                    *prefs = previous;
                    let _ = persist_preferences(&state.preferences_path, &prefs);
                    let _ = locked_state.set_checked(prefs.locked);
                }
            }
        },
        "pin" => if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut prefs) = state.preferences.lock() {
                let previous = prefs.clone();
                prefs.pinned_provider = if prefs.pinned_provider.is_some() { None } else { Some("codex".into()) };
                if persist_preferences(&state.preferences_path, &prefs).is_ok() {
                    let _ = pin_state.set_checked(prefs.pinned_provider.is_some());
                    let _ = app.emit_to("widget", "preferences-changed", prefs.clone());
                } else {
                    *prefs = previous;
                    let _ = pin_state.set_checked(prefs.pinned_provider.is_some());
                }
            }
        },
        "language-zh-CN" | "language-zh-TW" | "language-en" => if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut prefs) = state.preferences.lock() {
                let previous = prefs.clone();
                prefs.language = event.id.as_ref().strip_prefix("language-").unwrap_or("zh-CN").into();
                if persist_preferences(&state.preferences_path, &prefs).is_ok() {
                    let visible = app.get_webview_window("widget")
                        .and_then(|window| window.is_visible().ok())
                        .unwrap_or(true);
                    update_tray_menu(&tray_menu_for_events, &prefs.language, visible);
                    let _ = app.emit_to("widget", "preferences-changed", prefs.clone());
                } else {
                    *prefs = previous;
                    let visible = app.get_webview_window("widget")
                        .and_then(|window| window.is_visible().ok())
                        .unwrap_or(true);
                    update_tray_menu(&tray_menu_for_events, &prefs.language, visible);
                }
            }
        },
        "always-on-top" => if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut prefs) = state.preferences.lock() {
                let previous = prefs.clone();
                prefs.always_on_top = !prefs.always_on_top;
                if persist_preferences(&state.preferences_path, &prefs).is_ok()
                    && apply_always_on_top(app, prefs.always_on_top).is_ok()
                {
                    let _ = always_on_top_state.set_checked(prefs.always_on_top);
                    let _ = app.emit_to("widget", "preferences-changed", prefs.clone());
                } else {
                    *prefs = previous;
                    let _ = persist_preferences(&state.preferences_path, &prefs);
                    let _ = always_on_top_state.set_checked(prefs.always_on_top);
                }
            }
        },
        "theme-system" | "theme-dark" | "theme-light" => if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut prefs) = state.preferences.lock() {
                prefs.appearance = match event.id.as_ref() {
                    "theme-dark" => "dark".into(), "theme-light" => "light".into(), _ => "system".into(),
                };
                prefs.selected_skin = "default".into();
                if persist_preferences(&state.preferences_path, &prefs).is_ok() {
                    let _ = theme_system_state.set_checked(prefs.appearance == "system");
                    let _ = theme_dark_state.set_checked(prefs.appearance == "dark");
                    let _ = theme_light_state.set_checked(prefs.appearance == "light");
                    let _ = skin_blur_state.set_checked(false);
                    let _ = skin_computer_state.set_checked(false);
                    let _ = app.emit_to("widget", "preferences-changed", prefs.clone());
                }
            }
        },
        "skin-blur" | "skin-computer" => if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut prefs) = state.preferences.lock() {
                prefs.selected_skin = event.id.as_ref().strip_prefix("skin-").unwrap_or("blur").into();
                prefs.appearance = "light".into();
                if persist_preferences(&state.preferences_path, &prefs).is_ok() {
                    let _ = theme_system_state.set_checked(false);
                    let _ = theme_dark_state.set_checked(false);
                    let _ = theme_light_state.set_checked(false);
                    let _ = skin_blur_state.set_checked(prefs.selected_skin == "blur");
                    let _ = skin_computer_state.set_checked(prefs.selected_skin == "computer");
                    let _ = app.emit_to("widget", "preferences-changed", prefs.clone());
                }
            }
        },
        "autostart" => {
            let manager = app.autolaunch();
            let enabled = manager.is_enabled().unwrap_or(false);
            let result = if enabled { manager.disable() } else { manager.enable() };
            if result.is_ok() { let _ = autostart_menu.set_checked(!enabled); }
        }
        "quit" => app.exit(0),
        _ => {}
    }).build(app)?;
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut tray_menu) = state.tray_menu.lock() {
            *tray_menu = Some(tray_menu_state);
        }
    }
    Ok(())
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("widget") {
                let _ = window.show();
                let _ = window.set_focus();
                sync_show_menu_state(app, true);
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(WindowStateBuilder::default().build())
        .setup(|app| {
            let data_dir = app.path().app_config_dir()?;
            let preferences_path = data_dir.join("preferences.json");
            let preferences = load_preferences(&preferences_path);
            let _ = persist_preferences(&preferences_path, &preferences);
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(12))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("QuotaFloat/0.1")
                .build()
                .expect("static HTTP client configuration must be valid");
            app.manage(AppState {
                client,
                preferences: Mutex::new(preferences.clone()),
                preferences_path,
                fetch_lock: tokio::sync::Mutex::new(()),
                snapshot_cache: Mutex::new(None),
                #[cfg(debug_assertions)]
                simulate_short_window_for_testing: Mutex::new(false),
                geometry: Mutex::new(None),
                drag_mode: Mutex::new(None),
                tray_menu: Mutex::new(None),
            });
            if setup_tray(app).is_err() {
                eprintln!("tray setup failed; enabling taskbar fallback");
                if let Some(window) = app.get_webview_window("widget") {
                    let _ = window.set_skip_taskbar(false);
                }
            }
            if preferences.locked {
                let _ = apply_lock(app.handle(), true);
            }
            if let Some(window) = app.get_webview_window("widget") {
                let _ = apply_always_on_top(app.handle(), preferences.always_on_top);
                #[cfg(target_os = "macos")]
                let _ = window.set_visible_on_all_workspaces(true);
                // A saved window position can be outside the active monitor while
                // iterating in development. Keep the test widget discoverable.
                #[cfg(debug_assertions)]
                {
                    let _ = window.center();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            #[cfg(target_os = "macos")]
            register_space_change_fronting(app);
            #[cfg(debug_assertions)]
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(800));
                    if let Some(window) = handle.get_webview_window("widget") {
                        let _ = window.set_position(PhysicalPosition::new(120, 120));
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshots,
            refresh_snapshots,
            expand_widget,
            collapse_widget,
            begin_widget_drag,
            finish_widget_drag,
            get_preferences,
            set_preferences,
            set_widget_locked,
            set_widget_always_on_top,
            sync_widget_appearance
        ])
        .on_tray_icon_event(|app, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Some(window) = app.get_webview_window("widget") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    sync_show_menu_state(app, true);
                }
            }
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                sync_show_menu_state(window.app_handle(), false);
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build Quota Float");
    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::Resumed) {
            let _ = app_handle.emit_to("widget", "refresh-requested", ());
        }
    });
}
