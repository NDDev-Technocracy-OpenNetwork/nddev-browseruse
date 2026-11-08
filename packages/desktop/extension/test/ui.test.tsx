import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeAll, expect, test, vi } from "vitest";
import { Analytics } from "../src/features/group/Analytics.js";
import { Activity } from "../src/features/popup/activity.js";
import { Popup } from "../src/features/popup/Popup.js";
import { translations } from "../src/features/shared/i18n.js";
import { LanguageSwitcher } from "../src/features/shared/LanguageSwitcher.js";
import type { FrontendConfig } from "../src/generated/types.gen.js";

beforeAll(async () => {
  const catalog = JSON.parse(
    readFileSync(new URL("../_locales/en/messages.json", import.meta.url), "utf8"),
  ) as Record<string, { message: string }>;
  const dictionary = Object.fromEntries(
    Object.entries(catalog).map(([key, value]) => [key, value.message]),
  );
  await translations.init({
    lng: "en",
    resources: { en: { translation: dictionary } },
    interpolation: { escapeValue: false },
  });
});
afterEach(() => vi.unstubAllGlobals());

test("Off removes collected analytics from the rendered owner view", () => {
  const metrics = { completed: 347, failed: 19, totalMs: 123000, lastMs: 17000 };
  const off = renderToStaticMarkup(<Analytics metrics={metrics} observation="off" />);
  expect(off).toContain('id="metrics"');
  expect(off).toContain('hidden=""');
  expect(off).not.toContain("347");
  expect(off).not.toContain("2:03");
  const visible = renderToStaticMarkup(<Analytics metrics={metrics} observation="standard" />);
  expect(visible).toContain('id="operations">347</strong>');
  expect(visible).toContain('id="total">2:03</strong>');
});

test("activity text is escaped and repeated records remain separate", () => {
  const line = JSON.stringify({
    type: "action",
    text: '<img src=x onerror="unsafe()">',
    private: true,
  });
  const html = renderToStaticMarkup(<Activity lines={[line, line, "invalid record"]} />);
  expect(html).not.toContain("<img src=x");
  expect(html).toContain("&lt;img");
  expect(html.match(/data-type="action"/g)).toHaveLength(2);
  expect(html).toContain("invalid record");
});

test("the language control names all five languages in their native language", () => {
  const html = renderToStaticMarkup(<LanguageSwitcher />);
  for (const text of ["English", "Русский", "Қазақша", "العربية", "简体中文"])
    expect(html).toContain(text);
  expect(html).toContain('lang="kk"');
  expect(html).toContain('aria-pressed="true"');
  const compact = renderToStaticMarkup(<LanguageSwitcher compact />);
  expect(compact.match(/<option /g)).toHaveLength(5);
  for (const text of ["English", "Русский", "Қазақша", "العربية", "简体中文"])
    expect(compact).toContain(text);
});

test("the popup grants no control before the owner state is read", () => {
  vi.stubGlobal("location", new URL("chrome-extension://fixture/popup.html"));
  vi.stubGlobal("chrome", {
    runtime: {
      getManifest: () => ({ version: "fixture" }),
      getURL: (path: string) => `chrome-extension://fixture/${path}`,
    },
  });
  const config = JSON.parse(
    readFileSync(new URL("../public-settings.json", import.meta.url), "utf8"),
  ) as FrontendConfig;
  const html = renderToStaticMarkup(<Popup config={config} />);
  expect(html).toMatch(/id="toggle"[^>]*aria-checked="false"[^>]*disabled=""/);
  expect(html).toMatch(/data-consent-profile="full"[^>]*disabled=""/);
});
