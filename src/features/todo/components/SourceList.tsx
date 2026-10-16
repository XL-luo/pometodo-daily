import { useEffect, useId, useRef, useState, type ReactNode, type RefObject } from "react";
import { TODO_BATCH_SIZE, type TodoDisplayGroup, type TodoTask } from "../todoModel";
import { todoIcons } from "./TodoIcons";

interface Props {
  groups: TodoDisplayGroup[];
  collapsedSources: string[];
  onToggle: (key: string) => void;
  searching: boolean;
  renderTask: (task: TodoTask, index: number) => ReactNode;
  focusTaskId: string | null;
  listRef: RefObject<HTMLDivElement | null>;
  summary?: ReactNode;
}

function SourceGroup({ group, open, onToggle, renderTask, focusTaskId }: {
  group: TodoDisplayGroup; open: boolean; onToggle: () => void;
  renderTask: Props["renderTask"]; focusTaskId: string | null;
}) {
  const contentId = useId();
  const sectionRef = useRef<HTMLElement>(null);
  const [limit, setLimit] = useState(TODO_BATCH_SIZE);
  const focusIndex = focusTaskId ? group.tasks.findIndex(task => task.id === focusTaskId) : -1;
  const visibleLimit = Math.max(limit, focusIndex + 1);
  useEffect(() => {
    if (focusIndex < 0 || !open) return;
    setLimit(value => Math.max(value, focusIndex + 1));
    const timer = window.setTimeout(() => {
      // 不将原生传入的任务 ID 拼入 CSS 选择器。
      const target = [...(sectionRef.current?.querySelectorAll<HTMLElement>("[data-task-id]") ?? [])]
        .find(element => element.dataset.taskId === focusTaskId);
      target?.scrollIntoView({ block: "center", behavior: "smooth" });
    }, 0);
    return () => window.clearTimeout(timer);
  }, [focusIndex, focusTaskId, open]);

  return <section ref={sectionRef} className={`todo-source-group${open ? " open" : ""}`}>
    <h2 className="todo-source-heading">
      <button type="button" className="todo-completed-toggle todo-source-toggle" aria-expanded={open} aria-controls={contentId} onClick={onToggle}>
        <span className="todo-completed-chevron" aria-hidden="true">›</span>
        <span className="todo-source-name" title={group.label}>{group.label}</span>
        <span className="todo-completed-count">{group.tasks.length} 项</span>
      </button>
    </h2>
    <div id={contentId} hidden={!open} className="todo-source-cards">
      {open && group.tasks.slice(0, visibleLimit).map(renderTask)}
      {open && group.tasks.length > visibleLimit && <button className="clipboard-load-more" type="button"
        onClick={() => setLimit(value => Math.max(value, visibleLimit) + TODO_BATCH_SIZE)}>
        加载更多（剩余 {group.tasks.length - visibleLimit} 项）
      </button>}
    </div>
  </section>;
}

export default function SourceList({ groups, collapsedSources, onToggle, searching, renderTask, focusTaskId, listRef, summary }: Props) {
  // 搜索时的折叠只作用于当前查询，不覆盖用户的日常偏好。
  const [searchCollapsed, setSearchCollapsed] = useState<string[]>([]);
  const collapsed = new Set(searching ? searchCollapsed : collapsedSources);
  return <div className="todo-list todo-source-list" ref={listRef}>
    {summary}
    {!groups.length && <div className="page-empty-compact"><div className="empty-icon-compact">{todoIcons.todo}</div><span>{searching ? "没有找到匹配的任务" : "暂无"}</span></div>}
    {groups.map(group => <SourceGroup key={group.key} group={group}
      open={!collapsed.has(group.key)} focusTaskId={focusTaskId} renderTask={renderTask}
      onToggle={() => {
        if (searching) setSearchCollapsed(keys => keys.includes(group.key) ? keys.filter(key => key !== group.key) : [...keys, group.key]);
        else onToggle(group.key);
      }} />)}
  </div>;
}
