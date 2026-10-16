import { useEffect, useId, useRef, useState, type CSSProperties, type FocusEvent, type MouseEvent } from "react";
import { createPortal } from "react-dom";
import type { TodoTask } from "../../../contracts/todo";
import type { TodoDueInfo, TodoWorkflowAction } from "../todoModel";
import {
  dueWordsFor,
  formatTodoReceivedLabel,
  formatTodoPlanLabel,
  getTodoWorkflowActions,
  shouldShowTodoNote,
} from "../todoModel";
import { todoTexts } from "../todoTexts";
import { wordPackFor, type TodoWordPack } from "../wordPacks";
import { todoIcons } from "./TodoIcons";
import { getTodoTooltipPosition, TodoTooltipTimer, type TodoTooltipRect } from "./todoTooltip";

function stopCardAction(event: MouseEvent) {
  event.preventDefault();
  event.stopPropagation();
}

function workflowActionIcon(action: TodoWorkflowAction) {
  if (action === "defer") return todoIcons.delay;
  if (action === "start") return todoIcons.play;
  if (action === "rollback" || action === "restore") return todoIcons.undo;
  return todoIcons.check;
}

function workflowActionLabel(action: TodoWorkflowAction) {
  if (action === "defer") return todoTexts.defer;
  if (action === "start") return todoTexts.start;
  if (action === "rollback") return todoTexts.rollback;
  if (action === "restore") return todoTexts.restore;
  return todoTexts.complete;
}

function isPrimaryWorkflowAction(action: TodoWorkflowAction) {
  return action === "start" || action === "complete" || action === "restore";
}

