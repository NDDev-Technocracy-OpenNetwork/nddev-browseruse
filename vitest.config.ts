import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    pool: "threads",
    include: ["packages/**/test/**/*.test.{ts,tsx}"],
    maxWorkers: 4,
    testTimeout: 10000,
    restoreMocks: true,
  },
});
