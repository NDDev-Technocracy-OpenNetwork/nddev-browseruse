import { useCallback, useEffect, useRef, useState } from "react";
import type {
  BrowserStatus,
  FrontendConfig,
  Observation,
  OwnerRecent,
  OwnerRecording,
} from "../../generated/types.gen.js";
import { useLocale } from "../shared/i18n.js";
import { LanguageSwitcher } from "../shared/LanguageSwitcher.js";
import { Activity } from "./activity.js";

type Recent = OwnerRecent;
const descriptions = {
  full: "observationFull",
  standard: "observationStandard",
  off: "observationOff",
};
const profiles: Observation[] = ["full", "standard", "off"];
const refused: Record<string, string> = {
  no_agent_tab: "refusedNoAgentTab",
  session_not_found: "refusedSession",
  busy: "refusedBusy",
  permission_required: "refusedPermission",
  extension_unavailable: "refusedHost",
  timeout: "refusedTimeout",
};

export function Popup({ config }: { config: FrontendConfig }) {
  const { t, language } = useLocale();
  const [status, setStatus] = useState<BrowserStatus>();
  const [recording, setRecording] = useState<OwnerRecording>({});
  const [recent, setRecent] = useState<Recent>({});
  const [busy, setBusy] = useState(false);
  const gate = useRef({ mounted: true, pending: false, writing: false, revision: 0 });
  const latest = useRef({ status, recording });
  useEffect(() => {
    latest.current = { status, recording };
  }, [status, recording]);
  const forTab = Number(new URLSearchParams(location.search).get("tab")) || undefined;

  const apply = useCallback((value: BrowserStatus) => {
    if (typeof value.enabled !== "boolean" || typeof value.connected !== "boolean")
      throw new Error("invalid_owner_state");
    setStatus(value);
    if (value.observation !== "full") {
      setRecent({});
      setRecording({});
    }
  }, []);

  const refresh = useCallback(
    async (initial = false): Promise<void> => {
      const held = gate.current;
      if (held.pending || held.writing) return;
      held.pending = true;
      const revision = held.revision;
      try {
        const value = (await chrome.runtime.sendMessage({
          type: "get",
          tab: forTab,
        })) as BrowserStatus;
        if (!held.mounted || revision !== held.revision) return;
        apply(value);
        if (value.consentRequired || !value.connected) return;
        const [steps, record] = await Promise.all([
          chrome.runtime.sendMessage({ type: "recent", tab: forTab }) as Promise<Recent>,
          initial || latest.current.recording.recording
            ? (chrome.runtime.sendMessage({
                type: "record",
                action: "status",
                tab: forTab,
              }) as Promise<OwnerRecording>)
            : Promise.resolve(undefined),
        ]);
        if (held.mounted && revision === held.revision) {
          setRecent(value.observation === "off" ? {} : steps);
          if (record) {
            latest.current.recording = record;
            setRecording(record);
          }
        }
      } catch {
        if (held.mounted && revision === held.revision)
          setStatus((previous) => previous && { ...previous, connected: false });
      } finally {
        held.pending = false;
      }
    },
    [apply, forTab],
  );

  useEffect(() => {
    gate.current.mounted = true;
    void refresh(true);
    const timer = setInterval(() => void refresh(), config.refreshMs);
    return () => {
      gate.current.mounted = false;
      clearInterval(timer);
    };
  }, [config.refreshMs, refresh]);

  async function change(message: object, record = false): Promise<void> {
    const held = gate.current;
    if (held.writing) return;
    held.writing = true;
    held.revision++;
    setBusy(true);
    try {
      const value: unknown = await chrome.runtime.sendMessage({ ...message, tab: forTab });
      if (held.mounted) {
        if (record) {
          latest.current.recording = value as OwnerRecording;
          setRecording(value as OwnerRecording);
        } else apply(value as BrowserStatus);
      }
    } catch {
      // Lost replies never trigger a blind repeat of a write.
      if (held.mounted) setStatus((previous) => previous && { ...previous, connected: false });
    } finally {
      held.writing = false;
      if (held.mounted) {
        setBusy(false);
        await refresh();
      }
    }
  }
  const mismatch = status?.hostContract !== undefined || status?.hostUpdateRequired === true;
  const link = mismatch ? "mismatch" : !status?.connected ? "down" : status.enabled ? "ok" : "off";
  const consent = status?.consentRequired ?? true;
  const profile = status?.observation ?? "standard";
  const agentTab = Boolean(status?.group);
  const activeRecording = recording.recording === true;
  useEffect(() => {
    Object.assign(document.body.dataset, {
      link,
      compatible: String(!mismatch),
      consent: String(consent),
      observation: profile,
      agentTab: String(agentTab),
      recording: recording.error ? "error" : String(activeRecording),
      recent: String(recent.lines?.length ?? 0),
    });
  }, [
    link,
    mismatch,
    consent,
    profile,
    agentTab,
    recording.error,
    activeRecording,
    recent.lines?.length,
  ]);
  const sites = status?.privateSites ?? [];
  const site = status?.site;
  const ownVersion = status?.version ?? chrome.runtime.getManifest().version;
  const recordingNote = recording.error
    ? t(refused[recording.error] ?? "refusedHost")
    : activeRecording
      ? t("recordingNow", recording.events ?? 0)
      : recording.id
        ? t("recordedDone", recording.events ?? 0)
        : "";
  const privateChange = (action: string, value: string) =>
    change({ type: "private", action, site: value });
  return (
    <>
      <header>
        <img src="icons/icon-32.png" alt="" />
        <h1>{t("title")}</h1>
        <span className="pill" role="status" aria-live="polite">
          <span className="dot" />
          <span id="link">
            {t({ ok: "linkOk", off: "linkOff", down: "linkDown", mismatch: "linkMismatch" }[link])}
          </span>
        </span>
      </header>
      <p id="mismatch" className="notice" hidden={!mismatch}>
        {mismatch
          ? t(status?.hostUpdateRequired ? "hostUpdate" : "mismatch", config.minimumHost)
          : ""}
      </p>
      <p id="setup" className="notice setup" hidden={link !== "down"}>
        <span>{t("setupNeeded")}</span>
        <a id="install-link" target="_blank" rel="noreferrer" href={config.installUrl}>
          {t("setupLink")}
        </a>
      </p>
      <section id="consent" aria-labelledby="consent-title" hidden={!consent}>
        <h2 id="consent-title">{t("consentTitle")}</h2>
        <p>{t("consentPurpose")}</p>
        <p>{t("consentData")}</p>
        <p id="previous-choice" className="muted">
          {t("previousChoice", t(profile))}
        </p>
        <div className="consent-options">
          {profiles.map((value) => (
            <button
              key={value}
              className="nd-button nd-button--secondary"
              data-consent-profile={value}
              type="button"
              disabled={busy || !status}
              onClick={() => void change({ type: "accept", profile: value })}
            >
              {t({ full: "useFull", standard: "useStandard", off: "useOff" }[value])}
            </button>
          ))}
        </div>
        <a id="consent-privacy" target="_blank" rel="noreferrer" href={config.privacyUrl}>
          {t("privacyPolicy")}
        </a>
      </section>
      <section aria-labelledby="control-title">
        <div className="row">
          <div className="grow">
            <div id="control-title" className="strong">
              {t("controlTitle")}
            </div>
            <p id="control-note" className="muted">
              {status?.enabled
                ? `${t("controlOn")}${status.tabs ? ` ${t("agentTabs", status.tabs)}` : ""}`
                : t("controlOff")}
            </p>
          </div>
          <button
            id="toggle"
            type="button"
            className="switch"
            role="switch"
            aria-checked={status?.enabled ?? false}
            aria-labelledby="control-title"
            disabled={busy || consent || !status}
            onClick={() => void change({ type: "set", enabled: !status?.enabled })}
          />
        </div>
        <hr className="divider" />
        <div className="observe">
          <h2 id="observation-title">{t("observationTitle")}</h2>
          <fieldset className="segments" aria-labelledby="observation-title">
            {profiles.map((value) => (
              <button
                key={value}
                type="button"
                data-profile={value}
                aria-pressed={value === profile}
                disabled={busy || consent || !status}
                onClick={() => void change({ type: "observe", profile: value })}
              >
                {t(value)}
              </button>
            ))}
          </fieldset>
        </div>
        <p id="observation" className="muted">
          {t(descriptions[profile])}
        </p>
      </section>
      <section aria-labelledby="tab-title">
        <h2 id="tab-title">{t("tabTitle")}</h2>
        <div className="row">
          <div id="tab-state" className="grow strong">
            {status?.group ? (
              <span className="badge">{t("tabAgent", status.group)}</span>
            ) : (
              t(status?.hasTab === false ? "tabNone" : "tabOwner")
            )}
            {site && <span className="site">{site}</span>}
          </div>
          <button
            id="private-add"
            className="button small nd-button nd-button--secondary"
            type="button"
            title={t("privateNote")}
            disabled={busy || !site || sites.includes(site)}
            onClick={() => site && void privateChange("add", site)}
          >
            {site && sites.includes(site) ? t("privateIs", site) : t("privateAddAny")}
          </button>
        </div>
        <p id="tab-note" className="muted" hidden={agentTab || status?.hasTab === false}>
          {t("tabOwnerNote")}
        </p>
        <div className="actions">
          <button
            id="record"
            className={`button nd-button ${activeRecording ? "recording" : "primary"}`}
            type="button"
            title={t("recordHint")}
            disabled={busy || recording.error === "no_agent_tab"}
            hidden={!agentTab}
            data-recording={String(activeRecording)}
            onClick={() =>
              void change({ type: "record", action: activeRecording ? "stop" : "start" }, true)
            }
          >
            <span className="dot" />
            <span id="record-label">{t(activeRecording ? "stopRecord" : "record")}</span>
          </button>
        </div>
        <p id="recording" className="muted" aria-live="polite" hidden={!agentTab || !recordingNote}>
          {recordingNote}
        </p>
      </section>
      <section id="activity" aria-labelledby="activity-title" hidden={!agentTab}>
        <h2 id="activity-title" title={t("activityHint")}>
          {t("activityTitle")}
        </h2>
        <Activity lines={recent.lines ?? []} />
        <p
          id="recent-note"
          className="muted"
          hidden={!recent.error && (recent.lines?.length ?? 0) > 0}
        >
          {recent.error
            ? t(recent.error === "no_agent_tab" ? "activityNoTab" : "activityHost")
            : t("activityNone")}
        </p>
      </section>
      <section aria-labelledby="private-title">
        <h2 id="private-title" title={t("privateNote")}>
          {t("privateTitle")}
        </h2>
        <p id="private-note" className="muted" hidden={sites.length > 0}>
          {t("privateNote")}
        </p>
        <ul id="private" className="chips" data-count={sites.length} hidden={!sites.length}>
          {sites.map((value) => (
            <li key={value}>
              {value}
              <button
                type="button"
                title={t("privateRemove", value)}
                aria-label={t("privateRemove", value)}
                disabled={busy}
                onClick={() => void privateChange("remove", value)}
              >
                ×
              </button>
            </li>
          ))}
        </ul>
      </section>
      <footer>
        <span
          id="versions"
          title={
            status?.host
              ? t(
                  "versions",
                  ownVersion,
                  `${status.host.version} ${status.host.commit.slice(0, 7)}`,
                )
              : t("versionsNoHost", ownVersion)
          }
        >
          v{ownVersion}
        </span>
        <a id="guide-link" target="_blank" rel="noreferrer" href={config.guideUrl}>
          {t("guide")}
        </a>
        <a
          id="privacy-link"
          target="_blank"
          rel="noreferrer"
          data-i18n="privacyPolicy"
          href={config.privacyUrl}
        >
          {t("privacyPolicy")}
        </a>
        <a
          id="licenses-link"
          target="_blank"
          rel="noreferrer"
          href={chrome.runtime.getURL(`licenses/${language}.html`)}
        >
          {t("licenses")}
        </a>
        <LanguageSwitcher compact />
      </footer>
    </>
  );
}
