import { readFileSync } from "node:fs";
import i18next from "i18next";
import ICU from "i18next-icu";
import { expect, test } from "vitest";

function catalog(locale: string): Record<string, string> {
  const source = JSON.parse(
    readFileSync(new URL(`../_locales/${locale}/messages.json`, import.meta.url), "utf8"),
  ) as Record<string, { message: string }>;
  return Object.fromEntries(Object.entries(source).map(([key, value]) => [key, value.message]));
}

test("all five catalogs have complete ICU messages, including the disclosure", async () => {
  const english = catalog("en");
  for (const language of ["en", "ru", "kk", "ar", "zh_CN"]) {
    const messages = catalog(language);
    expect(Object.keys(messages).sort()).toEqual(Object.keys(english).sort());
    const translation = i18next.createInstance().use(
      new ICU({
        parseErrorHandler: (error) => {
          throw error;
        },
      }),
    );
    await translation.init({
      lng: language.replace("_", "-"),
      resources: { [language.replace("_", "-")]: { translation: messages } },
    });
    for (const key of Object.keys(messages)) {
      for (const count of [0, 1, 2, 5, 21]) {
        expect(String(translation.t(key, { v1: count, v2: "fixture" }))).not.toBe("");
      }
    }
    expect(messages.consentPurpose).toBeTruthy();
    expect(messages.consentData).toBeTruthy();
    expect(messages.extensionDescription?.length).toBeLessThanOrEqual(132);
  }
});
