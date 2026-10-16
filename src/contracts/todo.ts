/**
 * PomeTodo 界面与原生待办服务的数据接口。
 * 日期是 ISO 时间字符串；表单显示时按本地日历转换，dueAt 可为 null。
 * attachmentPaths 由原生服务发放，UI 将其作为不透明标识传回服务。
 */
export type TodoStatus = "pending" | "in_progress" | "completed";
export type TodoFilter = "today" | TodoStatus;
export type TodoStatusCounts = Record<TodoFilter, number>;

export interface TodoTask {
  id: string;
  customerName: string;
  title: string;
  note: string;
  status: TodoStatus;
  receivedAt: string;
  plannedStartAt?: string | null;
  dueAt: string | null;
  urgentAt?: string | null;
  completedAt: string | null;
  cancelledAt?: string | null;
  attachmentPaths: string[];
  updatedAt?: string;
}

/** 新建和编辑共用；不包含 ID、完成时间、版本等服务端管理的字段。 */
export interface TodoDraft {
  customerName: string;
  title: string;
  note: string;
  receivedAt: string;
  plannedStartAt?: string | null;
  dueAt: string | null;
  attachmentPaths: string[];
}

export interface TodoRecognitionDraft {
  customerName?: string | null;
  title?: string | null;
  note?: string | null;
  receivedAt?: string | null;
  plannedStartAt?: string | null;
  /** null 表示明确没有交付日期并清空默认值；未提供则保留手工值。 */
  dueAt?: string | null;
  attachmentPaths?: string[] | null;
}

/** 只有 kind=ai 可以在界面显示“AI 整理成功”；异常以 rejected Promise 返回。 */
export type TodoRecognitionResult =
  | { kind: "ai"; draft: TodoRecognitionDraft; message?: string }
  | { kind: "local"; draft: TodoRecognitionDraft; message: string }
  | { kind: "empty"; message: string };

/**
 * 宿主注入稳定的 services 对象。
 * 写操作 resolve 表示已持久化，并返回保存后的任务；失败 reject Error，message 可供用户阅读。
 * UI 按返回值更新列表，不在一次成功保存后强制重新读取再将读取失败当作保存失败。
 * 不吞掉错误，不用 UI 本地缓存冒充持久化；次数计算完全不属于本接口的 UI 实现。
 */
export interface TodoServices {
  listTasks(): Promise<TodoTask[]>;
  createTask(draft: TodoDraft): Promise<TodoTask>;
  updateTask(id: string, draft: TodoDraft): Promise<TodoTask>;
  setTaskStatus(id: string, status: TodoStatus): Promise<TodoTask>;
  cancelTask(id: string): Promise<TodoTask>;
  setTaskUrgent(id: string, urgent: boolean): Promise<TodoTask>;
  deleteTask(id: string): Promise<void>;
  recognizeClipboard(): Promise<TodoRecognitionResult>;
  /** 识别附件箱中指定截图（附件优先）。 */
  recognizeAttachment(id: string): Promise<TodoRecognitionResult>;
  saveClipboardAttachment(): Promise<string | null>;
  getAttachmentDataUrl(path: string): Promise<string>;
}

export interface TodoPageProps {
  services: TodoServices;
  /** 宿主打开设置时保留草稿，同时暂停后台快捷键。 */
  active?: boolean;
  refreshRequest?: number;
  customerSuggestions?: string[];
  /** sltool 快捷日期选项 JSON；未给出时沿用原模块默认值。 */
  quickDueOptions?: string;
  /** 原生事件由宿主接收。每次请求递增 requestId，UI 定位已有任务而不重复创建。 */
  focusTaskRequest?: { taskId: string; requestId: number };
}
