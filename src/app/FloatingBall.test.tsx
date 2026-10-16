import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { FloatingBall, getFloatingTaskStatusLabel, getFloatingTasks } from "./FloatingBall";
import type { TodoTask } from "../contracts/todo";

const task = (id: string, overrides: Partial<TodoTask> = {}): TodoTask => ({
  id, customerName: "虚构客户", title: "测试任务", note: "", status: "pending",
  receivedAt: "2026-09-06T00:00:00Z", dueAt: null, completedAt: null,
  attachmentPaths: [], updatedAt: "2026-09-06T00:00:00Z", ...overrides,
});

describe("旧版浮球行为", () => {
  it("球始终独立，预览使用另一个窗口以免缩放拖动留下旧位图", () => {
    const html = renderToStaticMarkup(<FloatingBall />);
    const preview = renderToStaticMarkup(<FloatingBall previewOnly />);
    expect(html).toContain("pome-floating-ball");
    expect(html).not.toContain("pome-float-panel");
    expect(preview).toContain("pome-float-panel");
    expect(preview).not.toContain("pome-floating-ball-row");
    expect(preview).toContain("查看全部");
    expect(html).toContain("待办</span>");
  });
  it("加急任务优先；同为加急时按最近加急时间、交付日期、状态和更新时间排序", () => {
    const rows = [task("none"), task("done", { status: "completed" }),
      task("later", { dueAt: "2026-09-08T00:00:00Z", urgentAt: "2026-09-06T00:00:00Z" }),
      task("urgent-new", { dueAt: null, urgentAt: "2026-09-06T02:00:00Z" }),
      task("urgent-old", { dueAt: "2026-09-06T00:00:00Z", urgentAt: "2026-09-06T01:00:00Z" }),
      task("pending", { dueAt: "2026-09-07T00:00:00Z" }),
      task("progress-old", { status: "in_progress", dueAt: "2026-09-07T00:00:00Z" }),
      task("progress-new", { status: "in_progress", dueAt: "2026-09-07T00:00:00Z", updatedAt: "2026-09-06T01:00:00Z" })];
    expect(getFloatingTasks(rows).map(row => row.id)).toEqual(["urgent-new", "urgent-old", "later", "progress-new", "progress-old", "pending", "none"]);
    expect(rows[0].id).toBe("none");
  });
  it("加急状态在悬停和无障碍文本中同时说明优先级与状态", () => {
    expect(getFloatingTaskStatusLabel(task("urgent-pending", { urgentAt: "2026-09-06T00:00:00Z" }))).toBe("加急 · 未完成");
    expect(getFloatingTaskStatusLabel(task("urgent-progress", { status: "in_progress", urgentAt: "2026-09-06T00:00:00Z" }))).toBe("加急 · 进行中");
    expect(getFloatingTaskStatusLabel(task("normal", { status: "in_progress" }))).toBe("进行中");
  });
  it("未设日期时仍按进行中、更新时间排序，保留全部计数", () => {
    const rows = Array.from({ length: 103 }, (_, i) => task(String(i)));
    rows.push(task("progress", { status: "in_progress" }));
    expect(getFloatingTasks(rows)).toHaveLength(104);
    expect(getFloatingTasks(rows)[0].id).toBe("progress");
  });
  it("更新时间按实际时刻排序，不按时区字符串字面排序", () => {
    const rows = [task("earlier", { updatedAt: "2026-09-06T10:00:00+08:00" }),
      task("later", { updatedAt: "2026-09-06T03:00:00Z" })];
    expect(getFloatingTasks(rows).map(row => row.id)).toEqual(["later", "earlier"]);
  });
});
