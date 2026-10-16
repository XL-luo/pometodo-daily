import { describe, expect, it, vi, afterEach } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { TodoTask } from "../../contracts/todo";
import { getTodoSourceGroups, getVisibleTodoTasks } from "./todoModel";
import { loadTodoViewPreferences, parseTodoViewPreferences, saveTodoViewPreferences, TODO_VIEW_PREFERENCES_KEY } from "./todoViewPreferences";
import SourceList from "./components/SourceList";

const task = (id: string, customerName: string, dueAt: string | null = null): TodoTask => ({
  id, customerName, title: `任务-${id}`, note: "", status: "pending", receivedAt: "2026-09-29T00:00:00Z",
  dueAt, completedAt: null, attachmentPaths: [],
});

describe("来源展示分组", () => {
  it("去除首尾空格归组，中文排序，空来源置底且不与同名标签混淆", () => {
    const tasks = [task("1", "张三"), task("2", " 阿里 "), task("3", "阿里"), task("4", "  "), task("5", "未填写来源")];
    const groups = getTodoSourceGroups(tasks);
    expect(groups.map(group => group.key)).toEqual(["阿里", "未填写来源", "张三", ""]);
    expect(groups[0].tasks.map(item => item.id)).toEqual(["2", "3"]);
    expect(groups.at(-1)?.label).toBe("未填写来源");
    expect(groups.flatMap(group => group.tasks)).toHaveLength(5);
    expect(tasks[1].customerName).toBe(" 阿里 ");
  });
  it("保留现有任务排序，并正确反映来源修改和删除", () => {
    const tasks = getVisibleTodoTasks([task("later", "来源", "2026-10-02"), task("early", "来源", "2026-09-28")]);
    expect(getTodoSourceGroups(tasks)[0].tasks.map(item => item.id)).toEqual(["early", "later"]);
    expect(getTodoSourceGroups(tasks.map(item => item.id === "early" ? { ...item, customerName: "其他" } : item))).toHaveLength(2);
    expect(getTodoSourceGroups(tasks.filter(item => item.id !== "early"))[0].tasks).toHaveLength(1);
    expect(getTodoSourceGroups([])).toEqual([]);
    expect(getTodoSourceGroups([task("empty", "")], "未填写主题")[0].label).toBe("未填写主题");
  });
});

describe("来源列表显示", () => {
  const tasks = [...Array.from({ length: 12 }, (_, i) => task(`a-${i}`, "阿里")), task("z", "张三")];
  const render = (overrides: Partial<Parameters<typeof SourceList>[0]> = {}) => renderToStaticMarkup(<SourceList
    groups={getTodoSourceGroups(tasks)} collapsedSources={[]} searching={false} onToggle={() => {}}
    renderTask={item => <article key={item.id} data-task-id={item.id}>{item.title}</article>}
    focusTaskId={null} listRef={{ current: null }} {...overrides} />);
  it("每组独立显示十项，后续来源标题和数量不被截断", () => {
    const html = render();
    expect(html).toContain("12 项");
    expect(html).toContain("剩余 2 项");
    expect(html).not.toContain('data-task-id="a-10"');
    expect(html).toContain('data-task-id="z"');
    expect(html).toContain('aria-expanded="true"');
    expect(html).toContain("aria-controls=");
  });
  it("折叠组不渲染卡片，搜索临时展开并只显示命中数量", () => {
    expect(render({ collapsedSources: ["阿里"] })).not.toContain('data-task-id="a-0"');
    const html = render({ collapsedSources: ["阿里"], searching: true, groups: getTodoSourceGroups([tasks[11]]) });
    expect(html).toContain('data-task-id="a-11"');
    expect(html).toContain("1 项");
  });
  it("定位超过首批的任务时直接包含目标", () => {
    expect(render({ focusTaskId: "a-11" })).toContain('data-task-id="a-11"');
  });
  it("只有归档时仍保留入口与空态", () => {
    const html = render({ groups: [], summary: <button>归档 3</button> });
    expect(html).toContain("归档 3");
    expect(html).toContain("暂无");
  });
});

describe("本机分组偏好", () => {
  afterEach(() => vi.unstubAllGlobals());
  it("缺失或损坏存储回退默认状态，过滤非法内容", () => {
    for (const raw of [null, "{", "null", "42"]) expect(parseTodoViewPreferences(raw)).toEqual({ mode: "default", collapsedSources: [] });
    expect(parseTodoViewPreferences('{"mode":"bad","collapsedSources":["",123,"阿里","阿里"]}')).toEqual({ mode: "default", collapsedSources: ["", "阿里"] });
  });
  it("保存后重载恢复模式与来源折叠，空来源和特殊名称安全保留", () => {
    const storage = new Map<string, string>();
    vi.stubGlobal("window", { localStorage: { getItem: (key: string) => storage.get(key) ?? null, setItem: (key: string, value: string) => storage.set(key, value) } });
    const prefs = { mode: "source" as const, collapsedSources: ["", "__proto__", "阿里"] };
    saveTodoViewPreferences(prefs);
    expect(storage.has(TODO_VIEW_PREFERENCES_KEY)).toBe(true);
    expect(loadTodoViewPreferences()).toEqual(prefs);
  });
  it("存储不可访问时不影响列表操作", () => {
    vi.stubGlobal("window", { get localStorage() { throw new Error("denied"); } });
    expect(loadTodoViewPreferences()).toEqual({ mode: "default", collapsedSources: [] });
    expect(() => saveTodoViewPreferences({ mode: "source", collapsedSources: [] })).not.toThrow();
  });
});
