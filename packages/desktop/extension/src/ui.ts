import i18next from "i18next";
import ICU from "i18next-icu";

const languages = ["en", "ru", "kk", "ar", "zh-CN"] as const;
const translations = i18next.createInstance().use(ICU);
const LANGUAGE = "popupLanguage";

export function t(key: string, ...values: (string | number)[]): string {
  return String(
    translations.t(key, Object.fromEntries(values.map((value, index) => [`v${index + 1}`, value]))),
  );
}

export async function selectLanguage(value: string | undefined, title = "title"): Promise<void> {
  if (!languages.some((language) => language === value)) return;
  await chrome.storage.local.set({ [LANGUAGE]: value });
  await loadLanguage(title);
}

export async function loadLanguage(title = "title"): Promise<void> {
  const stored: unknown = (await chrome.storage.local.get(LANGUAGE))[LANGUAGE];
  const ui = chrome.i18n.getUILanguage().toLowerCase();
  const language =
    languages.find((locale) => locale === stored) ??
    languages.find(
      (locale) => ui === locale.toLowerCase() || ui.startsWith(`${locale.toLowerCase()}-`),
    ) ??
    (ui.startsWith("zh") ? "zh-CN" : "en");
  const messages = (await fetch(
    chrome.runtime.getURL(`_locales/${language.replace("-", "_")}/messages.json`),
  ).then((response) => response.json())) as Record<string, { message: string }>;
  const dictionary = Object.fromEntries(
    Object.entries(messages).map(([key, value]) => [key, value.message]),
  );
  if (!translations.isInitialized)
    await translations.init({
      lng: language,
      fallbackLng: "en",
      resources: { [language]: { translation: dictionary } },
      interpolation: { escapeValue: false },
    });
  else {
    translations.addResourceBundle(language, "translation", dictionary, true, true);
    await translations.changeLanguage(language);
  }
  document.documentElement.lang = language;
  document.documentElement.dir = language === "ar" ? "rtl" : "ltr";
  const dark = matchMedia("(prefers-color-scheme: dark)").matches;
  const contrast = matchMedia("(prefers-contrast: more)").matches;
  document.documentElement.dataset.theme = `${dark ? "dark" : "light"}${contrast ? "-contrast" : ""}`;
  document.title = t(title);
  const legal = document.getElementById("licenses-link") as HTMLAnchorElement | null;
  if (legal) legal.href = chrome.runtime.getURL(`licenses/${language}.html`);
  for (const node of document.querySelectorAll<HTMLElement>("[data-i18n]"))
    node.textContent = t(node.dataset.i18n ?? "");
  for (const node of document.querySelectorAll<HTMLElement>("[data-i18n-aria]"))
    node.setAttribute("aria-label", t(node.dataset.i18nAria ?? ""));
  for (const button of document.querySelectorAll<HTMLButtonElement>("button[data-language]"))
    button.setAttribute("aria-pressed", String(button.dataset.language === language));
}
