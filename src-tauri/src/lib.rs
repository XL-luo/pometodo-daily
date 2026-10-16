mod ai_extract;
mod ai_http;
mod clipboard;
mod commands;
mod context_menu;
mod daily_commands;
mod daily_extract;
mod daily_report;
mod distribution;
mod file_commands;
mod floating;
mod local_extract;
mod ocr;
#[cfg(feature = "official-services")]
mod official;
mod reminders;
mod secrets;
mod service_extension;
mod settings_commands;
mod smart_arrange;
mod updates;
mod window_geometry;

use commands::StorageState;
use std::sync::Arc;
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn app_icon() -> tauri::image::Image<'static> {
    let image = image::load_from_memory(include_bytes!("../icons/icon.png"))
        .expect("embedded legacy logo must be a valid PNG")
        .into_rgba8();
    let (width, height) = image.dimensions();
    tauri::image::Image::new_owned(image.into_raw(), width, height)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name(if cfg!(debug_assertions) {
                    "PomeTodo Daily Development"
                } else {
                    "PomeTodo Daily"
                })
                .arg("--autostart")
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_window(app)
        }))
        .setup(|app| {
            let base = app.path().app_local_data_dir()?;
            let data_dir = if cfg!(debug_assertions) {
                base.join("development")
            } else {
                base.join("data")
            };
            app.manage(StorageState::open(data_dir.clone()));
            app.manage(file_commands::PendingFiles::default());
            app.manage(floating::FloatingState::default());
            let window_geometry = Arc::new(window_geometry::WindowGeometryState::default());
            window_geometry::configure(&window_geometry, &data_dir);
            app.manage(window_geometry);
            app.manage(context_menu::ContextMenuState::default());
            app.manage(updates::UpdateState::default());
            floating::create(app.handle())?;
            context_menu::create(app.handle())?;
            let start_minimized = {
                let state = app.state::<StorageState>();
                let guard = state.inner.lock().map_err(|_| "无法读取启动设置")?;
                guard
                    .as_ref()
                    .ok()
                    .and_then(|storage| storage.repo.settings().ok())
                    .is_some_and(|settings| settings.start_minimized)
            };
            TrayIconBuilder::new()
                .icon(app_icon())
                .tooltip("PomeTodo 待办与日报")
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Right,
                        button_state: MouseButtonState::Up,
                        position,
                        ..
                    } = event
                    {
                        if let Err(error) = context_menu::open(tray.app_handle(), false, position) {
                            let _ = tray.app_handle().emit("pometodo-system-error", error);
                        }
                        return;
                    }
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        show_window(tray.app_handle());
                    }
                })
                .build(app)?;
            if let Some(window) = app.get_webview_window("main") {
                window_geometry::restore(app.handle(), &data_dir);
                let _ = window.set_icon(app_icon());
                if updates::should_show_on_start(start_minimized, std::env::args_os().skip(1)) {
                    window.show()?;
                    let _ = window.set_focus();
                }
            }
            reminders::start(app.handle().clone());
            let _ = floating::refresh(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main"
                && matches!(
                    event,
                    tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Focused(_)
                )
            {
                let app = window.app_handle().clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let _ = floating::refresh(&app);
                });
            }
            if window.label() == "main"
                && matches!(
                    event,
                    tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)
                )
            {
                let state = window
                    .app_handle()
                    .state::<Arc<window_geometry::WindowGeometryState>>();
                window_geometry::remember(&window, state.inner());
            }
            if window.label() == "main"
                && matches!(event, tauri::WindowEvent::CloseRequested { .. })
            {
                let state = window
                    .app_handle()
                    .state::<Arc<window_geometry::WindowGeometryState>>();
                window_geometry::flush(&window, state.inner());
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let should_exit = {
                    let state = window.app_handle().state::<StorageState>();
                    state
                        .inner
                        .lock()
                        .ok()
                        .and_then(|guard| {
                            guard
                                .as_ref()
                                .ok()
                                .and_then(|storage| storage.repo.settings().ok())
                        })
                        .is_some_and(|settings| settings.close_action == "exit")
                };
                if window.label() == "main" && should_exit {
                    window.app_handle().exit(0);
                    return;
                }
                api.prevent_close();
                let _ = window.hide();
                if window.label() == "main" {
                    let _ = floating::refresh(window.app_handle());
                }
            }
        })
        .invoke_handler({
            let shared: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
                updates::pometodo_check_update,
                updates::pometodo_download_update,
                updates::pometodo_install_update,
                commands::pometodo_app_info,
                settings_commands::pometodo_settings_load,
                settings_commands::pometodo_settings_update,
                settings_commands::pometodo_customer_rename,
                settings_commands::pometodo_customer_hide,
                settings_commands::pometodo_key_save,
                settings_commands::pometodo_key_clear,
                file_commands::pometodo_choose_directory,
                file_commands::pometodo_apply_directory,
                file_commands::pometodo_export_backup,
                file_commands::pometodo_choose_backup_import,
                file_commands::pometodo_import_backup,
                floating::pometodo_float_expand,
                floating::pometodo_float_drag_start,
                floating::pometodo_float_dock_state,
                floating::pometodo_float_dock_reveal,
                floating::pometodo_float_present,
                floating::pometodo_float_preview_state,
                floating::pometodo_float_menu,
                floating::pometodo_open_main,
                context_menu::pometodo_menu_snapshot,
                context_menu::pometodo_menu_present,
                context_menu::pometodo_menu_action,
                commands::pometodo_set_theme,
                commands::pometodo_list_tasks,
                commands::pometodo_create_task,
                commands::pometodo_update_task,
                commands::pometodo_set_status,
                commands::pometodo_cancel_task,
                commands::pometodo_set_urgent,
                commands::pometodo_delete_task,
                daily_commands::pometodo_daily_snapshot,
                daily_commands::pometodo_daily_create_source,
                daily_commands::pometodo_daily_create_sources,
                daily_commands::pometodo_daily_update_source,
                daily_commands::pometodo_daily_recognize_attachment,
                daily_commands::pometodo_daily_delete_source,
                daily_commands::pometodo_daily_save_report,
                daily_commands::pometodo_daily_generate,
                commands::pometodo_attachment_data_url,
                commands::pometodo_open_attachment,
                clipboard::pometodo_recognize_clipboard,
                clipboard::pometodo_recognize_attachment,
                smart_arrange::pometodo_smart_arrange_state,
                smart_arrange::pometodo_set_smart_arrange,
                clipboard::pometodo_save_clipboard_attachment,
                clipboard::pometodo_import_attachment,
            ];
            move |request: tauri::ipc::Invoke<tauri::Wry>| {
                #[cfg(feature = "official-services")]
                if request.message.command().starts_with("pometodo_official_") {
                    return official::handle(request);
                }
                shared(request)
            }
        })
        .run(tauri::generate_context!())
        .expect("PomeTodo 无法启动");
}
