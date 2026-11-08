import { existsSync, lstatSync, mkdtempSync, readdirSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
// Validate the staged candidate itself: all executor imports resolve and production packages
// remain inside its directory after a move or removal of the global Bun cache.
import { expect, test } from "vitest";

const repository = join(dirname(fileURLToPath(import.meta.url)), "../../..");

test.skipIf(process.platform === "win32")(
  "the installed executor carries every module it imports",
  () => {
    const root = mkdtempSync(join(tmpdir(), "browseruse-stage-"));
    const stage = join(root, "candidate");
    try {
      const staged = Bun.spawnSync(
        [join(repository, ".tools/bin/nddev-browseruse-tools"), "stage", stage, "debug"],
        { cwd: repository },
      );
      expect(staged.stderr.toString()).not.toContain("browseruse tools:");
      expect(staged.exitCode).toBe(0);
      // Bundling resolves every relative import; packages come from the installed dependencies.
      const built = Bun.spawnSync(
        [
          process.execPath,
          "build",
          join(stage, "executor/executor.ts"),
          "--target",
          "bun",
          "--packages",
          "external",
          "--outdir",
          join(stage, "out"),
        ],
        { cwd: stage },
      );
      expect(built.stderr.toString()).not.toContain("Could not resolve");
      expect(built.exitCode).toBe(0);
      expect(existsSync(join(stage, "executor/test"))).toBe(false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  },
);

// Bun's isolated linker links packages into its global cache by absolute path. An install made
// that way breaks once that cache is cleaned, and on every other device it is copied to.
test.skipIf(process.platform === "win32")(
  "the installed dependencies stay inside the install",
  () => {
    const area = mkdtempSync(join(tmpdir(), "browseruse-stage-"));
    const stage = join(area, "candidate");
    try {
      const installed = Bun.spawnSync(
        [join(repository, ".tools/bin/nddev-browseruse-tools"), "stage", stage, "debug"],
        { cwd: repository },
      );
      expect(installed.stderr.toString()).not.toContain("browseruse tools:");
      expect(installed.exitCode).toBe(0);
      const root = realpathSync(stage);
      const outside: string[] = [];
      const walk = (directory: string): void => {
        for (const name of readdirSync(directory)) {
          const path = join(directory, name);
          const entry = lstatSync(path);
          if (entry.isSymbolicLink()) {
            const target = existsSync(path) ? realpathSync(path) : "";
            if (!target.startsWith(`${root}/`)) outside.push(path.slice(stage.length));
          } else if (entry.isDirectory()) {
            walk(path);
          }
        }
      };
      walk(join(stage, "node_modules"));
      expect(outside).toEqual([]);
      expect(existsSync(join(stage, "node_modules/playwright-core/package.json"))).toBe(true);
    } finally {
      rmSync(area, { recursive: true, force: true });
    }
  },
);
