import i18next from "i18next";
import ICU from "i18next-icu";
import { initReactI18next, useTranslation } from "react-i18next";
import { watchTheme } from "./theme.js";

const languages = ["en", "ru", "kk", "ar", "zh-CN"] as const;
export const translations = i18next.createInstance().use(ICU).use(initReactI18next);
const LANGUAGE = "popupLanguage";

function t(key: string, ...values: (string | number)[]): string {
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
  watchTheme();
  document.title = t(title);
}

export function useLocale() {
  const { i18n } = useTranslation(undefined, { i18n: translations });
  return { t, language: i18n.language };
}
