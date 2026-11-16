import { z } from "zod";
import type { Problem } from "./problem.generated";
export type { Problem } from "./problem.generated";

export const problemSchema = z
  .object({
    title: z.string(),
    fix: z.string(),
    detail: z.string().nullable(),
  })
  .strict() satisfies z.ZodType<Problem>;

export const problem = (
  title: string,
  fix: string,
  cause?: unknown,
): Problem => ({
  title,
  fix,
  detail: cause === undefined ? null : String(cause),
});
