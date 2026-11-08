import { useLocale } from "../shared/i18n.js";

const kinds: Record<string, string> = {
  action: "kindAction",
  navigation: "kindNavigation",
  page: "kindPage",
  network: "kindNetwork",
  execution: "kindExecution",
  download: "kindDownload",
  console: "kindConsole",
  dialog: "kindDialog",
  "page-error": "kindPageError",
  recording: "kindRecording",
  "person-action": "kindPersonAction",
  toast: "kindToast",
};

function readable(locator: string): string {
  return locator
    .replace(/internal:role=([a-z]+)\[name="((?:[^"\\]|\\.)*)"[is]?\]/g, '$1 "$2"')
    .replace(
      /internal:(?:text|label|attr=\[placeholder|testid=\[data-testid)=?"((?:[^"\\]|\\.)*)"[is]?\]?/g,
      '"$1"',
    )
    .replace(/internal:role=([a-z]+)/g, "$1");
}

export function Activity({ lines }: { lines: string[] }) {
  const { t } = useLocale();
  const occurrences = new Map<string, number>();
  function ago(at: unknown): string {
    const time = typeof at === "string" ? Date.parse(at) : Number.NaN;
    if (Number.isNaN(time)) return "";
    const seconds = Math.max(0, Math.round((Date.now() - time) / 1000));
    if (seconds < 5) return t("ageNow");
    const [value, unit] =
      seconds < 60
        ? [seconds, "second"]
        : seconds < 3600
          ? [Math.round(seconds / 60), "minute"]
          : seconds < 86400
            ? [Math.round(seconds / 3600), "hour"]
            : [Math.round(seconds / 86400), "day"];
    return t("ageUnit", value, unit);
  }
  return (
    <ol id="recent" className="steps">
      {lines
        .slice()
        .reverse()
        .map((line) => {
          const occurrence = occurrences.get(line) ?? 0;
          occurrences.set(line, occurrence + 1);
          let record: Record<string, unknown>;
          try {
            record = JSON.parse(line) as Record<string, unknown>;
          } catch {
            record = { type: "other", text: line };
          }
          const type = typeof record.type === "string" ? record.type : "other";
          const summary = [
            record.api,
            record.action,
            record.method,
            typeof record.locator === "string" ? readable(record.locator) : undefined,
            record.url,
            record.text,
            record.name,
            record.message,
          ]
            .filter((part) => typeof part === "string" && part !== "")
            .join(" ");
          const text = record.private === true ? `${t("private")} ${summary}`.trim() : summary;
          return (
            <li
              key={`${occurrence}:${line}`}
              data-type={type}
              data-private={String(record.private === true)}
              data-error={String(
                record.ok === false ||
                  typeof record.error === "string" ||
                  typeof record.failure === "string",
              )}
            >
              <span className="when">{ago(record.at)}</span>
              <span className="kind">{t(kinds[type] ?? "kindOther")}</span>
              <span className="what" title={text}>
                {text}
              </span>
            </li>
          );
        })}
    </ol>
  );
}
