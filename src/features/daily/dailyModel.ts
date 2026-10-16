import type { TodoTask } from "../../contracts/todo";

export function localDateValue(date = new Date()): string {
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 10);
}

export function taskCompletedOn(task: TodoTask, date: string): boolean {
  return Boolean(task.completedAt && localDateValue(new Date(task.completedAt!)) === date);
}

export function taskDateValue(value: string | null | undefined): string | null {
  return value ? localDateValue(new Date(value)) : null;
}

export type DailyTaskReason = "今日完成" | "进行中" | "今日新增" | "计划期内" | "已到期" | "明日截止";

export function taskReasons(task: TodoTask, workDate: string): DailyTaskReason[] {
  if (task.cancelledAt) return [];
  const selected = new Date(`${workDate}T12:00:00`);
  selected.setDate(selected.getDate() + 1);
  const tomorrow = localDateValue(selected);
  const reasons: DailyTaskReason[] = [];
  if (taskCompletedOn(task, workDate)) reasons.push("今日完成");
  if (task.status === "in_progress") reasons.push("进行中");
  if (taskDateValue(task.receivedAt) === workDate) reasons.push("今日新增");
  const due = taskDateValue(task.dueAt);
  const start = taskDateValue(task.plannedStartAt);
  if (task.status !== "completed" && start && start <= workDate && (!due || due >= workDate)) reasons.push("计划期内");
  if (task.status !== "completed" && due && due < workDate) reasons.push("已到期");
  else if (task.status !== "completed" && due === workDate) reasons.push("已到期");
  else if (task.status !== "completed" && due === tomorrow) reasons.push("明日截止");
  return reasons;
}

export function dailyTaskCandidates(tasks: TodoTask[], workDate: string): TodoTask[] {
  return tasks.filter((task) => taskReasons(task, workDate).length > 0);
}
