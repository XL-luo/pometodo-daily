use pometodo_core::{
    local::LocalStorage,
    models::{TodoDraft, TodoStatus, TodoTask},
    store::TodoRepository,
};
use serde::Serialize;
use std::{path::PathBuf, sync::Mutex};
use tauri::{Emitter, Manager};

pub struct StorageState {
    pub inner: Mutex<Result<LocalStorage, String>>,
}

impl StorageState {
    pub fn open(data_dir: PathBuf) -> Self {
        Self {
            inner: Mutex::new(LocalStorage::open(&data_dir)),
        }
    }
}

pub(crate) async fn with_repo<T: Send + 'static>(
    app: tauri::AppHandle,
    operation: impl FnOnce(&mut TodoRepository) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    with_storage(app, move |storage| operation(&mut storage.repo)).await
}

pub(crate) async fn with_storage<T: Send + 'static>(
    app: tauri::AppHandle,
    operation: impl FnOnce(&mut LocalStorage) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<StorageState>();
        let mut guard = state
            .inner
            .lock()
            .map_err(|_| "数据操作中断，请重启软件".to_string())?;
        operation(guard.as_mut().map_err(|error| error.clone())?)
    })
    .await
    .map_err(|_| "本地数据操作未完成".to_string())?
}

pub(crate) fn validate_attachment_ids(state: &LocalStorage, ids: &[String]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let store = state.attachment_store()?;
    for id in ids {
        store.path_for(id)?;
    }
    Ok(())
}

pub(crate) async fn with_mutation<T: Send + 'static>(
    app: tauri::AppHandle,
    operation: impl FnOnce(&mut LocalStorage) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let result = with_storage(app.clone(), operation).await?;
    let _ = app.emit("pometodo-data-changed", ());
    if let Err(error) = crate::floating::refresh(&app) {
        let _ = app.emit("pometodo-system-error", error);
    }
    Ok(result)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    service_mode: &'static str,
    data_directory: String,
    theme: String,
    version: String,
    quick_due_options: Vec<pometodo_core::settings::QuickDueSetting>,
    customers: Vec<pometodo_core::settings::CustomerSetting>,
    close_action: String,
    floating_ball_enabled: bool,
}

#[tauri::command]
pub async fn pometodo_app_info(app: tauri::AppHandle) -> Result<AppInfo, String> {
    let version = app.package_info().version.to_string();
    with_storage(app, move |storage| {
        let settings = storage.repo.settings()?;
        Ok(AppInfo {
            service_mode: crate::distribution::SERVICE_MODE,
            data_directory: storage.locations.data_directory.display().to_string(),
            theme: settings.theme,
            quick_due_options: settings.quick_due_options,
            close_action: settings.close_action,
            floating_ball_enabled: settings.floating_ball_enabled,
            customers: storage.repo.customers()?,
            version,
        })
    })
    .await
}

#[tauri::command]
pub async fn pometodo_set_theme(app: tauri::AppHandle, theme: String) -> Result<(), String> {
    with_mutation(app, move |storage| storage.repo.set_theme(&theme)).await
}

#[tauri::command]
pub async fn pometodo_list_tasks(app: tauri::AppHandle) -> Result<Vec<TodoTask>, String> {
    with_repo(app, |repo| repo.list()).await
}

#[tauri::command]
pub async fn pometodo_create_task(
    app: tauri::AppHandle,
    draft: TodoDraft,
) -> Result<TodoTask, String> {
    with_mutation(app, move |storage| {
        validate_attachment_ids(storage, &draft.attachment_paths)?;
        storage.repo.create(draft)
    })
    .await
}

#[tauri::command]
pub async fn pometodo_update_task(
    app: tauri::AppHandle,
    id: String,
    draft: TodoDraft,
) -> Result<TodoTask, String> {
    with_mutation(app, move |storage| {
        validate_attachment_ids(storage, &draft.attachment_paths)?;
        storage.repo.update(&id, draft)
    })
    .await
}

#[tauri::command]
pub async fn pometodo_set_status(
    app: tauri::AppHandle,
    id: String,
    status: TodoStatus,
) -> Result<TodoTask, String> {
    with_mutation(app, move |storage| storage.repo.set_status(&id, status)).await
}

