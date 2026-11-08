import type { ExecutorSettings } from "../generated/types.gen.js";
import values from "../settings.json" with { type: "json" };

// Only the generated public-to-executor projection is shipped; it contains no private paths.
export const executorSettings: ExecutorSettings = values;
