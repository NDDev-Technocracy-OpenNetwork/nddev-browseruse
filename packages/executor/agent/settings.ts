import type { ExecutorSettings } from "../generated/types.gen.js";
import values from "../settings.json" with { type: "json" };

// Only the generated public-to-executor projection is shipped; it contains no private paths.
export const executorSettings: ExecutorSettings = values;

/** Fixed for this executor lifetime. The host validates all units and bounds at startup. */
export function configure(settings: ExecutorSettings | undefined): boolean {
  if (!settings || typeof settings !== "object") return false;
  for (const name of ["execution", "activity", "recorder"] as const) {
    const incoming = settings[name];
    const expected = executorSettings[name];
    if (
      !incoming ||
      typeof incoming !== "object" ||
      Object.keys(incoming).length !== Object.keys(expected).length ||
      !Object.keys(expected).every((key) => {
        const value = (incoming as Record<string, unknown>)[key];
        return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
      })
    )
      return false;
  }
  Object.assign(executorSettings.execution, settings.execution);
  Object.assign(executorSettings.activity, settings.activity);
  Object.assign(executorSettings.recorder, settings.recorder);
  return true;
}
