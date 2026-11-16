export type SpeechModel =
  "tiny.en" | "base.en" | "small.en" | "medium.en" | "large-v3-turbo";
export type ModelState =
  | { kind: "available" }
  | {
      kind: "downloading";
      received: number;
      total: number;
      cancelling: boolean;
    }
  | { kind: "preparing" }
  | { kind: "installed" }
  | { kind: "failed"; message: string };
export type ModelEntry = {
  id: SpeechModel;
  name: string;
  description: string;
  bytes: number;
  accuracy: number;
  measuredMs: number;
  recommended: boolean;
  active: boolean;
  state: ModelState;
};
