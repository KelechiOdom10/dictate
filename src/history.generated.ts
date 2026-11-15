export type TakeStatus = "pasted" | "copied" | "skipped" | "failed";
export type HistoryEntry = {
  id: number;
  text: string;
  createdAtMs: number;
  app: string | null;
  model: string;
  audioMs: number;
  inferenceMs: number | null;
  totalMs: number;
  status: TakeStatus;
  detail: string | null;
};
export type HistoryList = { entries: Array<HistoryEntry>; hasMore: boolean };
