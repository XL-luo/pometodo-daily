use super::*;

#[test]
fn failed_restore_rolls_back_the_delete_and_keeps_preferences() {
    let mut current = TodoRepository::open_in_memory().unwrap();
    let saved = current.create(sample()).unwrap();
    current.set_theme("dark").unwrap();
    let mut source = TodoRepository::open_in_memory().unwrap();
    source.create(sample()).unwrap();
    let records = source.backup_records().unwrap();
    current.conn.execute_batch("CREATE TRIGGER fail_restore BEFORE INSERT ON todo_tasks BEGIN SELECT RAISE(ABORT, 'simulated disk failure'); END;").unwrap();
    assert!(current
        .replace_backup_records(&records, &std::collections::BTreeMap::new())
        .is_err());
    assert_eq!(current.list().unwrap()[0].id, saved.id);
    assert_eq!(current.theme().unwrap(), "dark");
}

fn sample() -> TodoDraft {
    TodoDraft {
        customer_name: "示例客户".into(),
        title: "准备示例文档".into(),
        note: "虚构测试内容".into(),
        received_at: "2026-09-06T09:00:00+08:00".into(),
        planned_start_at: None,
        due_at: None,
        attachment_paths: vec![],
    }
}

#[test]
fn tasks_survive_reopen_with_nullable_due_date() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("todo.sqlite");
    let created = {
        let mut repo = TodoRepository::open(&db).unwrap();
        repo.create(sample()).unwrap()
    };
    let repo = TodoRepository::open(&db).unwrap();
    let tasks = repo.list().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, created.id);
    assert_eq!(tasks[0].customer_name, "示例客户");
    assert_eq!(tasks[0].due_at, None);
    assert_eq!(tasks[0].status, TodoStatus::Pending);
}

#[test]
fn required_fields_and_invalid_dates_do_not_create_rows() {
    let mut repo = TodoRepository::open_in_memory().unwrap();
    // 客户为选填：空白客户会保留为空字符串并正常建任务。
    let mut draft = sample();
    draft.customer_name = "  ".into();
    let created_without_customer = repo.create(draft).unwrap();
    assert_eq!(created_without_customer.customer_name, "");
    let mut draft = sample();
    draft.title = "\n".into();
    assert!(repo.create(draft).is_err());
    let mut draft = sample();
    draft.due_at = Some("明天".into());
    assert!(repo.create(draft).is_err());
    // 仅空客户任务入库。
    assert_eq!(repo.list().unwrap().len(), 1);
}

#[test]
fn daily_batch_save_is_atomic_and_records_can_be_edited() {
    let mut repo = TodoRepository::open_in_memory().unwrap();
    let saved = repo
        .create_daily_sources(vec![
            DailySourceDraft { work_date: "2026-09-28".into(), text: "处理客户反馈".into(), attachment_paths: vec!["shared-image".into()] },
            DailySourceDraft { work_date: "2026-09-28".into(), text: "确认交付范围".into(), attachment_paths: vec!["shared-image".into()] },
        ])
        .unwrap();
    assert_eq!(saved.len(), 2);
    assert!(saved.iter().all(|source| source.status == DailySourceStatus::Processed));
    assert_eq!(repo.list_daily_sources("2026-09-28").unwrap().len(), 2);
    let edited = repo.update_daily_source(&saved[0].id, "处理客户反馈并回复").unwrap();
    assert_eq!(edited.text, "处理客户反馈并回复");
    assert_eq!(edited.attachment_paths, ["shared-image"]);
    let bad = repo.create_daily_sources(vec![
        DailySourceDraft { work_date: "2026-09-28".into(), text: "仍然有效".into(), attachment_paths: vec![] },
        DailySourceDraft { work_date: "invalid".into(), text: "不能写入".into(), attachment_paths: vec![] },
    ]);
    assert!(bad.is_err());
    assert_eq!(repo.list_daily_sources("2026-09-28").unwrap().len(), 2);
}

#[test]
fn edits_keep_identity_and_completion_can_be_reversed() {
    let mut repo = TodoRepository::open_in_memory().unwrap();
    let created = repo.create(sample()).unwrap();
    let urgent = repo.set_urgent(&created.id, true).unwrap();
    assert!(urgent.urgent_at.is_some());
    let done = repo.set_status(&created.id, TodoStatus::Completed).unwrap();
    assert!(done.completed_at.is_some());
    assert!(done.urgent_at.is_none());
    let restored = repo
        .set_status(&created.id, TodoStatus::InProgress)
        .unwrap();
    assert!(restored.completed_at.is_none());
    let mut draft = sample();
    draft.title = "修改后的示例".into();
    draft.due_at = Some("2026-09-07T00:00:00+08:00".into());
    let edited = repo.update(&created.id, draft).unwrap();
    assert_eq!(edited.id, created.id);
    assert_eq!(edited.status, TodoStatus::InProgress);
    assert_eq!(edited.title, "修改后的示例");
    assert!(edited.due_at.is_some());
    let without_due = repo.update(&created.id, sample()).unwrap();
    assert!(without_due.due_at.is_none());
    assert!(repo.update("missing", sample()).is_err());
}

