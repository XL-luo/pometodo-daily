use crate::models::{
    DailyReport, DailySource, DailySourceDraft, DailySourceStatus, TodoDraft, TodoStatus, TodoTask,
};
use chrono::{DateTime, Local, NaiveDate, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;
use std::time::Duration;

pub struct TodoRepository {
    conn: Connection,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupRecord {
    pub task: TodoTask,
    pub created_at: String,
    pub deleted_at: Option<String>,
    pub version: i64,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DailyBackup {
    pub sources: Vec<DailySource>,
    pub reports: Vec<DailyReport>,
}

pub fn validate_backup_records(records: &[BackupRecord]) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    if records.len() > 100_000 {
        return Err("备份任务数量超过限制".into());
    }
    for record in records {
        let task = &record.task;
        if uuid::Uuid::parse_str(&task.id).is_err() || !ids.insert(&task.id) || record.version < 1 {
            return Err("备份任务标识重复或无效".into());
        }
        validate(TodoDraft {
            customer_name: task.customer_name.clone(),
            title: task.title.clone(),
            note: task.note.clone(),
            received_at: task.received_at.clone(),
            planned_start_at: task.planned_start_at.clone(),
            due_at: task.due_at.clone(),
            attachment_paths: task.attachment_paths.clone(),
        })?;
        for date in [
            Some(&record.created_at),
            Some(&task.updated_at),
            record.deleted_at.as_ref(),
            task.completed_at.as_ref(),
            task.cancelled_at.as_ref(),
            task.urgent_at.as_ref(),
            task.planned_start_at.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            DateTime::parse_from_rfc3339(date).map_err(|_| "备份中包含无效日期")?;
        }
    }
    Ok(())
}

const SELECT_TASK: &str = "SELECT id, customer_name, title, note, status, received_at,
    planned_start_at, due_at, urgent_at, completed_at, cancelled_at, attachment_paths, updated_at FROM todo_tasks";

fn storage_error(error: rusqlite::Error) -> String {
    format!("本地数据操作失败，请检查磁盘空间和目录权限：{error}")
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn status_name(status: TodoStatus) -> &'static str {
    match status {
        TodoStatus::Pending => "pending",
        TodoStatus::InProgress => "in_progress",
        TodoStatus::Completed => "completed",
    }
}

fn source_status_name(status: DailySourceStatus) -> &'static str {
    match status {
        DailySourceStatus::Pending => "pending",
        DailySourceStatus::Processed => "processed",
        DailySourceStatus::Failed => "failed",
    }
}

fn validate_work_date(value: &str) -> Result<(), String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| "工作日期无效，请重新选择".into())
}

