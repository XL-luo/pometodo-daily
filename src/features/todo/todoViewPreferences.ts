export type TodoGroupingMode = "default" | "source";
export interface TodoViewPreferences {
  mode: TodoGroupingMode;
  collapsedSources: string[];
}

export const TODO_VIEW_PREFERENCES_KEY = "pometodo.todo-view.v1";
const defaults = (): TodoViewPreferences => ({ mode: "default", collapsedSources: [] });

export function parseTodoViewPreferences(raw: string | null): TodoViewPreferences {
  try {
    const value: unknown = JSON.parse(raw ?? "null");
    if (!value || typeof value !== "object") return defaults();
    const prefs = value as Partial<TodoViewPreferences>;
    return {
      mode: prefs.mode === "source" ? "source" : "default",
      collapsedSources: Array.isArray(prefs.collapsedSources)
        ? [...new Set(prefs.collapsedSources.filter((key): key is string => typeof key === "string"))]
        : [],
    };
  } catch { return defaults(); }
}

export function loadTodoViewPreferences(): TodoViewPreferences {
  try { return parseTodoViewPreferences(window.localStorage.getItem(TODO_VIEW_PREFERENCES_KEY)); }
  catch { return defaults(); }
}

export function saveTodoViewPreferences(preferences: TodoViewPreferences): void {
  // 存储不可用时仍可在当前会话正常操作列表。
  try { window.localStorage.setItem(TODO_VIEW_PREFERENCES_KEY, JSON.stringify(preferences)); }
  catch { /* 本机界面偏好不影响任务保存。 */ }
}
