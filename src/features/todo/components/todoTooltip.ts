export const TODO_TOOLTIP_DELAY_MS = 100;
export const TODO_TOOLTIP_ADJACENT_WINDOW_MS = 260;

export interface TodoTooltipRect {
  left: number;
  right: number;
  top: number;
  bottom: number;
  width: number;
  height: number;
}

export interface TodoTooltipPosition {
  left: number;
  top: number;
  placement: "above" | "below";
}

/** Keeps tooltip timing consistent for pointer hover and keyboard focus. */
export class TodoTooltipTimer<T> {
  private timer: ReturnType<typeof setTimeout> | null = null;
  private lastVisibleAt = Number.NEGATIVE_INFINITY;

  constructor(
    private readonly show: (value: T) => void,
    private readonly hide: () => void,
    private readonly delay = TODO_TOOLTIP_DELAY_MS,
    private readonly adjacentWindow = TODO_TOOLTIP_ADJACENT_WINDOW_MS,
  ) {}

  enter(value: T, immediate = false) {
    this.clearTimer();
    const now = Date.now();
    if (immediate || now - this.lastVisibleAt <= this.adjacentWindow) {
      this.lastVisibleAt = now;
      this.show(value);
      return;
    }
    this.timer = setTimeout(() => {
      this.timer = null;
      this.lastVisibleAt = Date.now();
      this.show(value);
    }, this.delay);
  }

  leavePointer() {
    this.clearTimer();
    this.hide();
  }

  dismiss() {
    this.clearTimer();
    this.lastVisibleAt = Number.NEGATIVE_INFINITY;
    this.hide();
  }

  dispose() {
    this.dismiss();
  }

  private clearTimer() {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }
}

export function getTodoTooltipPosition(
  rect: TodoTooltipRect,
  viewportWidth: number,
  viewportHeight: number,
): TodoTooltipPosition {
  const estimatedWidth = 180;
  const edge = 8;
  const gap = 8;
  const minCenter = edge + estimatedWidth / 2;
  const maxCenter = Math.max(minCenter, viewportWidth - edge - estimatedWidth / 2);
  const left = Math.min(maxCenter, Math.max(minCenter, rect.left + rect.width / 2));
  const placement = rect.top >= 42 ? "above" : "below";
  const top = placement === "above" ? rect.top - gap : Math.min(viewportHeight - edge, rect.bottom + gap);
  return { left, top, placement };
}