fn parse_string_list(value: String, column: usize) -> rusqlite::Result<Vec<String>> {
    serde_json::from_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn read_source(row: &Row<'_>) -> rusqlite::Result<DailySource> {
    let status: String = row.get(4)?;
    let status = match status.as_str() {
        "pending" => DailySourceStatus::Pending,
        "processed" => DailySourceStatus::Processed,
        "failed" => DailySourceStatus::Failed,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(DailySource {
        id: row.get(0)?,
        work_date: row.get(1)?,
        text: row.get(2)?,
        attachment_paths: parse_string_list(row.get(3)?, 3)?,
        status,
        error: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn read_task(row: &Row<'_>) -> rusqlite::Result<TodoTask> {
    let status: String = row.get(4)?;
    let status = match status.as_str() {
        "pending" => TodoStatus::Pending,
        "in_progress" => TodoStatus::InProgress,
        "completed" => TodoStatus::Completed,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let attachments: String = row.get(11)?;
    let attachment_paths = serde_json::from_str(&attachments).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(TodoTask {
        id: row.get(0)?,
        customer_name: row.get(1)?,
        title: row.get(2)?,
        note: row.get(3)?,
        status,
        received_at: row.get(5)?,
        planned_start_at: row.get(6)?,
        due_at: row.get(7)?,
        urgent_at: row.get(8)?,
        completed_at: row.get(9)?,
        cancelled_at: row.get(10)?,
        attachment_paths,
        updated_at: row.get(12)?,
    })
}

fn get_task(conn: &Connection, id: &str) -> Result<TodoTask, String> {
    conn.query_row(
        &format!("{SELECT_TASK} WHERE id = ?1 AND deleted_at IS NULL"),
        [id],
        read_task,
    )
    .optional()
    .map_err(storage_error)?
    .ok_or_else(|| "这条待办不存在或已删除".into())
}

fn validate(mut draft: TodoDraft) -> Result<TodoDraft, String> {
    draft.customer_name = draft.customer_name.trim().to_string();
    draft.title = draft.title.trim().to_string();
    draft.note = draft.note.trim().to_string();
    if draft.title.is_empty() {
        return Err("请填写任务内容".into());
    }
    DateTime::parse_from_rfc3339(&draft.received_at)
        .map_err(|_| "接收日期无效，请重新选择".to_string())?;
    if let Some(date) = &draft.due_at {
        DateTime::parse_from_rfc3339(date)
            .map_err(|_| "完成日期无效，请重新选择或清空".to_string())?;
    }
    if let Some(date) = &draft.planned_start_at {
        DateTime::parse_from_rfc3339(date)
            .map_err(|_| "计划开始日期无效，请重新选择或清空".to_string())?;
    }
    if let (Some(start), Some(due)) = (&draft.planned_start_at, &draft.due_at) {
        if DateTime::parse_from_rfc3339(start).map_err(|_| "计划开始日期无效，请重新选择或清空")?.with_timezone(&Local).date_naive()
            > DateTime::parse_from_rfc3339(due).map_err(|_| "完成日期无效，请重新选择或清空")?.with_timezone(&Local).date_naive() {
            return Err("计划开始日期不能晚于完成日期".into());
        }
    }
    Ok(draft)
}

impl TodoRepository {
    pub fn daily_backup(&self) -> Result<DailyBackup, String> {
        let mut sources_stmt = self.conn.prepare(
            "SELECT id,work_date,text,attachment_paths,status,error,created_at,updated_at FROM daily_sources ORDER BY created_at,id",
        ).map_err(storage_error)?;
        let sources = sources_stmt
            .query_map([], read_source)
            .map_err(storage_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage_error)?;
        let mut reports_stmt = self.conn.prepare(
            "SELECT work_date,content,generated_at,updated_at FROM daily_reports ORDER BY work_date",
        ).map_err(storage_error)?;
        let reports = reports_stmt
            .query_map([], |row| {
                Ok(DailyReport {
                    work_date: row.get(0)?,
                    content: row.get(1)?,
                    generated_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })
            .map_err(storage_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage_error)?;
        Ok(DailyBackup { sources, reports })
    }

    pub fn replace_daily_backup(
        &mut self,
        daily: &DailyBackup,
        images: &std::collections::BTreeMap<String, String>,
    ) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(storage_error)?;
        tx.execute("DELETE FROM daily_sources", [])
            .map_err(storage_error)?;
        tx.execute("DELETE FROM daily_reports", [])
            .map_err(storage_error)?;
        for source in &daily.sources {
            validate_work_date(&source.work_date)?;
            let attachments = source
                .attachment_paths
                .iter()
                .map(|id| {
                    images
                        .get(id)
                        .cloned()
                        .ok_or_else(|| "备份缺少日报截图".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            tx.execute(
                "INSERT INTO daily_sources(id,work_date,text,attachment_paths,status,error,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![source.id,source.work_date,source.text,serde_json::to_string(&attachments).map_err(|e| e.to_string())?,source_status_name(source.status),source.error,source.created_at,source.updated_at],
            ).map_err(storage_error)?;
        }
        for report in &daily.reports {
            validate_work_date(&report.work_date)?;
            tx.execute(
                "INSERT INTO daily_reports(work_date,content,generated_at,updated_at) VALUES (?1,?2,?3,?4)",
                params![report.work_date,report.content,report.generated_at,report.updated_at],
            ).map_err(storage_error)?;
        }
        tx.commit().map_err(storage_error)
    }

    pub fn backup_records(&self) -> Result<Vec<BackupRecord>, String> {
        let columns = SELECT_TASK
            .strip_suffix(" FROM todo_tasks")
            .expect("task query has a fixed table");
        let mut statement = self
            .conn
            .prepare(&format!(
                "{columns}, created_at, deleted_at, version FROM todo_tasks"
            ))
            .map_err(storage_error)?;
        let result = statement
            .query_map([], |row| {
                Ok(BackupRecord {
                    task: read_task(row)?,
                    created_at: row.get(13)?,
                    deleted_at: row.get(14)?,
                    version: row.get(15)?,
                })
            })
            .map_err(storage_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage_error)?;
        validate_backup_records(&result)?;
        Ok(result)
    }

    pub fn replace_backup_records(
        &mut self,
        records: &[BackupRecord],
        images: &std::collections::BTreeMap<String, String>,
    ) -> Result<(), String> {
        validate_backup_records(records)?;
        let tx = self.conn.transaction().map_err(storage_error)?;
        tx.execute("DELETE FROM todo_tasks", [])
            .map_err(storage_error)?;
        for record in records {
            let t = &record.task;
            let attachments = t
                .attachment_paths
                .iter()
                .map(|id| {
                    images
                        .get(id)
                        .cloned()
                        .ok_or_else(|| "备份缺少任务截图".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let attachments = serde_json::to_string(&attachments).map_err(|e| e.to_string())?;
            tx.execute("INSERT INTO todo_tasks(id,customer_name,title,note,status,received_at,planned_start_at,due_at,urgent_at,completed_at,cancelled_at,attachment_paths,created_at,updated_at,deleted_at,version) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)", params![t.id,t.customer_name,t.title,t.note,status_name(t.status),t.received_at,t.planned_start_at,t.due_at,t.urgent_at,t.completed_at,t.cancelled_at,attachments,record.created_at,t.updated_at,record.deleted_at,record.version]).map_err(storage_error)?;
        }
        tx.commit().map_err(storage_error)
    }

    /// 调用方持有存储锁；SQLite 在线备份包含 WAL 中已提交的事务。
    pub fn snapshot_to(&self, path: &Path) -> Result<(), String> {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| format!("无法创建备份文件（不会覆盖已有文件）：{error}"))?;
        self.conn
            .backup(rusqlite::DatabaseName::Main, path, None)
            .map_err(storage_error)
    }

    pub fn attachment_ids(&self) -> Result<std::collections::BTreeSet<String>, String> {
        let mut statement = self
            .conn
            .prepare("SELECT attachment_paths FROM todo_tasks UNION ALL SELECT attachment_paths FROM daily_sources")
            .map_err(storage_error)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(storage_error)?;
        let mut ids = std::collections::BTreeSet::new();
        for row in rows {
            let values: Vec<String> = serde_json::from_str(&row.map_err(storage_error)?)
                .map_err(|_| "截图记录损坏，操作已停止")?;
            ids.extend(values);
        }
        Ok(ids)
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        Self::from_connection(Connection::open(path).map_err(storage_error)?)
    }

    pub fn open_in_memory() -> Result<Self, String> {
        Self::from_connection(Connection::open_in_memory().map_err(storage_error)?)
    }

    fn from_connection(mut conn: Connection) -> Result<Self, String> {
        conn.busy_timeout(Duration::from_secs(3))
            .map_err(storage_error)?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(storage_error)?;
        if version > 5 {
            return Err("这份数据来自较新的 PomeTodo，请更新软件后打开".into());
        }
        if version == 0 {
            let tables: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                [], |row| row.get(0),
            ).map_err(storage_error)?;
            if tables != 0 {
                return Err("无法识别这份数据库，已保留原文件，请通过数据导入处理".into());
            }
            let tx = conn.transaction().map_err(storage_error)?;
            tx.execute_batch(
                "CREATE TABLE todo_tasks (
                    id TEXT PRIMARY KEY,
                    customer_name TEXT NOT NULL DEFAULT '',
                    title TEXT NOT NULL CHECK(length(trim(title)) > 0),
                    note TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL CHECK(status IN ('pending','in_progress','completed')),
                    received_at TEXT NOT NULL, due_at TEXT, urgent_at TEXT, completed_at TEXT, cancelled_at TEXT,
                    attachment_paths TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                    deleted_at TEXT, version INTEGER NOT NULL DEFAULT 1
                );
                CREATE INDEX idx_todo_visible ON todo_tasks(deleted_at, status, updated_at);
                CREATE TABLE app_preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                CREATE TABLE daily_sources (
                    id TEXT PRIMARY KEY, work_date TEXT NOT NULL, text TEXT NOT NULL DEFAULT '',
                    attachment_paths TEXT NOT NULL DEFAULT '[]',
                    status TEXT NOT NULL CHECK(status IN ('pending','processed','failed')),
                    error TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
                );
                CREATE INDEX idx_daily_sources_date ON daily_sources(work_date, created_at);
                CREATE TABLE daily_reports (
                    work_date TEXT PRIMARY KEY, content TEXT NOT NULL, generated_at TEXT NOT NULL, updated_at TEXT NOT NULL
                );
                PRAGMA user_version = 4;",
            )
            .map_err(storage_error)?;
            tx.commit().map_err(storage_error)?;
        }
        if version == 1 {
            // 客户改选填：重建任务表去掉客户非空约束，数据与列顺序原样保留。
            let tx = conn.transaction().map_err(storage_error)?;
            tx.execute_batch(
                "CREATE TABLE todo_tasks_v2 (
                    id TEXT PRIMARY KEY,
                    customer_name TEXT NOT NULL DEFAULT '',
                    title TEXT NOT NULL CHECK(length(trim(title)) > 0),
                    note TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL CHECK(status IN ('pending','in_progress','completed')),
                    received_at TEXT NOT NULL, due_at TEXT, urgent_at TEXT, completed_at TEXT,
                    attachment_paths TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                    deleted_at TEXT, version INTEGER NOT NULL DEFAULT 1
                );
                INSERT INTO todo_tasks_v2 SELECT id, customer_name, title, note, status, received_at, due_at, urgent_at, completed_at, attachment_paths, created_at, updated_at, deleted_at, version FROM todo_tasks;
                DROP TABLE todo_tasks;
                ALTER TABLE todo_tasks_v2 RENAME TO todo_tasks;
                CREATE INDEX idx_todo_visible ON todo_tasks(deleted_at, status, updated_at);
                PRAGMA user_version = 2;",
            )
            .map_err(storage_error)?;
            tx.commit().map_err(storage_error)?;
        }
        let current_version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(storage_error)?;
        if current_version == 2 {
            let tx = conn.transaction().map_err(storage_error)?;
            tx.execute_batch(
                "CREATE TABLE daily_sources (
                    id TEXT PRIMARY KEY, work_date TEXT NOT NULL, text TEXT NOT NULL DEFAULT '',
                    attachment_paths TEXT NOT NULL DEFAULT '[]',
                    status TEXT NOT NULL CHECK(status IN ('pending','processed','failed')),
                    error TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
                );
                CREATE INDEX idx_daily_sources_date ON daily_sources(work_date, created_at);
                CREATE TABLE daily_reports (
                    work_date TEXT PRIMARY KEY, content TEXT NOT NULL, generated_at TEXT NOT NULL, updated_at TEXT NOT NULL
                );
                PRAGMA user_version = 3;",
            )
            .map_err(storage_error)?;
            tx.commit().map_err(storage_error)?;
        }
        let current_version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(storage_error)?;
        if current_version == 3 {
            let tx = conn.transaction().map_err(storage_error)?;
            tx.execute_batch(
                "ALTER TABLE todo_tasks ADD COLUMN cancelled_at TEXT;
                 PRAGMA user_version = 4;",
            )
            .map_err(storage_error)?;
            tx.commit().map_err(storage_error)?;
        }
        let current_version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(storage_error)?;
        if current_version == 4 {
            let tx = conn.transaction().map_err(storage_error)?;
            tx.execute_batch("ALTER TABLE todo_tasks ADD COLUMN planned_start_at TEXT; PRAGMA user_version = 5;")
                .map_err(storage_error)?;
            tx.commit().map_err(storage_error)?;
        }
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;",
        )
        .map_err(storage_error)?;
        // 启动时确认所需列可读；不能把损坏或不兼容结构当成空任务列表。
        conn.prepare(&format!("{SELECT_TASK} WHERE deleted_at IS NULL LIMIT 0"))
            .map_err(storage_error)?;
        Ok(Self { conn })
    }

    pub fn list(&self) -> Result<Vec<TodoTask>, String> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "{SELECT_TASK} WHERE deleted_at IS NULL ORDER BY updated_at DESC, id"
            ))
            .map_err(storage_error)?;
        let rows = stmt.query_map([], read_task).map_err(storage_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(storage_error)
    }

    pub fn list_daily_sources(&self, work_date: &str) -> Result<Vec<DailySource>, String> {
        validate_work_date(work_date)?;
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id,work_date,text,attachment_paths,status,error,created_at,updated_at
             FROM daily_sources WHERE work_date=?1 ORDER BY created_at DESC, id",
            )
            .map_err(storage_error)?;
        let rows = stmt
            .query_map([work_date], read_source)
            .map_err(storage_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage_error)?;
        Ok(rows)
    }

    pub fn get_daily_source(&self, id: &str) -> Result<DailySource, String> {
        self.conn.query_row(
            "SELECT id,work_date,text,attachment_paths,status,error,created_at,updated_at FROM daily_sources WHERE id=?1",
            [id], read_source,
        ).optional().map_err(storage_error)?.ok_or_else(|| "这条素材不存在".into())
    }

    pub fn create_daily_source(
        &mut self,
        mut draft: DailySourceDraft,
    ) -> Result<DailySource, String> {
        validate_work_date(&draft.work_date)?;
        draft.text = draft.text.trim().to_string();
        if draft.text.is_empty() && draft.attachment_paths.is_empty() {
            return Err("请输入文字或添加截图".into());
        }
        if draft.text.chars().count() > 12_000 {
            return Err("素材文字不能超过 12000 字，请拆分保存".into());
        }
        if draft.attachment_paths.len() > 5 {
            return Err("一条素材最多添加 5 张截图，请拆分保存".into());
        }
        let attachments =
            serde_json::to_string(&draft.attachment_paths).map_err(|e| e.to_string())?;
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = now();
        self.conn.execute(
            "INSERT INTO daily_sources(id,work_date,text,attachment_paths,status,created_at,updated_at)
             VALUES (?1,?2,?3,?4,'pending',?5,?5)",
            params![id,draft.work_date,draft.text,attachments,timestamp],
        ).map_err(storage_error)?;
        self.get_daily_source(&id)
    }

    /// 保存一组已由用户核对过的工作记录。整个批次要么都写入，要么都不写入，
    /// 这样截图拆成多条事项时不会留下半份日报素材。
    pub fn create_daily_sources(
        &mut self,
        drafts: Vec<DailySourceDraft>,
    ) -> Result<Vec<DailySource>, String> {
        if drafts.is_empty() {
            return Err("没有可保存的工作记录".into());
        }
        if drafts.len() > 50 {
            return Err("一次最多保存 50 条工作记录".into());
        }
        let mut prepared = Vec::with_capacity(drafts.len());
        for mut draft in drafts {
            validate_work_date(&draft.work_date)?;
            draft.text = draft.text.trim().to_string();
            if draft.text.is_empty() {
                return Err("工作记录内容不能为空".into());
            }
            if draft.text.chars().count() > 12_000 {
                return Err("工作记录不能超过 12000 字，请拆分保存".into());
            }
            if draft.attachment_paths.len() > 5 {
                return Err("一条工作记录最多关联 5 张截图".into());
            }
            prepared.push(draft);
        }
        let timestamp = now();
        let mut sources = Vec::with_capacity(prepared.len());
        let tx = self.conn.transaction().map_err(storage_error)?;
        for draft in prepared {
            let id = uuid::Uuid::new_v4().to_string();
            let attachments = serde_json::to_string(&draft.attachment_paths).map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT INTO daily_sources(id,work_date,text,attachment_paths,status,created_at,updated_at)
                 VALUES (?1,?2,?3,?4,'processed',?5,?5)",
                params![id,draft.work_date,draft.text,attachments,timestamp],
            ).map_err(storage_error)?;
            sources.push(DailySource {
                id,
                work_date: draft.work_date,
                text: draft.text,
                attachment_paths: draft.attachment_paths,
                status: DailySourceStatus::Processed,
                error: None,
                created_at: timestamp.clone(),
                updated_at: timestamp.clone(),
            });
        }
        tx.commit().map_err(storage_error)?;
        Ok(sources)
    }

    pub fn update_daily_source(
        &mut self,
        id: &str,
        text: &str,
    ) -> Result<DailySource, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("工作记录内容不能为空".into());
        }
        if text.chars().count() > 12_000 {
            return Err("工作记录不能超过 12000 字，请拆分保存".into());
        }
        let changed = self.conn.execute(
            "UPDATE daily_sources SET text=?2,status='processed',error=NULL,updated_at=?3 WHERE id=?1",
            params![id, text, now()],
        ).map_err(storage_error)?;
        if changed == 0 {
            return Err("这条工作记录不存在".into());
        }
        self.get_daily_source(id)
    }

    pub fn delete_daily_source(&mut self, id: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM daily_sources WHERE id=?1", [id])
            .map_err(storage_error)?;
        Ok(())
    }

    pub fn mark_daily_source(
        &mut self,
        id: &str,
        status: DailySourceStatus,
        error: Option<&str>,
    ) -> Result<DailySource, String> {
        self.conn
            .execute(
                "UPDATE daily_sources SET status=?2,error=?3,updated_at=?4 WHERE id=?1",
                params![id, source_status_name(status), error, now()],
            )
            .map_err(storage_error)?;
        self.get_daily_source(id)
    }

    pub fn report(&self, work_date: &str) -> Result<Option<DailyReport>, String> {
        validate_work_date(work_date)?;
        self.conn.query_row(
            "SELECT work_date,content,generated_at,updated_at FROM daily_reports WHERE work_date=?1",
            [work_date],
            |row| Ok(DailyReport { work_date: row.get(0)?, content: row.get(1)?, generated_at: row.get(2)?, updated_at: row.get(3)? }),
        ).optional().map_err(storage_error)
    }

    pub fn save_report(&mut self, work_date: &str, content: &str) -> Result<DailyReport, String> {
        validate_work_date(work_date)?;
        let content = content.trim();
        if content.is_empty() {
            return Err("日报内容不能为空".into());
        }
        let timestamp = now();
        self.conn.execute(
            "INSERT INTO daily_reports(work_date,content,generated_at,updated_at) VALUES (?1,?2,?3,?3)
             ON CONFLICT(work_date) DO UPDATE SET content=excluded.content,updated_at=excluded.updated_at",
            params![work_date,content,timestamp],
        ).map_err(storage_error)?;
        self.report(work_date)?.ok_or_else(|| "日报保存失败".into())
    }

    pub fn create(&mut self, draft: TodoDraft) -> Result<TodoTask, String> {
        let draft = validate(draft)?;
        let attachments =
            serde_json::to_string(&draft.attachment_paths).map_err(|error| error.to_string())?;
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = now();
        let tx = self.conn.transaction().map_err(storage_error)?;
        tx.execute(
            "INSERT INTO todo_tasks (id, customer_name, title, note, status, received_at, planned_start_at, due_at,
                attachment_paths, created_at, updated_at) VALUES (?1,?2,?3,?4,'pending',?5,?6,?7,?8,?9,?9)",
            params![id, draft.customer_name, draft.title, draft.note, draft.received_at, draft.planned_start_at, draft.due_at, attachments, timestamp],
        ).map_err(storage_error)?;
        let result = get_task(&tx, &id)?;
        tx.commit().map_err(storage_error)?;
        Ok(result)
    }

    pub fn update(&mut self, id: &str, draft: TodoDraft) -> Result<TodoTask, String> {
        let draft = validate(draft)?;
        let attachments =
            serde_json::to_string(&draft.attachment_paths).map_err(|error| error.to_string())?;
        let tx = self.conn.transaction().map_err(storage_error)?;
        tx.execute(
            "UPDATE todo_tasks SET customer_name=?2, title=?3, note=?4, received_at=?5, planned_start_at=?6, due_at=?7,
                attachment_paths=?8, updated_at=?9, version=version+1 WHERE id=?1 AND deleted_at IS NULL",
            params![id, draft.customer_name, draft.title, draft.note, draft.received_at, draft.planned_start_at, draft.due_at, attachments, now()],
        ).map_err(storage_error)?;
        let result = get_task(&tx, id)?;
        tx.commit().map_err(storage_error)?;
        Ok(result)
    }

    pub fn set_status(&mut self, id: &str, status: TodoStatus) -> Result<TodoTask, String> {
        let tx = self.conn.transaction().map_err(storage_error)?;
        let existing = get_task(&tx, id)?;
        if existing.status == status {
            return Ok(existing);
        }
        let timestamp = now();
        let completed_at = (status == TodoStatus::Completed).then_some(timestamp.clone());
        tx.execute(
            "UPDATE todo_tasks SET status=?2, completed_at=?3,
                cancelled_at=NULL,
                urgent_at=CASE WHEN ?2='completed' THEN NULL ELSE urgent_at END,
                updated_at=?4, version=version+1 WHERE id=?1 AND deleted_at IS NULL",
            params![id, status_name(status), completed_at, timestamp],
        )
        .map_err(storage_error)?;
        let result = get_task(&tx, id)?;
        tx.commit().map_err(storage_error)?;
        Ok(result)
    }

    pub fn cancel_task(&mut self, id: &str) -> Result<TodoTask, String> {
        let tx = self.conn.transaction().map_err(storage_error)?;
        let existing = get_task(&tx, id)?;
        if existing.cancelled_at.is_some() {
            return Ok(existing);
        }
        if existing.status == TodoStatus::Completed {
            return Err("已完成的待办不能取消".into());
        }
        let timestamp = now();
        tx.execute(
            "UPDATE todo_tasks SET status='completed', completed_at=?2, cancelled_at=?2,
                urgent_at=NULL, updated_at=?2, version=version+1
             WHERE id=?1 AND deleted_at IS NULL",
            params![id, timestamp],
        )
        .map_err(storage_error)?;
        let result = get_task(&tx, id)?;
        tx.commit().map_err(storage_error)?;
        Ok(result)
    }

    pub fn set_urgent(&mut self, id: &str, urgent: bool) -> Result<TodoTask, String> {
        let tx = self.conn.transaction().map_err(storage_error)?;
        let task = get_task(&tx, id)?;
        if urgent && task.status == TodoStatus::Completed {
            return Err("已完成的待办无需加急".into());
        }
        let timestamp = now();
        let urgent_at = urgent.then_some(timestamp.clone());
        tx.execute(
            "UPDATE todo_tasks SET urgent_at=?2, updated_at=?3, version=version+1 WHERE id=?1 AND deleted_at IS NULL",
            params![id, urgent_at, timestamp],
        ).map_err(storage_error)?;
        let result = get_task(&tx, id)?;
        tx.commit().map_err(storage_error)?;
        Ok(result)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        self.conn.execute(
            "UPDATE todo_tasks SET deleted_at=?2, updated_at=?2, version=version+1 WHERE id=?1 AND deleted_at IS NULL",
            params![id, now()],
        ).map_err(storage_error)?;
        Ok(())
    }

    pub fn theme(&self) -> Result<String, String> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM app_preferences WHERE key='theme'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)?
            .unwrap_or_else(|| "system".into()))
    }

    pub(crate) fn read_preference(&self, key: &str) -> Result<Option<String>, String> {
        self.conn
            .query_row(
                "SELECT value FROM app_preferences WHERE key=?1",
                [key],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)
    }

    pub(crate) fn save_preferences(&mut self, values: &[(&str, &str)]) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(storage_error)?;
        for (key, value) in values {
            tx.execute("INSERT INTO app_preferences(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![key,value]).map_err(storage_error)?;
        }
        tx.commit().map_err(storage_error)
    }

    pub fn set_theme(&mut self, theme: &str) -> Result<(), String> {
        if !matches!(theme, "light" | "dark" | "system") {
            return Err("不支持的主题选项".into());
        }
        self.conn.execute(
            "INSERT INTO app_preferences(key,value) VALUES ('theme',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [theme],
        ).map_err(storage_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