#[test]
fn cancellation_is_persisted_distinct_from_normal_completion_and_restores_cleanly() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("cancel.sqlite");
    let (id, cancelled_at) = {
        let mut repo = TodoRepository::open(&path).unwrap();
        let task = repo.create(sample()).unwrap();
        repo.set_urgent(&task.id, true).unwrap();
        let cancelled = repo.cancel_task(&task.id).unwrap();
        assert_eq!(cancelled.status, TodoStatus::Completed);
        assert!(cancelled.cancelled_at.is_some());
        assert_eq!(cancelled.completed_at, cancelled.cancelled_at);
        assert!(cancelled.urgent_at.is_none());
        let repeated = repo.cancel_task(&task.id).unwrap();
        assert_eq!(repeated.cancelled_at, cancelled.cancelled_at);
        (task.id, cancelled.cancelled_at)
    };
    let mut repo = TodoRepository::open(&path).unwrap();
    let loaded = repo.list().unwrap().into_iter().find(|task| task.id == id).unwrap();
    assert_eq!(loaded.cancelled_at, cancelled_at);
    let restored = repo.set_status(&id, TodoStatus::InProgress).unwrap();
    assert!(restored.cancelled_at.is_none());
    assert!(restored.completed_at.is_none());
    let normal = repo.set_status(&id, TodoStatus::Completed).unwrap();
    assert!(normal.cancelled_at.is_none());
    assert!(repo.cancel_task(&id).is_err());
}

#[test]
fn deleted_tasks_are_hidden_but_keep_sync_metadata() {
    let mut repo = TodoRepository::open_in_memory().unwrap();
    let task = repo.create(sample()).unwrap();
    repo.delete(&task.id).unwrap();
    assert!(repo.list().unwrap().is_empty());
    let metadata: (Option<String>, i64) = repo
        .conn
        .query_row(
            "SELECT deleted_at, version FROM todo_tasks WHERE id = ?1",
            [&task.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(metadata.0.is_some());
    assert_eq!(metadata.1, 2);
    assert!(repo.update(&task.id, sample()).is_err());
    assert!(repo.set_status(&task.id, TodoStatus::Pending).is_err());
    // 删除重试是幂等的，不复活或重复写入。
    repo.delete(&task.id).unwrap();
}

#[test]
fn storage_write_failures_reach_the_caller() {
    let mut repo = TodoRepository::open_in_memory().unwrap();
    let task = repo.create(sample()).unwrap();
    repo.conn.execute_batch("PRAGMA query_only = ON").unwrap();
    assert!(repo.create(sample()).is_err());
    assert!(repo.update(&task.id, sample()).is_err());
    assert!(repo.delete(&task.id).is_err());
    assert_eq!(repo.list().unwrap()[0].title, task.title);
}

#[test]
fn corrupt_or_newer_databases_are_never_replaced() {
    let temp = tempfile::tempdir().unwrap();
    let corrupt = temp.path().join("corrupt.sqlite");
    std::fs::write(&corrupt, b"not a database").unwrap();
    assert!(TodoRepository::open(&corrupt).is_err());
    assert_eq!(std::fs::read(&corrupt).unwrap(), b"not a database");
    let future = temp.path().join("future.sqlite");
    let conn = rusqlite::Connection::open(&future).unwrap();
    conn.execute_batch("PRAGMA user_version = 999").unwrap();
    drop(conn);
    assert!(TodoRepository::open(&future).is_err());
}

#[test]
fn theme_preference_survives_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.sqlite");
    let mut repo = TodoRepository::open(&path).unwrap();
    assert_eq!(repo.theme().unwrap(), "system");
    repo.set_theme("dark").unwrap();
    assert!(repo.set_theme("arbitrary").is_err());
    drop(repo);
    let repo = TodoRepository::open(&path).unwrap();
    assert_eq!(repo.theme().unwrap(), "dark");
}

#[test]
fn daily_sources_and_report_survive_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("daily.sqlite");
    let source_id = {
        let mut repo = TodoRepository::open(&path).unwrap();
        let source = repo
            .create_daily_source(DailySourceDraft {
                work_date: "2026-09-28".into(),
                text: "完成日报功能验证".into(),
                attachment_paths: vec![],
            })
            .unwrap();
        repo.save_report("2026-09-28", "今日完成\n- 完成日报功能验证")
            .unwrap();
        source.id
    };
    let repo = TodoRepository::open(&path).unwrap();
    assert_eq!(
        repo.list_daily_sources("2026-09-28").unwrap()[0].id,
        source_id
    );
    assert!(repo
        .report("2026-09-28")
        .unwrap()
        .unwrap()
        .content
        .contains("日报功能"));
}

