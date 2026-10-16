import type { DailyServices } from "../contracts/daily";
import type { InvokeCommand } from "./todoServices";
import { nativeError } from "./todoServices";

export function createDailyServices(invoke: InvokeCommand): DailyServices {
  async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    try { return await invoke<T>(command, args); }
    catch (error) { throw nativeError(error); }
  }
  return {
    snapshot: (workDate) => call("pometodo_daily_snapshot", { workDate }),
    createSource: (draft) => call("pometodo_daily_create_source", { draft }),
    createSources: (drafts) => call("pometodo_daily_create_sources", { drafts }),
    updateSource: (id, draft) => call("pometodo_daily_update_source", { id, draft }),
    deleteSource: (id) => call("pometodo_daily_delete_source", { id }),
    recognizeAttachment: (id) => call("pometodo_daily_recognize_attachment", { id }),
    saveClipboardAttachment: () => call("pometodo_save_clipboard_attachment"),
    getAttachmentDataUrl: (path) => call("pometodo_attachment_data_url", { path }),
    saveReport: (workDate, content) => call("pometodo_daily_save_report", { workDate, content }),
    generate: (workDate, selectedTaskIds, supplement) => call("pometodo_daily_generate", { workDate, selectedTaskIds, supplement }),
    listTasks: () => call("pometodo_list_tasks"),
  };
}
