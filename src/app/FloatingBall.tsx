import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import type { TodoTask } from "../contracts/todo";
import { getTodoDueInfo } from "../features/todo/todoModel";
import { wordPackFor } from "../features/todo/wordPacks";
import "./floating.css";

type DockSide = "left" | "right";
type DockStatus = { side: DockSide | null; hidden: boolean };

/** The preview puts active urgent work first, then follows the WPF due/status/update order. */
export function getFloatingTasks(rows: TodoTask[]): TodoTask[] {
  const dueTime = (value: string | null) => {
    const time = value ? Date.parse(value) : NaN;
    return Number.isFinite(time) ? time : Infinity;
  };
  return rows.filter(task => task.status === "pending" || task.status === "in_progress")
    .sort((a, b) => {
      const aUrgent = Boolean(a.urgentAt);
      const bUrgent = Boolean(b.urgentAt);
      if (aUrgent !== bUrgent) return bUrgent ? 1 : -1;
      if (aUrgent && bUrgent) {
        const urgentDiff = (Date.parse(b.urgentAt ?? "") || 0) - (Date.parse(a.urgentAt ?? "") || 0);
        if (urgentDiff !== 0) return urgentDiff;
      }
      return dueTime(a.dueAt) - dueTime(b.dueAt)
      || Number(b.status === "in_progress") - Number(a.status === "in_progress")
      || (Date.parse(b.updatedAt ?? "") || 0) - (Date.parse(a.updatedAt ?? "") || 0);
    });
}

export function getFloatingTaskStatusLabel(task: TodoTask) {
  const statusLabel = task.status === "in_progress" ? "进行中" : "未完成";
  return task.urgentAt ? `加急 · ${statusLabel}` : statusLabel;
}