#[test]
fn version_two_database_migrates_without_losing_tasks() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("version-two.sqlite");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE todo_tasks (
            id TEXT PRIMARY KEY, customer_name TEXT NOT NULL DEFAULT '',
            title TEXT NOT NULL CHECK(length(trim(title)) > 0), note TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL CHECK(status IN ('pending','in_progress','completed')),
            received_at TEXT NOT NULL, due_at TEXT, urgent_at TEXT, completed_at TEXT,
            attachment_paths TEXT NOT NULL DEFAULT '[]', created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL, deleted_at TEXT, version INTEGER NOT NULL DEFAULT 1
        );
        CREATE INDEX idx_todo_visible ON todo_tasks(deleted_at, status, updated_at);
        CREATE TABLE app_preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        INSERT INTO todo_tasks VALUES ('00000000-0000-4000-8000-000000000001','','旧任务','','pending',
            '2026-09-01T00:00:00Z',NULL,NULL,NULL,'[]','2026-09-01T00:00:00Z','2026-09-01T00:00:00Z',NULL,1);
        PRAGMA user_version = 2;",
    ).unwrap();
    drop(conn);
    let repo = TodoRepository::open(&path).unwrap();
    assert_eq!(repo.list().unwrap()[0].title, "旧任务");
    assert!(repo.list().unwrap()[0].planned_start_at.is_none());
    assert!(repo.list_daily_sources("2026-09-28").unwrap().is_empty());
    assert_eq!(
        repo.conn
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        5
    );
}

#[test]
fn planned_range_survives_reopen_edits_defer_and_clear() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("planned.sqlite");
    let mut draft = sample();
    draft.planned_start_at = Some("2026-10-01T00:00:00+08:00".into());
    draft.due_at = Some("2026-10-05T00:00:00+08:00".into());
    let mut repo = TodoRepository::open(&path).unwrap();
    let saved = repo.create(draft.clone()).unwrap();
    drop(repo);
    let mut repo = TodoRepository::open(&path).unwrap();
    assert_eq!(repo.list().unwrap()[0].planned_start_at, draft.planned_start_at);
    draft.due_at = Some("2026-10-08T00:00:00+08:00".into());
    assert_eq!(repo.update(&saved.id, draft.clone()).unwrap().planned_start_at, draft.planned_start_at);
    assert_eq!(repo.set_status(&saved.id, TodoStatus::InProgress).unwrap().planned_start_at, draft.planned_start_at);
    draft.planned_start_at = None;
    assert!(repo.update(&saved.id, draft).unwrap().planned_start_at.is_none());
    drop(repo);
    assert!(TodoRepository::open(&path).unwrap().list().unwrap()[0].planned_start_at.is_none());
}

#[test]
fn invalid_planned_dates_never_create_or_replace_tasks() {
    let mut repo = TodoRepository::open_in_memory().unwrap();
    let saved = repo.create(sample()).unwrap();
    for start in ["2026-02-30T00:00:00+08:00", "2026-10-06T00:00:00+08:00"] {
        let mut draft = sample();
        draft.planned_start_at = Some(start.into());
        draft.due_at = Some("2026-10-05T00:00:00+08:00".into());
        assert!(repo.create(draft.clone()).is_err());
        assert!(repo.update(&saved.id, draft).is_err());
        assert_eq!(repo.list().unwrap(), [saved.clone()]);
    }
    // 比較日期而非時刻，同一天的開始時間晚於完成時間仍合法。
    let mut draft = sample();
    draft.planned_start_at = Some("2026-10-05T23:00:00+08:00".into());
    draft.due_at = Some("2026-10-05T08:00:00+08:00".into());
    assert!(repo.update(&saved.id, draft).is_ok());
}

#[test]
fn version_four_upgrade_preserves_tasks_preferences_and_daily_records() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("version-four.sqlite");
    let mut repo = TodoRepository::open(&path).unwrap();
    let saved = repo.create(sample()).unwrap();
    repo.set_theme("dark").unwrap();
    let source = repo.create_daily_source(DailySourceDraft { work_date: "2026-10-01".into(), text: "旧工作记录".into(), attachment_paths: vec![] }).unwrap();
    repo.conn.execute_batch("ALTER TABLE todo_tasks DROP COLUMN planned_start_at; PRAGMA user_version = 4;").unwrap();
    drop(repo);
    let repo = TodoRepository::open(&path).unwrap();
    assert_eq!(repo.list().unwrap(), [saved]);
    assert!(repo.list().unwrap()[0].planned_start_at.is_none());
    assert_eq!(repo.theme().unwrap(), "dark");
    assert_eq!(repo.list_daily_sources("2026-10-01").unwrap()[0].id, source.id);
    assert_eq!(repo.conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0)).unwrap(), 5);
}
