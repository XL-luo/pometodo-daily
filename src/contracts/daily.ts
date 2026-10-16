import type { TodoTask } from "./todo";

export type DailySourceStatus = "pending" | "processed" | "failed";

export interface DailySource {
  id: string;
  workDate: string;
  text: string;
  attachmentPaths: string[];
  status: DailySourceStatus;
  error: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface DailyReport {
  workDate: string;
  content: string;
  generatedAt: string;
  updatedAt: string;
}

export interface DailySnapshot {
  sources: DailySource[];
  report: DailyReport | null;
}

export interface DailyGenerationResult {
  kind: "ai" | "local";
  content: string;
  message: string;
}

export interface DailyRecognitionResult {
  kind: "ai" | "ocr";
  items: string[];
  message: string;
}

export interface DailyServices {
  snapshot(workDate: string): Promise<DailySnapshot>;
  createSource(draft: { workDate: string; text: string; attachmentPaths: string[] }): Promise<DailySource>;
  createSources(drafts: Array<{ workDate: string; text: string; attachmentPaths: string[] }>): Promise<DailySource[]>;
  updateSource(id: string, draft: { text: string }): Promise<DailySource>;
  deleteSource(id: string): Promise<void>;
  recognizeAttachment(id: string): Promise<DailyRecognitionResult>;
  saveClipboardAttachment(): Promise<string | null>;
  getAttachmentDataUrl(path: string): Promise<string>;
  saveReport(workDate: string, content: string): Promise<DailyReport>;
  generate(workDate: string, selectedTaskIds: string[], supplement: string): Promise<DailyGenerationResult>;
  listTasks(): Promise<TodoTask[]>;
}
