import type { GroupMetrics, Observation } from "../../generated/types.gen.js";
import { useLocale } from "../shared/i18n.js";
import { duration } from "./format.js";

export function Analytics({
  metrics: collected,
  observation,
}: {
  metrics: GroupMetrics | null;
  observation: Observation;
}) {
  const { t, language } = useLocale();
  const metrics = observation === "off" ? null : collected;
  const number = new Intl.NumberFormat(language);
  return (
    <section id="metrics" className="metrics" aria-label={t("groupAnalytics")} hidden={!metrics}>
      <article>
        <h2>{t("groupOperations")}</h2>
        <strong id="operations">{metrics ? number.format(metrics.completed) : "—"}</strong>
      </article>
      <article>
        <h2>{t("groupFailures")}</h2>
        <strong id="failures">{metrics ? number.format(metrics.failed) : "—"}</strong>
      </article>
      <article>
        <h2>{t("groupTotalTime")}</h2>
        <strong id="total">{metrics ? duration(metrics.totalMs) : "—"}</strong>
      </article>
      <article>
        <h2>{t("groupLastTime")}</h2>
        <strong id="last">{metrics?.lastMs == null ? "—" : duration(metrics.lastMs)}</strong>
      </article>
    </section>
  );
}
