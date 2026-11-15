export type AnnotationEntry = {
  /**
   * The folder name, `2026-11-14_14-56-36`.
   */
  id: string;
  /**
   * RFC 3339, UTC.
   */
  startedAt: string;
  durationMs: number;
  app: string | null;
  frames: number;
  marks: number;
  text: string;
  /**
   * Absolute path of the folder, for display.
   */
  folder: string;
  hasContactSheet: boolean;
  packagingMs: number | null;
};