export function FloatingBall({ previewOnly = false }: { previewOnly?: boolean }) {
  const [tasks, setTasks] = useState<TodoTask[]>([]);
  const [customerLabel, setCustomerLabel] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [above, setAbove] = useState(false);
  const [dock, setDock] = useState<DockStatus>({ side: null, hidden: false });
  const expandedRef = useRef(false);
  const changing = useRef(false);
  const transitionVersion = useRef(0);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const drag = useRef<{ x: number; y: number; dragged: boolean } | null>(null);
  const ball = useRef<HTMLButtonElement>(null);

  const collapse = useCallback(() => {
    transitionVersion.current += 1;
    expandedRef.current = false;
    setExpanded(false);
    if (closeTimer.current) clearTimeout(closeTimer.current);
    // Only the ball owns the close timer; the preview only plays its fade.
    if (previewOnly) return;
    closeTimer.current = setTimeout(() => {
      closeTimer.current = null;
      void invoke("pometodo_float_expand", { expanded: false })
        .catch(() => setError("浮球未能收起，请重试。"));
    }, 120);
  }, [previewOnly]);
  const requestCollapse = () => { void emit("pometodo-floating-collapse"); };

  useEffect(() => {
    let active = true;
    let refreshVersion = 0;
    let preference = "system";
    const media = matchMedia("(prefers-color-scheme: dark)");
    const applyTheme = () => {
      document.documentElement.dataset.theme = preference === "system" ? (media.matches ? "dark" : "light") : preference;
    };
    const refresh = async () => {
      const version = ++refreshVersion;
      try {
        const [rows, info, settings] = await Promise.all([
          invoke<TodoTask[]>("pometodo_list_tasks"),
          invoke<{ theme: string }>("pometodo_app_info"),
          invoke<{ settings: { customerLabel: string } }>("pometodo_settings_load"),
        ]);
        if (!active || version !== refreshVersion) return;
        preference = info.theme;
        applyTheme();
        setCustomerLabel(settings.settings.customerLabel);
        setTasks(getFloatingTasks(rows));
        setError(null);
      } catch {
        if (active && version === refreshVersion) setError("暂时无法读取待办，请打开主窗口检查。");
      }
    };
    void refresh();
    const present = (payload: boolean) => {
        if (!previewOnly || !active) return;
        if (closeTimer.current) { clearTimeout(closeTimer.current); closeTimer.current = null; }
        const version = ++transitionVersion.current;
        setAbove(payload);
        setExpanded(false);
        requestAnimationFrame(() => requestAnimationFrame(() => {
          if (!active || version !== transitionVersion.current) return;
          void invoke<boolean>("pometodo_float_present").then(shown => {
            if (!active || version !== transitionVersion.current) return;
            expandedRef.current = shown;
            if (shown) requestAnimationFrame(() => { if (active && version === transitionVersion.current) setExpanded(true); });
          }).catch(() => setError("预览未能显示"));
        }));
      };
    const presentSubscription = listen<boolean>("pometodo-floating-present", ({ payload }) => present(payload));
    let dockVersion = 0;
    const dockSubscription = previewOnly ? Promise.resolve(() => {}) : listen<DockStatus>("pometodo-floating-dock-state", ({ payload }) => {
      dockVersion += 1;
      if (active) setDock(payload);
    });
    const subscriptions = [
      listen("pometodo-floating-refresh", () => void refresh()),
      listen("pometodo-floating-collapse", collapse),
      listen<string>("pometodo-floating-error", ({ payload }) => { if (active) setError(payload); }),
      listen("pometodo-floating-closed", () => { expandedRef.current = false; setExpanded(false); }),
      presentSubscription,
      dockSubscription,
    ];
    const initialVersion = transitionVersion.current;
    if (previewOnly) void Promise.all(subscriptions).then(() => invoke<boolean | null>("pometodo_float_preview_state"))
      .then(value => { if (active && initialVersion === transitionVersion.current && value !== null) present(value); })
      .catch(() => { if (active) setError("预览暂不可用"); });
    if (!previewOnly) void dockSubscription.then(async () => {
      const version = dockVersion;
      const status = await invoke<DockStatus>("pometodo_float_dock_state");
      if (active && version === dockVersion) setDock(status);
    }).catch(() => {});
    for (const subscription of subscriptions) void subscription.catch(() => { if (active) setError("浮球事件连接失败，请重新打开应用。"); });
    const timer = setInterval(() => void refresh(), 30000);
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        void emit("pometodo-floating-collapse");
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    media.addEventListener("change", applyTheme);
    return () => {
      active = false;
      clearInterval(timer);
      window.removeEventListener("keydown", handleKeyDown);
      if (closeTimer.current) clearTimeout(closeTimer.current);
      media.removeEventListener("change", applyTheme);
      for (const subscription of subscriptions) void subscription.then(unlisten => unlisten()).catch(() => {});
    };
  }, [collapse, previewOnly]);

  async function toggle() {
    if (expandedRef.current) { requestCollapse(); return; }
    if (changing.current) return;
    changing.current = true;
    const version = ++transitionVersion.current;
    if (closeTimer.current) { clearTimeout(closeTimer.current); closeTimer.current = null; }
    try {
      await invoke("pometodo_float_expand", { expanded: true });
      if (version !== transitionVersion.current) return;
      expandedRef.current = true;
      requestAnimationFrame(() => requestAnimationFrame(() => {
        if (expandedRef.current) setExpanded(true);
      }));
    } catch { setError("浮球未能展开，请从托盘打开待办。"); }
    finally { changing.current = false; }
  }

  async function open(taskId?: string) {
    try {
      await invoke("pometodo_open_main", { taskId: taskId ?? null, settings: false });
      requestCollapse();
    } catch { setError("未能打开待办，请从托盘重试。"); }
  }

  return <div
    className={`pome-floating-shell${previewOnly ? " preview-only" : ""}${above ? " above" : ""}${expanded ? " expanded" : ""}${dock.side ? ` docked dock-${dock.side}` : ""}${dock.hidden ? " docked-hidden" : ""}`}
  >
    {!previewOnly && <div className="pome-floating-ball-row">
      <button ref={ball} className="pome-floating-ball" aria-expanded={expanded} aria-controls="floating-preview"
        aria-label={`${expanded ? "收起" : "展开"}待办浮球，共 ${tasks.length} 项`}
        onContextMenu={event => {
          event.preventDefault();
          void invoke("pometodo_float_menu").catch(() => setError("浮球菜单未能打开，请从主窗口进入设置。"));
        }}
        onPointerDown={event => {
          if (event.button === 0) {
            drag.current = { x: event.clientX, y: event.clientY, dragged: false };
            event.currentTarget.setPointerCapture(event.pointerId);
          }
        }}
        onPointerMove={event => {
          const start = drag.current;
          if (start && event.buttons === 1 && !start.dragged && Math.hypot(event.clientX - start.x, event.clientY - start.y) >= 5) {
            start.dragged = true;
            event.currentTarget.releasePointerCapture(event.pointerId);
            void invoke("pometodo_float_drag_start").catch(() => setError("浮球未能移动，请重试。"));
          }
        }}
        onClick={() => { if (!drag.current?.dragged) void toggle(); drag.current = null; }}>
        <span className="pome-float-shadow" aria-hidden="true" />
        <span className="pome-float-ring" aria-hidden="true" />
        <span className="pome-float-circle"><strong>{tasks.length}</strong><span>待办</span></span>
      </button>
    </div>}
    {previewOnly && <div className="pome-float-card-layer" inert={!expanded} aria-hidden={!expanded}>
      <div className="pome-float-arrow" />
      <section id="floating-preview" className="pome-float-panel" aria-label="最近待办">
        {error && <p className="pome-float-error" role="alert">{error}</p>}
        <div className="pome-float-tasks">{tasks.slice(0, 3).map(task => {
          const wordPack = wordPackFor(customerLabel ?? undefined);
          const due = getTodoDueInfo(task, new Date(), { today: `今天${wordPack.dueWord}`, tomorrow: `明天${wordPack.dueWord}`, onTime: `按时${wordPack.dueWord}`, late: days => `逾期 ${days} 天${wordPack.dueWord}`, unscheduled: `未设${wordPack.dueWord}日期`, clear: `不设${wordPack.dueWord}日期`, expiringToday: `${wordPack.todayWord}到期` });
          const taskLabel = getFloatingTaskStatusLabel(task);
          return <button className="pome-float-task" key={task.id} title={taskLabel} onClick={() => void open(task.id)}>
            <div className="pome-float-task-heading">
              <span className={`pome-float-status ${task.status}${task.urgentAt ? " urgent" : ""}`} aria-label={taskLabel} />
              <strong>{task.title}</strong>
              {task.urgentAt && <span className="pome-float-urgent-badge" aria-label="加急">急</span>}
              <small className={`pome-float-due ${due?.tone ?? "normal"}`}>{due?.label ?? `未设${wordPack.dueWord}`}</small>
            </div>
            <span className="pome-float-customer">{task.customerName}</span>
          </button>;
        })}</div>
        <button className="pome-float-view-all" onClick={() => void open()}>查看全部 →</button>
      </section>
    </div>}
  </div>;
}