#[tauri::command]
pub async fn pometodo_cancel_task(
    app: tauri::AppHandle,
    id: String,
) -> Result<TodoTask, String> {
    with_mutation(app, move |storage| storage.repo.cancel_task(&id)).await
}

#[tauri::command]
pub async fn pometodo_set_urgent(
    app: tauri::AppHandle,
    id: String,
    urgent: bool,
) -> Result<TodoTask, String> {
    with_mutation(app, move |storage| storage.repo.set_urgent(&id, urgent)).await
}

#[tauri::command]
pub async fn pometodo_delete_task(app: tauri::AppHandle, id: String) -> Result<(), String> {
    with_mutation(app, move |storage| storage.repo.delete(&id)).await
}

#[tauri::command]
pub async fn pometodo_attachment_data_url(
    app: tauri::AppHandle,
    path: String,
) -> Result<String, String> {
    with_storage(app, move |storage| {
        use base64::Engine;
        let bytes = storage.attachment_store()?.read_png(&path)?;
        Ok(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    })
    .await
}

/// Only managed PNG files may be handed to the system file association.
fn attachment_for_viewer(storage: &LocalStorage, id: &str) -> Result<PathBuf, String> {
    let store = storage.attachment_store()?;
    let path = store.path_for(id)?;
    if path.extension().and_then(|value| value.to_str()) != Some("png") {
        return Err("只能打开 PomeTodo 保存的 PNG 截图".into());
    }
    store.read_png(id)?;
    Ok(path)
}

#[tauri::command]
pub async fn pometodo_open_attachment(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let image = with_storage(app, move |storage| attachment_for_viewer(storage, &path)).await?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::{
            core::{w, PCWSTR},
            Win32::{
                Foundation::HWND,
                UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
            },
        };
        let wide: Vec<u16> = image.as_os_str().encode_wide().chain(Some(0)).collect();
        // No command shell or arbitrary executable arguments; Windows opens the verified PNG.
        let result = unsafe {
            ShellExecuteW(
                HWND::default(),
                w!("open"),
                PCWSTR(wide.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            return Err("无法打开系统看图软件，请检查 Windows 默认图片应用".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = image;
        Err("当前平台不支持系统看图".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_viewer_accepts_only_existing_managed_pngs() {
        let directory = tempfile::tempdir().unwrap();
        let storage = LocalStorage::open(directory.path()).unwrap();
        let store = storage.attachment_store().unwrap();
        let id = store.save_png(b"\x89PNG\r\n\x1a\nimage-test").unwrap();
        assert!(attachment_for_viewer(&storage, &id).is_ok());
        for invalid in [
            "../private.png",
            "C:\\private.png",
            "cmd.exe",
            "missing.png",
        ] {
            assert!(attachment_for_viewer(&storage, invalid).is_err());
        }
        std::fs::write(store.path_for(&id).unwrap(), b"not an image").unwrap();
        assert!(attachment_for_viewer(&storage, &id).is_err());
    }

    #[test]
    fn text_only_tasks_do_not_require_a_working_attachment_directory() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("attachments"), b"blocked directory").unwrap();
        let state = StorageState::open(directory.path().to_path_buf());
        let mut guard = state.inner.lock().unwrap();
        let storage = guard.as_mut().unwrap();
        assert!(storage.attachments.is_err());
        let mut draft = TodoDraft {
            customer_name: "测试客户".into(),
            title: "纯文字保存".into(),
            note: String::new(),
            received_at: "2026-09-06T00:00:00Z".into(),
            planned_start_at: None,
            due_at: None,
            attachment_paths: vec![],
        };
        assert!(validate_attachment_ids(storage, &draft.attachment_paths).is_ok());
        let saved = storage.repo.create(draft.clone()).unwrap();
        assert_eq!(storage.repo.list().unwrap()[0].id, saved.id);
        draft
            .attachment_paths
            .push("not-an-existing-image.png".into());
        assert!(validate_attachment_ids(storage, &draft.attachment_paths).is_err());
    }
}