function formatCompletedLabel(value: string | null | undefined, cancelled: boolean) {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  const formatted = new Intl.DateTimeFormat("zh-CN", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
  return `${cancelled ? "取消于" : "完成于"} ${formatted}`;
}

interface TodoCardProps {
  task: TodoTask;
  enterIndex: number;
  dueInfo: TodoDueInfo | null;
  todayContext?: boolean;
  archived?: boolean;
  highlighted: boolean;
  busy: boolean;
  wordPack?: TodoWordPack;
  onOpenEdit: (task: TodoTask) => void;
  onSetUrgent: (task: TodoTask, urgent: boolean) => void;
  onCancelTask: (task: TodoTask) => void;
  onOpenDeferPicker: (task: TodoTask) => void;
  onWorkflowAction: (task: TodoTask, action: TodoWorkflowAction) => void;
  onOpenAttachment: (path: string) => void;
}

export default function TodoCard({
  task,
  enterIndex,
  dueInfo,
  todayContext,
  archived = false,
  highlighted,
  busy,
  wordPack = wordPackFor(undefined),
  onOpenEdit,
  onSetUrgent,
  onCancelTask,
  onOpenDeferPicker,
  onWorkflowAction,
  onOpenAttachment,
}: TodoCardProps) {
  const [activeTooltip, setActiveTooltip] = useState<{ actionId: string; label: string; rect: TodoTooltipRect } | null>(null);
  const tooltipId = `todo-action-tooltip-${useId().replace(/:/g, "")}`;
  const tooltipTimer = useRef<TodoTooltipTimer<{ actionId: string; label: string; rect: TodoTooltipRect }> | null>(null);
  if (!tooltipTimer.current) {
    tooltipTimer.current = new TodoTooltipTimer((value) => setActiveTooltip(value), () => setActiveTooltip(null));
  }
  useEffect(() => () => tooltipTimer.current?.dispose(), []);
  useEffect(() => {
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") tooltipTimer.current?.dismiss();
    };
    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, []);
  useEffect(() => {
    if (activeTooltip?.actionId === "urgent") {
      const label = task.urgentAt ? todoTexts.unmarkUrgent : todoTexts.markUrgent;
      if (activeTooltip.label !== label) setActiveTooltip({ ...activeTooltip, label });
    }
  }, [activeTooltip, task.urgentAt]);
  const tooltipProps = (actionId: string, label: string) => ({
    "aria-describedby": activeTooltip?.actionId === actionId ? tooltipId : undefined,
    onMouseEnter: (event: MouseEvent<HTMLButtonElement>) => {
      const bounds = event.currentTarget.getBoundingClientRect();
      const rect: TodoTooltipRect = { left: bounds.left, right: bounds.right, top: bounds.top, bottom: bounds.bottom, width: bounds.width, height: bounds.height };
      tooltipTimer.current?.enter({ actionId, label, rect });
    },
    onMouseLeave: () => tooltipTimer.current?.leavePointer(),
    onFocus: (event: FocusEvent<HTMLButtonElement>) => {
      const bounds = event.currentTarget.getBoundingClientRect();
      const rect: TodoTooltipRect = { left: bounds.left, right: bounds.right, top: bounds.top, bottom: bounds.bottom, width: bounds.width, height: bounds.height };
      tooltipTimer.current?.enter({ actionId, label, rect }, true);
    },
    onBlur: () => tooltipTimer.current?.dismiss(),
  });
  const hasAttachments = task.attachmentPaths.length > 0;
  const workflowActions = archived ? [] : getTodoWorkflowActions(task);
  const dueWords = dueWordsFor(wordPack.dueWord);
  const todayLabel = todayContext && task.dueAt ? dueInfo?.tone === "overdue" ? "逾期" : task.status === "in_progress" ? "进行中" : dueInfo?.label === dueWords.today ? dueWords.expiringToday : null : null;
  const sourceName = task.customerName || wordPack.noCustomer;

  return (
    <>
      <article
      className={`notification todo-card status-${task.status}${task.urgentAt ? " urgent" : ""}${dueInfo ? ` tone-${dueInfo.tone}` : ""}${highlighted ? " floating-highlight" : ""}${busy ? " busy" : ""}`}
      data-task-id={task.id}
      style={{ "--enter-delay": enterIndex } as CSSProperties}
      onClick={() => onOpenEdit(task)}
    >
      <div className="notibar" />
      <div className="noticontent">
        <div className="notititle todo-card-header">
          <div className="todo-card-source">
            {sourceName && <span className="todo-card-person">{sourceName}</span>}
            <span className="todo-card-received">{sourceName ? "· " : ""}{formatTodoReceivedLabel(task.receivedAt)}</span>
          </div>
          {todayLabel && <span className={`todo-card-status-pill ${todayLabel === "逾期" ? "overdue" : todayLabel === "进行中" ? "upcoming" : "today"}`}><span className="todo-control-label">{todayLabel}</span></span>}
          {task.status === "completed" && task.cancelledAt && <span className="todo-card-cancelled-badge" title="此任务已取消"><span className="todo-control-label">已取消</span></span>}
          {task.status === "completed" && !task.cancelledAt && <span className="todo-card-completed-badge" title="此任务已完成"><span className="todo-control-label">已完成</span></span>}
          {task.urgentAt && (
            <button
              className="todo-card-urgent-badge"
              type="button"
              disabled={busy}
              onClick={(event) => {
                stopCardAction(event);
                onSetUrgent(task, false);
              }}
              title={todoTexts.unmarkUrgent}
              aria-label={todoTexts.unmarkUrgent}
            >
              <span className="todo-control-label">{todoTexts.urgent}</span>
            </button>
          )}
        </div>
        <div className="notibody todo-card-body">
          <div className="todo-card-title">{task.title}</div>
          {shouldShowTodoNote(task) && <div className="todo-card-note">{task.note}</div>}
        </div>
        <div className="notititle todo-card-footer">
          <div className="todo-card-meta">
            {task.status === "completed" && (task.cancelledAt || task.completedAt) && <span className={`todo-card-completion-meta${task.cancelledAt ? " cancelled" : ""}`}>{formatCompletedLabel(task.cancelledAt ?? task.completedAt, Boolean(task.cancelledAt))}</span>}
            {dueInfo && (
            <button
              className={`todo-card-due-badge${task.status === "completed" ? " readonly" : ""}`}
              type="button"
              disabled={busy}
              onClick={(event) => {
                stopCardAction(event);
                if (task.status !== "completed") onOpenDeferPicker(task);
              }}
              title={task.status === "completed" ? dueInfo.label : todoTexts.defer}
              aria-label={task.status === "completed" ? dueInfo.label : todoTexts.defer}
            >
              {todoIcons.calendar}<span className="todo-control-label">{dueInfo.label}</span>
            </button>
          )}
            {hasAttachments && (
              <button
                className="todo-card-attachment-badge"
                type="button"
                onClick={(event) => { stopCardAction(event); onOpenAttachment(task.attachmentPaths[0]); }}
                title={todoTexts.viewScreenshots}
                aria-label={`查看 ${task.attachmentPaths.length} 张截图`}
              >
                {todoIcons.image}<span className="todo-control-label">查看 {task.attachmentPaths.length} 张截图</span>
              </button>
            )}
            {formatTodoPlanLabel(task.plannedStartAt, task.dueAt) && <span className="todo-card-plan-meta">{formatTodoPlanLabel(task.plannedStartAt, task.dueAt)}</span>}
          </div>
          <div className="todo-card-actions">
            {task.status !== "completed" && (
              <button
                className={`todo-workflow-action compact-action todo-urgent-action${task.urgentAt ? " urgent" : ""}`}
                type="button"
                disabled={busy}
                onClick={(event) => {
                  tooltipTimer.current?.dismiss();
                  stopCardAction(event);
                  onSetUrgent(task, !task.urgentAt);
                }}
                aria-label={task.urgentAt ? todoTexts.unmarkUrgent : todoTexts.markUrgent}
                {...tooltipProps("urgent", task.urgentAt ? todoTexts.unmarkUrgent : todoTexts.markUrgent)}
              >
                {todoIcons.flag}
                <span className="todo-control-label">{task.urgentAt ? todoTexts.unmarkUrgent : todoTexts.markUrgent}</span>
              </button>
            )}
            {task.status !== "completed" && (
              <button
                className="todo-workflow-action compact-action cancel-task"
                type="button"
                disabled={busy}
                onClick={(event) => {
                  tooltipTimer.current?.dismiss();
                  stopCardAction(event);
                  onCancelTask(task);
                }}
                aria-label={todoTexts.cancelTask}
                {...tooltipProps("cancel", todoTexts.cancelTask)}
              >
                {todoIcons.cancel}
                <span className="todo-control-label">{todoTexts.cancelTask}</span>
              </button>
            )}
            {workflowActions.map((action) => (
              <button
                key={action}
                className={`todo-workflow-action ${action} ${isPrimaryWorkflowAction(action) ? "primary-action" : "compact-action"}`}
                type="button"
                disabled={busy}
                onClick={(event) => {
                  tooltipTimer.current?.dismiss();
                  stopCardAction(event);
                  onWorkflowAction(task, action);
                }}
                aria-label={workflowActionLabel(action)}
                {...tooltipProps(action, workflowActionLabel(action))}
              >
                {workflowActionIcon(action)}
                <span className="todo-control-label">{workflowActionLabel(action)}</span>
              </button>
            ))}
          </div>
        </div>
      </div>
      </article>
      {activeTooltip && typeof document !== "undefined" && (() => {
        const position = getTodoTooltipPosition(activeTooltip.rect, window.innerWidth, window.innerHeight);
        return createPortal(
          <div
            id={tooltipId}
            className={`todo-action-tooltip ${position.placement === "below" ? "is-below" : ""}`}
            role="tooltip"
            style={{ left: position.left, top: position.top }}
          >
            {activeTooltip.label}
          </div>,
          document.body,
        );
      })()}
    </>
  );
}
