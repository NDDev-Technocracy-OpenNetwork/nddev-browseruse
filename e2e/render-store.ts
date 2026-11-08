// Thin browser rendering boundary. Rust owns copy, dimensions, inputs and provenance.
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { launchFixture } from "./launch.js";

const plan = JSON.parse(readFileSync(process.argv[2] ?? "", "utf8")) as {
  chrome: string;
  frames: { html: string; png: string; width: number; height: number }[];
};
const profile = mkdtempSync(join(tmpdir(), "nddev-store-render-"));
const browser = await launchFixture(profile, {
  executablePath: plan.chrome,
  headless: true,
  viewport: { width: 1280, height: 800 },
  deviceScaleFactor: 1,
});
try {
  await browser.route(/^https?:/, (route) => route.abort());
  const page = await browser.newPage();
  for (const frame of plan.frames) {
    await page.setViewportSize({ width: frame.width, height: frame.height });
    await page.goto(pathToFileURL(frame.html).href);
    await page.evaluate(async () => {
      await document.fonts.ready;
      await Promise.all(Array.from(document.images, (image) => image.decode()));
    });
    if (
      await page.evaluate(() =>
        Array.from(document.querySelectorAll(".copy,.capture"), (node) => {
          const bounds = node.getBoundingClientRect();
          return (
            bounds.left < 0 ||
            bounds.right > innerWidth ||
            bounds.top < 0 ||
            bounds.bottom > innerHeight
          );
        }).some(Boolean),
      )
    )
      throw new Error(`store artwork overflows: ${frame.png}`);
    await page.screenshot({ path: frame.png, type: "png" });
  }
} finally {
  await browser.close();
  rmSync(profile, { recursive: true, force: true });
}
