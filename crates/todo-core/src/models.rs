use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoTask {
    pub id: String,
    pub customer_name: String,
    pub title: String,
    pub note: String,
    pub status: TodoStatus,
    pub received_at: String,
    #[serde(default)]
    pub planned_start_at: Option<String>,
    pub due_at: Option<String>,
    pub urgent_at: Option<String>,
    pub completed_at: Option<String>,
    #[serde(default)]
    pub cancelled_at: Option<String>,
    pub attachment_paths: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoDraft {
    pub customer_name: String,
    pub title: String,
    pub note: String,
    pub received_at: String,
    pub planned_start_at: Option<String>,
    pub due_at: Option<String>,
    pub attachment_paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DailySourceStatus {
    Pending,
    Processed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailySource {
    pub id: String,
    pub work_date: String,
    pub text: String,
    pub attachment_paths: Vec<String>,
    pub status: DailySourceStatus,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailySourceDraft {
    pub work_date: String,
    pub text: String,
    pub attachment_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyReport {
    pub work_date: String,
    pub content: String,
    pub generated_at: String,
    pub updated_at: String,
}
