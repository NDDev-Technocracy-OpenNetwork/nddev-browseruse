import type { FrontendConfig, GroupDashboard } from "./generated/types.gen.js";
import { loadLanguage, selectLanguage, t } from "./ui.js";

const id = location.hash.slice(1);
const element = (name: string) => document.getElementById(name) as HTMLElement;
let current: GroupDashboard | undefined;
let received = 0;
let pending = false;
let ending = false;

function duration(ms: number): string {
  const seconds = Math.max(0, Math.ceil(ms / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function render(): void {
  const state = current?.status;
  const phase = !current?.identity
    ? "unavailable"
    : !current.connected
      ? "down"
      : (state?.phase ?? "starting");
  document.body.dataset.phase = phase;
  element("workspace-title").textContent = current?.identity?.project ?? t("groupTitle");
  const phases = {
    starting: "groupStarting",
    ready: "groupReady",
    running: "groupRunning",
    recording: "groupRecording",
    closing: "groupClosing",
    unavailable: "groupUnavailable",
    down: "groupLinkDown",
  };
  element("phase").textContent = t(phases[phase]);
  const elapsed = state ? Date.now() - received : 0;
  element("uptime").textContent = state ? duration(state.uptimeMs + elapsed) : "—";
  element("tabs").textContent = state
    ? new Intl.NumberFormat(document.documentElement.lang).format(state.siteTabs)
    : "—";
  element("cleanup").textContent = !state
    ? t("groupLinkDown")
    : state.phase === "running" || state.phase === "recording"
      ? t("groupBusyCleanup")
      : t("groupIdle", duration(Math.max(0, state.idleTimeoutMs - state.idleMs - elapsed)));
  const metrics = state?.metrics;
  element("metrics").hidden = !metrics;
  const number = new Intl.NumberFormat(document.documentElement.lang);
  element("operations").textContent = metrics ? number.format(metrics.completed) : "—";
  element("failures").textContent = metrics ? number.format(metrics.failed) : "—";
  element("total").textContent = metrics ? duration(metrics.totalMs) : "—";
  element("last").textContent =
    metrics?.lastMs === null || metrics?.lastMs === undefined ? "—" : duration(metrics.lastMs);
  element("privacy").textContent = t(
    current?.observation === "off" ? "groupPrivacyOff" : "groupPrivacyMetadata",
  );
  for (const name of ["finish", "keep"])
    (element(name) as HTMLButtonElement).disabled = ending || !state || state.phase === "closing";
  const failure = element("error");
  failure.hidden = !current?.error;
  failure.textContent = current?.error ? t("groupRequestFailed") : "";
}

async function refresh(): Promise<void> {
  if (pending || ending) return;
  pending = true;
  try {
    current = (await chrome.runtime.sendMessage({
      type: "group",
      action: "status",
      id,
    })) as GroupDashboard;
    received = Date.now();
  } catch {
    if (current) current = { ...current, connected: false, error: "request_failed" };
  } finally {
    pending = false;
    render();
  }
}

async function finish(keep: boolean): Promise<void> {
  ending = true;
  render();
  try {
    await chrome.runtime.sendMessage({ type: "group", action: keep ? "keep" : "close", id });
  } catch {
    // Closing the control tab can end its own response channel. A surviving page re-reads state;
    // it never blindly retries a possibly completed owner action.
  } finally {
    ending = false;
    // If the page survived (for example after a refused request), read the actual state.
    // Never leave its controls stuck or repeat a possibly completed write.
    await refresh();
  }
}

async function start(): Promise<void> {
  await loadLanguage("groupTitle");
  element("version").textContent = `v${chrome.runtime.getManifest().version}`;
  const config = (await fetch(chrome.runtime.getURL("public-settings.json")).then((response) =>
    response.json(),
  )) as FrontendConfig;
  for (const button of document.querySelectorAll<HTMLButtonElement>("button[data-language]"))
    button.addEventListener("click", () => {
      void selectLanguage(button.dataset.language, "groupTitle").then(render);
    });
  element("finish").addEventListener("click", () => {
    void finish(false);
  });
  element("keep").addEventListener("click", () => {
    void finish(true);
  });
  await refresh();
  setInterval(() => {
    void refresh();
  }, config.refreshMs);
  setInterval(render, 1000);
}

void start().catch(() => {
  element("phase").textContent = chrome.i18n.getMessage("groupUnavailable");
});
