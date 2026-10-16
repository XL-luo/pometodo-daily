import { describe, expect, it, vi } from "vitest";
import { getTodoTooltipPosition, TodoTooltipTimer } from "./todoTooltip";

describe("TodoTooltipTimer", () => {
  it("waits 100ms on first hover and switches adjacent icons immediately", () => {
    vi.useFakeTimers();
    const shown: string[] = [];
    const timer = new TodoTooltipTimer((value: string) => shown.push(value), () => undefined);

    timer.enter("加急");
    expect(shown).toEqual([]);
    vi.advanceTimersByTime(99);
    expect(shown).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(shown).toEqual(["加急"]);

    timer.enter("取消任务");
    expect(shown).toEqual(["加急", "取消任务"]);
    timer.dismiss();
    vi.useRealTimers();
  });

  it("focus can show immediately and dismiss clears a pending timer", () => {
    vi.useFakeTimers();
    const shown: string[] = [];
    const timer = new TodoTooltipTimer((value: string) => shown.push(value), () => undefined);
    timer.enter("完成", true);
    expect(shown).toEqual(["完成"]);
    timer.dismiss();
    timer.enter("恢复");
    timer.dismiss();
    vi.advanceTimersByTime(200);
    expect(shown).toEqual(["完成"]);
    vi.useRealTimers();
  });
});

describe("getTodoTooltipPosition", () => {
  it("keeps the tooltip inside the viewport and flips below near the top edge", () => {
    expect(getTodoTooltipPosition({ left: 0, right: 28, top: 4, bottom: 32, width: 28, height: 28 }, 400, 300)).toEqual({
      left: 98,
      top: 40,
      placement: "below",
    });
    expect(getTodoTooltipPosition({ left: 390, right: 418, top: 100, bottom: 128, width: 28, height: 28 }, 400, 300).left).toBe(302);
  });
});
