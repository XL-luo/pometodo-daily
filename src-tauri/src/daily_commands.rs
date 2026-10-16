use crate::{
    ai_http::AiHttp,
    commands::{validate_attachment_ids, with_mutation, with_storage},
};
use pometodo_core::models::{DailyReport, DailySource, DailySourceDraft};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyGenerationResult {
    kind: &'static str,
    content: String,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailySnapshot {
    sources: Vec<DailySource>,
    report: Option<DailyReport>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyRecognitionResult {
    kind: &'static str,
    items: Vec<String>,
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DailySourceUpdate {
    text: String,
}

#[tauri::command]
pub async fn pometodo_daily_snapshot(
    app: tauri::AppHandle,
    work_date: String,
) -> Result<DailySnapshot, String> {
    with_storage(app, move |storage| {
        Ok(DailySnapshot {
            sources: storage.repo.list_daily_sources(&work_date)?,
            report: storage.repo.report(&work_date)?,
        })
    })
    .await
}

#[tauri::command]
pub async fn pometodo_daily_create_source(
    app: tauri::AppHandle,
    draft: DailySourceDraft,
) -> Result<DailySource, String> {
    with_mutation(app, move |storage| {
        validate_attachment_ids(storage, &draft.attachment_paths)?;
        storage.repo.create_daily_source(draft)
    })
    .await
}

#[tauri::command]
pub async fn pometodo_daily_create_sources(
    app: tauri::AppHandle,
    drafts: Vec<DailySourceDraft>,
) -> Result<Vec<DailySource>, String> {
    with_mutation(app, move |storage| {
        for draft in &drafts {
            validate_attachment_ids(storage, &draft.attachment_paths)?;
        }
        storage.repo.create_daily_sources(drafts)
    })
    .await
}

#[tauri::command]
pub async fn pometodo_daily_update_source(
    app: tauri::AppHandle,
    id: String,
    draft: DailySourceUpdate,
) -> Result<DailySource, String> {
    with_mutation(app, move |storage| {
        storage.repo.update_daily_source(&id, &draft.text)
    })
    .await
}

/// 优先由视觉模型读图；未配置 AI 或视觉失败时，回退到 Windows OCR 原文，
/// 由界面明确告知用户仍需手工拆分和核对。
#[tauri::command]
pub async fn pometodo_daily_recognize_attachment(
    app: tauri::AppHandle,
    id: String,
) -> Result<DailyRecognitionResult, String> {
    let (path, credentials) = with_storage(app, move |storage| {
        let path = storage.attachment_store()?.path_for(&id)?;
        let credentials = crate::smart_arrange::selected_user_key(storage).ok();
        Ok((path, credentials))
    })
    .await?;
    tauri::async_runtime::spawn_blocking(move || {
        let vision = crate::clipboard::prepare_vision_image(&path);
        if let (Ok(images), Some((provider, key))) = (vision, credentials) {
            let result = crate::ai_http::AiHttp::new()
                .map_err(|error| error.to_string())
                .and_then(|client| {
                    client
                        .complete_vision(
                            &provider,
                            &key,
                            &crate::daily_extract::messages("请直接阅读截图中的工作内容。"),
                            &images,
                        )
                        .map_err(|error| error.to_string())
                })
                .and_then(|content| crate::daily_extract::parse(&content));
            if let Ok(parsed) = result {
                return Ok(DailyRecognitionResult {
                    kind: "ai",
                    items: parsed.items,
                    message: "已从截图识别出可核对的工作事项".into(),
                });
            }
        }
        let document = crate::ocr::recognize_document(&path)
            .map_err(|_| "截图暂时无法识别，请重试或手工补录".to_string())?;
        let text = document.text.trim();
        if text.is_empty() {
            return Err("截图中没有读到可用文字，请手工补录".into());
        }
        Ok(DailyRecognitionResult {
            kind: "ocr",
            items: vec![text.to_string()],
            message: "AI 未完成智能拆分，已保留 OCR 原文，请核对后手工拆分".into(),
        })
    })
    .await
    .map_err(|_| "截图识别未完成".to_string())?
}

#[tauri::command]
pub async fn pometodo_daily_delete_source(app: tauri::AppHandle, id: String) -> Result<(), String> {
    with_mutation(app, move |storage| storage.repo.delete_daily_source(&id)).await
}

#[tauri::command]
pub async fn pometodo_daily_save_report(
    app: tauri::AppHandle,
    work_date: String,
    content: String,
) -> Result<DailyReport, String> {
    with_mutation(app, move |storage| {
        storage.repo.save_report(&work_date, &content)
    })
    .await
}

#[tauri::command]
pub async fn pometodo_daily_generate(
    app: tauri::AppHandle,
    work_date: String,
    selected_task_ids: Vec<String>,
    supplement: String,
) -> Result<DailyGenerationResult, String> {
    let (tasks, credentials) = with_storage(app, move |storage| {
        let selected: std::collections::HashSet<_> = selected_task_ids.into_iter().collect();
        let tasks = storage
            .repo
            .list()?
            .into_iter()
            .filter(|task| selected.contains(&task.id))
            .collect::<Vec<_>>();
        let credentials = crate::smart_arrange::selected_user_key(storage).ok();
        Ok((tasks, credentials))
    })
    .await?;
    let fallback = crate::daily_report::local_report(&tasks, &work_date, &supplement)?;
    let Some((provider, key)) = credentials else {
        return Ok(DailyGenerationResult {
            kind: "local",
            content: fallback,
            message: "AI 未启用，已使用本地模板生成".into(),
        });
    };
    let messages = crate::daily_report::messages(&tasks, &work_date, &supplement)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        AiHttp::new()
            .map_err(|error| error.to_string())?
            .complete(&provider, &key, &messages)
            .map_err(|error| error.to_string())
            .and_then(|value| crate::daily_report::parse(&value))
    })
    .await
    .map_err(|_| "日报生成未完成".to_string())?;
    match result {
        Ok(content) => Ok(DailyGenerationResult {
            kind: "ai",
            content,
            message: "AI 已根据待办和截止日期生成草稿".into(),
        }),
        Err(_) => Ok(DailyGenerationResult {
            kind: "local",
            content: fallback,
            message: "AI 暂时不可用，已自动使用本地模板".into(),
        }),
    }
}
