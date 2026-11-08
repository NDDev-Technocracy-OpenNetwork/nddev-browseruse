import { useState } from "react";
import { selectLanguage, useLocale } from "./i18n.js";

const labels = { en: "English", ru: "Русский", kk: "Қазақша", ar: "العربية", "zh-CN": "简体中文" };

export function LanguageSwitcher({
  title = "title",
  compact = false,
}: {
  title?: string;
  compact?: boolean;
}) {
  const { t, language } = useLocale();
  const [failed, setFailed] = useState(false);
  function choose(value: string): void {
    setFailed(false);
    void selectLanguage(value, title).catch(() => setFailed(true));
  }
  return (
    <fieldset className="languages" aria-label={t("languageTitle")}>
      {compact ? (
        <select
          data-language="select"
          aria-label={t("languageTitle")}
          value={language}
          onChange={(event) => choose(event.currentTarget.value)}
        >
          {Object.entries(labels).map(([locale, label]) => (
            <option key={locale} value={locale} lang={locale}>
              {label}
            </option>
          ))}
        </select>
      ) : (
        Object.entries(labels).map(([locale, label]) => (
          <button
            type="button"
            key={locale}
            lang={locale}
            data-language={locale}
            aria-pressed={locale === language}
            onClick={() => choose(locale)}
          >
            {label}
          </button>
        ))
      )}
      {failed && <span role="alert">{t("groupRequestFailed")}</span>}
    </fieldset>
  );
}
