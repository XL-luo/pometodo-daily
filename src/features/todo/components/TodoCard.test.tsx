import { describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { TodoTask } from "../../../contracts/todo";
import TodoCard from "./TodoCard";

const task = (urgentAt: string | null): TodoTask => ({
  id: "task-1",
  customerName: "示例客户",
  title: "示例待办",
  note: "",
  status: "pending",
  receivedAt: "2026-09-30T00:00:00Z",
  dueAt: null,
  urgentAt,
  completedAt: null,
  attachmentPaths: [],
});

function renderCard(current: TodoTask, onSetUrgent = vi.fn()) {
  return renderToStaticMarkup(
    <TodoCard
      task={current}
      enterIndex={0}
      dueInfo={null}
      highlighted={false}
      busy={false}
      onOpenEdit={vi.fn()}
      onSetUrgent={onSetUrgent}
      onCancelTask={vi.fn()}
      onOpenDeferPicker={vi.fn()}
      onWorkflowAction={vi.fn()}
      onOpenAttachment={vi.fn()}
    />,
  );
}

describe("TodoCard 加急操作", () => {
  it("按当前状态显示加急或取消加急入口", () => {
    const pending = renderCard(task(null));
    expect(pending).toContain('aria-label="加急"');
    expect(pending).toContain("compact-action");
    expect(pending).toContain("primary-action");
    expect(renderCard(task("2026-09-30T01:00:00Z"))).toContain('aria-label="取消加急"');
  });

  it("所有未完成任务显示取消任务，已完成任务不显示", () => {
    expect(renderCard(task(null))).toContain('aria-label="取消任务"');
    expect(renderCard(task("2026-09-30T01:00:00Z"))).toContain('aria-label="取消任务"');
    expect(renderCard({ ...task(null), status: "in_progress" })).toContain('aria-label="取消任务"');
    expect(renderCard({ ...task(null), status: "completed", completedAt: "2026-09-30T02:00:00Z" })).not.toContain('aria-label="取消任务"');
  });

  it("区分取消完成与正常完成，并保留恢复入口", () => {
    const cancelled = renderCard({ ...task(null), status: "completed", completedAt: "2026-09-30T02:00:00Z", cancelledAt: "2026-09-30T02:00:00Z" });
    expect(cancelled).toContain("已取消");
    expect(cancelled).toContain("取消于");
    expect(cancelled).toContain('aria-label="恢复"');
    const completed = renderCard({ ...task(null), status: "completed", completedAt: "2026-09-30T02:00:00Z" });
    expect(completed).toContain("已完成");
    expect(completed).toContain("完成于");
    expect(completed).not.toContain("已取消");
  });

});
