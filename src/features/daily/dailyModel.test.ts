import { describe, expect, it } from "vitest";
import { dailyTaskCandidates, taskReasons } from "./dailyModel";
import type { TodoTask } from "../../contracts/todo";

describe("daily model", () => {
  it("selects completed, progressing and deadline-related tasks", () => {
    const task = (id: string, patch: Partial<TodoTask>): TodoTask => ({
      id, customerName: "", title: id, note: "", status: "pending", receivedAt: "2026-09-20T09:00:00+08:00",
      dueAt: null, urgentAt: null, completedAt: null, attachmentPaths: [], ...patch,
    });
    const tasks = [
      task("done", { status: "completed", completedAt: "2026-09-28T18:00:00+08:00" }),
      task("cancelled", { status: "completed", completedAt: "2026-09-28T18:30:00+08:00", cancelledAt: "2026-09-28T18:30:00+08:00" }),
      task("active", { status: "in_progress" }),
      task("tomorrow", { dueAt: "2026-09-29T18:00:00+08:00" }),
      task("later", { dueAt: "2026-10-10T18:00:00+08:00" }),
    ];
    expect(dailyTaskCandidates(tasks, "2026-09-28").map((task) => task.id)).toEqual(["done", "active", "tomorrow"]);
    expect(taskReasons(tasks[3], "2026-09-28")).toContain("明日截止");
  });

  it("includes tasks whose selected date is inside the planned range", () => {
    const task: TodoTask = {
      id: "planned", customerName: "", title: "planned", note: "", status: "pending",
      receivedAt: "2026-09-01T09:00:00+08:00", plannedStartAt: "2026-09-27T09:00:00+08:00",
      dueAt: "2026-09-30T18:00:00+08:00", completedAt: null, attachmentPaths: [],
    };
    expect(taskReasons(task, "2026-09-28")).toContain("计划期内");
    expect(dailyTaskCandidates([task], "2026-09-28")).toEqual([task]);
    expect(dailyTaskCandidates([task], "2026-09-26")).toEqual([]);
    expect(taskReasons(task, "2026-09-27")).toContain("计划期内");
    expect(taskReasons(task, "2026-09-30")).toContain("计划期内");
    expect(taskReasons(task, "2026-10-01")).not.toContain("计划期内");
    expect(taskReasons({ ...task, dueAt: null }, "2026-10-01")).toContain("计划期内");
    expect(taskReasons(task, "2026-09-28")).not.toContain("进行中");
    expect(dailyTaskCandidates([{ ...task, cancelledAt: "2026-09-28T08:00:00+08:00" }], "2026-09-28")).toEqual([]);
  });
});
