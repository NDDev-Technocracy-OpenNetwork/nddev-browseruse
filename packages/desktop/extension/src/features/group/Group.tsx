import { useCallback, useEffect, useRef, useState } from "react";
import type { FrontendConfig, GroupDashboard } from "../../generated/types.gen.js";
import { useLocale } from "../shared/i18n.js";
import { LanguageSwitcher } from "../shared/LanguageSwitcher.js";
import { Analytics } from "./Analytics.js";
import { duration } from "./format.js";

export function Group({ config }: { config: FrontendConfig }) {
  const { t, language } = useLocale();
  const [current, setCurrent] = useState<{ value: GroupDashboard; at: number }>();
  const [now, setNow] = useState(Date.now());
  const [ending, setEnding] = useState(false);
  const gate = useRef({ ending: false, pending: false, mounted: true, revision: 0 });
  const id = location.hash.slice(1);
  const refresh = useCallback(async (): Promise<void> => {
    const held = gate.current;
    if (held.pending || held.ending) return;
    held.pending = true;
    const revision = held.revision;
    try {
      const value = (await chrome.runtime.sendMessage({
        type: "group",
        action: "status",
        id,
      })) as GroupDashboard;
      if (held.mounted && revision === held.revision) setCurrent({ value, at: Date.now() });
    } catch {
      if (held.mounted && revision === held.revision)
        setCurrent(
          (previous) =>
            previous && {
              ...previous,
              value: { ...previous.value, connected: false, error: "request_failed" },
            },
        );
    } finally {
      held.pending = false;
    }
  }, [id]);
  useEffect(() => {
    gate.current.mounted = true;
    void refresh();
    const poll = setInterval(() => void refresh(), config.refreshMs);
    const tick = setInterval(() => setNow(Date.now()), config.lifecycle.sweepIntervalMs);
    return () => {
      gate.current.mounted = false;
      clearInterval(poll);
      clearInterval(tick);
    };
  }, [config.refreshMs, config.lifecycle.sweepIntervalMs, refresh]);

  async function finish(keep: boolean): Promise<void> {
    if (gate.current.ending) return;
    gate.current.ending = true;
    gate.current.revision++;
    setEnding(true);
    try {
      await chrome.runtime.sendMessage({ type: "group", action: keep ? "keep" : "close", id });
    } catch {
      // The control tab may have closed its own response channel. Do not replay the write.
    } finally {
      gate.current.ending = false;
      if (gate.current.mounted) {
        setEnding(false);
        await refresh();
      }
    }
  }
  const state = current?.value.status;
  const phase = !current
    ? "starting"
    : !current.value.identity
      ? "unavailable"
      : !current.value.connected
        ? "down"
        : (state?.phase ?? "starting");
  useEffect(() => {
    document.body.dataset.phase = phase;
  }, [phase]);
  const phases = {
    starting: "groupStarting",
    ready: "groupReady",
    running: "groupRunning",
    recording: "groupRecording",
    closing: "groupClosing",
    unavailable: "groupUnavailable",
    down: "groupLinkDown",
  };
  const elapsed = current ? Math.max(0, now - current.at) : 0;
  const number = new Intl.NumberFormat(language);
  return (
    <main>
      <header>
        <div className="brand">
          <img src="icons/icon-48.png" alt="" />
          <span>{t("title")}</span>
        </div>
        <LanguageSwitcher title="groupTitle" />
      </header>
      <section className="workspace" aria-labelledby="workspace-title">
        <div className="workspace-heading">
          <div>
            <p className="eyebrow">{t("groupTitle")}</p>
            <h1 id="workspace-title">{current?.value.identity?.project ?? t("groupTitle")}</h1>
          </div>
          <span id="phase" className="pill" role="status" aria-live="polite">
            {t(phases[phase])}
          </span>
        </div>
        <p className="description">{t("groupDescription")}</p>
        <div className="summary">
          <div>
            <span>{t("groupUptime")}</span>
            <strong id="uptime">{state ? duration(state.uptimeMs + elapsed) : "—"}</strong>
          </div>
          <div>
            <span>{t("groupTabs")}</span>
            <strong id="tabs">{state ? number.format(state.siteTabs) : "—"}</strong>
          </div>
          <div className="deadline">
            <span id="cleanup">
              {!state
                ? t("groupLinkDown")
                : state.phase === "running" || state.phase === "recording"
                  ? t("groupBusyCleanup")
                  : t(
                      "groupIdle",
                      duration(Math.max(0, state.idleTimeoutMs - state.idleMs - elapsed)),
                    )}
            </span>
          </div>
        </div>
      </section>
      <Analytics
        metrics={state?.metrics ?? null}
        observation={current?.value.observation ?? "off"}
      />
      <p id="privacy" className="note">
        {t(current?.value.observation === "off" ? "groupPrivacyOff" : "groupPrivacyMetadata")}
      </p>
      <section className="controls" aria-labelledby="controls-title">
        <h2 id="controls-title">{t("groupControls")}</h2>
        <p>{t("groupStopHint")}</p>
        <div className="actions">
          <button
            id="finish"
            type="button"
            className="nd-button"
            disabled={ending || !state || state.phase === "closing"}
            onClick={() => void finish(false)}
          >
            {t("groupClose")}
          </button>
          <button
            id="keep"
            type="button"
            className="nd-button nd-button--secondary"
            disabled={ending || !state || state.phase === "closing"}
            onClick={() => void finish(true)}
          >
            {t("groupKeep")}
          </button>
        </div>
        <p className="note">{t("groupKeepHint")}</p>
        <p id="error" role="status" hidden={!current?.value.error}>
          {current?.value.error ? t("groupRequestFailed") : ""}
        </p>
      </section>
      <footer>
        <span>v{chrome.runtime.getManifest().version}</span>
        <a
          id="licenses-link"
          target="_blank"
          rel="noreferrer"
          href={chrome.runtime.getURL(`licenses/${language}.html`)}
        >
          {t("licenses")}
        </a>
      </footer>
    </main>
  );
}
