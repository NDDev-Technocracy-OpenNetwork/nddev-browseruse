// The owner's control of agents: the switch, the tab the owner looks at (its agent group, the
// recorder, keeping its site private), the agent's newest steps there, what agent activity is
// collected, and the private sites. The worker applies and reports all of it; this page only shows
// it, in the owner's language.
import i18next from "i18next";
import ICU from "i18next-icu";
import type {
  BrowserStatus,
  FrontendConfig,
  Observation,
  OwnerRecording,
} from "./generated/types.gen.js";

type Status = Partial<BrowserStatus> & Pick<BrowserStatus, "enabled" | "connected">;
type Recording = OwnerRecording;
type Language = "en" | "ru" | "kk" | "ar" | "zh-CN";
type Messages = Record<string, { message: string }>;
const LANGUAGES: readonly Language[] = ["en", "ru", "kk", "ar", "zh-CN"];
const translations = i18next.createInstance().use(ICU);
let config: FrontendConfig;

// A popup opened as a page of its own, as in tests, names the tab it acts for.
const forTab = Number(new URLSearchParams(location.search).get("tab")) || undefined;
const LANGUAGE = "popupLanguage";
const element = <T extends HTMLElement>(id: string) => document.getElementById(id) as T | null;

let language: Language = "en";
let messages: Messages = {};
let last: Status | undefined;
let lastRecording: Recording = {};
let lastSteps: { lines?: string[]; error?: string } = {};

function t(key: string, ...values: (string | number)[]): string {
  return String(
    translations.t(key, Object.fromEntries(values.map((value, index) => [`v${index + 1}`, value]))),
  );
}

async function loadLanguage(): Promise<void> {
  const stored = await chrome.storage.local
    .get(LANGUAGE)
    .then((values) => values[LANGUAGE] as unknown)
    .catch(() => undefined);
  const ui = chrome.i18n.getUILanguage().toLowerCase();
  const wanted =
    LANGUAGES.find((locale) => locale === stored) ??
    LANGUAGES.find(
      (locale) => ui === locale.toLowerCase() || ui.startsWith(`${locale.toLowerCase()}-`),
    ) ??
    (ui.startsWith("zh") ? "zh-CN" : "en");
  language = wanted;
  messages = (await fetch(
    chrome.runtime.getURL(`_locales/${language.replace("-", "_")}/messages.json`),
  ).then((response) => response.json())) as Messages;
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
  document.documentElement.dataset.theme = matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
  document.title = t("title");
  for (const node of document.querySelectorAll<HTMLElement>("[data-i18n]")) {
    node.textContent = t(node.dataset.i18n ?? "");
  }
  for (const node of document.querySelectorAll<HTMLElement>("[data-i18n-aria]"))
    node.setAttribute("aria-label", t(node.dataset.i18nAria ?? ""));
  for (const button of document.querySelectorAll<HTMLButtonElement>("button[data-language]")) {
    button.setAttribute("aria-pressed", String(button.dataset.language === language));
  }
}

function onClick(button: HTMLButtonElement, work: () => Promise<void>): void {
  button.addEventListener("click", () => {
    void work().catch(() => {
      // A lost reply does not prove the worker rejected the change. Show the broken link;
      // reopening the popup reads the authoritative state without retrying the action.
      show({
        ...last,
        enabled: last?.enabled ?? false,
        consentRequired: last?.consentRequired ?? true,
        connected: false,
      });
    });
  });
}

for (const button of document.querySelectorAll<HTMLButtonElement>("button[data-language]")) {
  onClick(button, async () => {
    await chrome.storage.local.set({ [LANGUAGE]: button.dataset.language }).catch(() => undefined);
    await loadLanguage();
    render();
  });
}

const DESCRIPTIONS: Record<Observation, string> = {
  full: "observationFull",
  standard: "observationStandard",
  off: "observationOff",
};

function show(status: Status): void {
  if (typeof status.enabled !== "boolean" || typeof status.connected !== "boolean")
    throw new Error("configuration_unavailable");
  last = status;
  render();
}

function render(): void {
  if (last) renderStatus(last);
  renderRecording(lastRecording);
  renderSteps(lastSteps);
}

function renderStatus(status: Status): void {
  const mismatch = status.hostContract !== undefined || status.hostUpdateRequired === true;
  const link = mismatch ? "mismatch" : !status.connected ? "down" : status.enabled ? "ok" : "off";
  document.body.dataset.link = link;
  document.body.dataset.compatible = String(!mismatch);
  const linkText = element("link");
  if (linkText) {
    linkText.textContent = t(
      { ok: "linkOk", off: "linkOff", down: "linkDown", mismatch: "linkMismatch" }[link],
    );
  }
  const notice = element("mismatch");
  if (notice) {
    notice.hidden = !mismatch;
    notice.textContent = mismatch
      ? t(status.hostUpdateRequired ? "hostUpdate" : "mismatch", config.minimumHost)
      : "";
  }
  // Without the host nothing works: a store install tells the owner where to get it.
  const setup = element("setup");
  if (setup) setup.hidden = link !== "down";
  const toggle = element<HTMLButtonElement>("toggle");
  if (toggle) {
    toggle.setAttribute("aria-checked", String(status.enabled));
    toggle.disabled = status.consentRequired === true;
  }
  const control = element("control-note");
  if (control) {
    control.textContent = status.enabled
      ? `${t("controlOn")}${status.tabs ? ` ${t("agentTabs", status.tabs)}` : ""}`
      : t("controlOff");
  }
  const current = status.observation ?? "standard";
  for (const button of profiles) {
    button.setAttribute("aria-pressed", String(button.dataset.profile === current));
    button.disabled = status.consentRequired === true;
  }
  const described = element("observation");
  if (described) described.textContent = t(DESCRIPTIONS[current]);
  document.body.dataset.observation = current;
  document.body.dataset.consent = String(status.consentRequired === true);
  const consent = element("consent");
  if (consent) consent.hidden = status.consentRequired !== true;
  const previous = element("previous-choice");
  if (previous) previous.textContent = t("previousChoice", t(current));
  renderTab(status);
  renderPrivate(status);
  const versions = element("versions");
  if (versions) {
    const own = status.version ?? chrome.runtime.getManifest().version;
    // The footer keeps one line: the extension's version, both builds in its tooltip.
    versions.textContent = `v${own}`;
    versions.title = status.host
      ? t("versions", own, `${status.host.version} ${status.host.commit.slice(0, 7)}`)
      : t("versionsNoHost", own);
  }
}

function renderTab(status: Status): void {
  const state = element("tab-state");
  const note = element("tab-note");
  if (!state || !note) return;
  state.replaceChildren();
  if (status.group) {
    const badge = document.createElement("span");
    badge.className = "badge";
    badge.textContent = t("tabAgent", status.group);
    state.append(badge);
    // The record button and its tooltip say what the owner can do here.
    note.textContent = "";
  } else {
    state.textContent = status.hasTab === false ? t("tabNone") : t("tabOwner");
    note.textContent = status.hasTab === false ? "" : t("tabOwnerNote");
  }
  note.hidden = note.textContent === "";
  if (status.site) {
    const site = document.createElement("span");
    site.className = "site";
    site.textContent = status.site;
    state.append(site);
  }
  // Recording and the agent's steps belong to agent tabs; elsewhere they would only explain why
  // they do not work.
  const agentTab = Boolean(status.group);
  document.body.dataset.agentTab = String(agentTab);
  for (const id of ["record", "recording", "activity"]) {
    const part = element(id);
    if (part) part.hidden = !agentTab;
  }
}

// Private sites: the owner adds the site of the tab and removes any listed one.
function renderPrivate(status: Status): void {
  const list = element("private");
  const add = element<HTMLButtonElement>("private-add");
  if (!list || !add) return;
  const sites = status.privateSites ?? [];
  list.replaceChildren(
    ...sites.map((site) => {
      const item = document.createElement("li");
      item.textContent = site;
      const remove = document.createElement("button");
      remove.type = "button";
      remove.textContent = "×";
      remove.title = t("privateRemove", site);
      remove.setAttribute("aria-label", t("privateRemove", site));
      onClick(remove, () => changePrivate("remove", site));
      item.append(remove);
      return item;
    }),
  );
  list.hidden = sites.length === 0;
  const note = element("private-note");
  if (note) note.hidden = sites.length > 0;
  const title = element("private-title");
  if (title) title.title = t("privateNote");
  list.dataset.count = String(sites.length);
  const site = status.site;
  add.disabled = !site || sites.includes(site);
  add.dataset.site = site ?? "";
  // The tab's site is named beside its group already; the button keeps to one line.
  add.textContent = site && sites.includes(site) ? t("privateIs", site) : t("privateAddAny");
  add.title = t("privateNote");
}

async function changePrivate(action: "add" | "remove", site: string): Promise<void> {
  show(
    (await chrome.runtime.sendMessage({ type: "private", action, site, tab: forTab })) as Status,
  );
}

element<HTMLButtonElement>("private-add")?.addEventListener("click", (event) => {
  const site = (event.currentTarget as HTMLButtonElement).dataset.site;
  if (site) void changePrivate("add", site);
});

const toggle = element<HTMLButtonElement>("toggle");
if (toggle)
  onClick(toggle, async () => {
    toggle.disabled = true;
    const enabled = toggle.getAttribute("aria-checked") !== "true";
    show((await chrome.runtime.sendMessage({ type: "set", enabled, tab: forTab })) as Status);
  });

const profiles = [...document.querySelectorAll<HTMLButtonElement>("button[data-profile]")];
for (const button of profiles) {
  onClick(button, async () => {
    for (const each of profiles) each.disabled = true;
    show(
      (await chrome.runtime.sendMessage({
        type: "observe",
        profile: button.dataset.profile,
        tab: forTab,
      })) as Status,
    );
  });
}

// The recorder of the agent group of this tab: what the owner does there becomes a timeline the
// agent reads with `recorder events`, for example to write a skill from it.
const REFUSED: Record<string, string> = {
  no_agent_tab: "refusedNoAgentTab",
  session_not_found: "refusedSession",
  busy: "refusedBusy",
  permission_required: "refusedPermission",
  extension_unavailable: "refusedHost",
  timeout: "refusedTimeout",
};

function renderRecording(answer: Recording): void {
  const record = element<HTMLButtonElement>("record");
  const label = element("record-label");
  const note = element("recording");
  if (!record || !label || !note) return;
  const active = answer.recording === true;
  label.textContent = t(active ? "stopRecord" : "record");
  record.classList.toggle("recording", active);
  record.classList.toggle("primary", !active);
  record.dataset.recording = String(active);
  record.disabled = answer.error === "no_agent_tab";
  document.body.dataset.recording = answer.error ? "error" : String(active);
  note.textContent = answer.error
    ? REFUSED[answer.error]
      ? t(REFUSED[answer.error] ?? "")
      : (answer.message ?? answer.error)
    : active
      ? t("recordingNow", answer.events ?? 0)
      : answer.id
        ? t("recordedDone", answer.events ?? 0)
        : "";
  record.title = t("recordHint");
  // The hint lives in the button's tooltip; the line below only reports a recording or a refusal.
  note.hidden = document.body.dataset.agentTab !== "true" || note.textContent === "";
}

async function ask(action: "start" | "stop" | "status"): Promise<void> {
  lastRecording = (await chrome.runtime.sendMessage({
    type: "record",
    action,
    tab: forTab,
  })) as Recording;
  renderRecording(lastRecording);
}

element<HTMLButtonElement>("record")?.addEventListener("click", (event) => {
  const record = event.currentTarget as HTMLButtonElement;
  record.disabled = true;
  void ask(record.dataset.recording === "true" ? "stop" : "start");
});

// The newest steps of the agent group of the owner's tab, as its activity journal holds them.
const KINDS: Record<string, string> = {
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

// A step's age in the narrowest unit, such as "now", "4s" or "2 мин": the list reads newest first.
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
        : seconds < 86_400
          ? [Math.round(seconds / 3600), "hour"]
          : [Math.round(seconds / 86_400), "day"];
  return t("ageUnit", value, unit);
}

// Playwright's own selector engines, as the recorder writes them, read as what the owner sees:
// internal:role=textbox[name="Email"i] becomes textbox "Email".
function readable(locator: string): string {
  return locator
    .replace(/internal:role=([a-z]+)\[name="((?:[^"\\]|\\.)*)"[is]?\]/g, '$1 "$2"')
    .replace(
      /internal:(?:text|label|attr=\[placeholder|testid=\[data-testid)=?"((?:[^"\\]|\\.)*)"[is]?\]?/g,
      '"$1"',
    )
    .replace(/internal:role=([a-z]+)/g, "$1");
}

function step(line: string): HTMLLIElement {
  const item = document.createElement("li");
  let record: Record<string, unknown> = {};
  try {
    record = JSON.parse(line) as Record<string, unknown>;
  } catch {
    record = { type: "other", text: line };
  }
  const type = typeof record.type === "string" ? record.type : "other";
  item.dataset.type = type;
  item.dataset.private = String(record.private === true);
  item.dataset.error = String(
    record.ok === false || typeof record.error === "string" || typeof record.failure === "string",
  );
  const when = document.createElement("span");
  when.className = "when";
  when.textContent = ago(record.at);
  const kind = document.createElement("span");
  kind.className = "kind";
  kind.textContent = t(KINDS[type] ?? "kindOther");
  const what = document.createElement("span");
  what.className = "what";
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
  what.textContent = record.private === true ? `${t("private")} ${summary}`.trim() : summary;
  what.title = what.textContent;
  item.append(when, kind, what);
  return item;
}

function renderSteps(answer: { lines?: string[]; error?: string }): void {
  const list = element("recent");
  const note = element("recent-note");
  if (!list || !note) return;
  const lines = answer.lines ?? [];
  list.replaceChildren(...lines.slice().reverse().map(step));
  note.textContent = answer.error
    ? t(answer.error === "no_agent_tab" ? "activityNoTab" : "activityHost")
    : lines.length === 0
      ? t("activityNone")
      : "";
  note.hidden = note.textContent === "";
  const title = element("activity-title");
  if (title) title.title = t("activityHint");
  document.body.dataset.recent = String(lines.length);
}

async function refreshSteps(): Promise<void> {
  lastSteps = (await chrome.runtime.sendMessage({ type: "recent", tab: forTab })) as {
    lines?: string[];
    error?: string;
  };
  renderSteps(lastSteps);
}

async function start(): Promise<void> {
  config = (await fetch(chrome.runtime.getURL("public-settings.json")).then((response) =>
    response.json(),
  )) as FrontendConfig;
  for (const [id, url] of [
    ["guide-link", config.guideUrl],
    ["privacy-link", config.privacyUrl],
    ["consent-privacy", config.privacyUrl],
    ["install-link", config.installUrl],
  ]) {
    const anchor = element<HTMLAnchorElement>(id ?? "");
    if (anchor && url) anchor.href = url;
  }
  await loadLanguage();
  show((await chrome.runtime.sendMessage({ type: "get", tab: forTab })) as Status);
  if (!last?.consentRequired) await Promise.all([ask("status"), refreshSteps()]);
  let refreshing = false;
  setInterval(() => {
    if (refreshing) return;
    refreshing = true;
    void (async () => {
      show((await chrome.runtime.sendMessage({ type: "get", tab: forTab })) as Status);
      if (last?.consentRequired || !last?.connected) return;
      await refreshSteps();
      if (lastRecording.recording) await ask("status");
    })()
      .catch(() => undefined)
      .finally(() => {
        refreshing = false;
      });
  }, config.refreshMs);
}
void start().catch(() => {
  document.body.dataset.link = "down";
  const notice = element("setup");
  if (notice) {
    notice.hidden = false;
    notice.textContent = chrome.i18n.getMessage("setupFailed");
  }
});

for (const button of document.querySelectorAll<HTMLButtonElement>("button[data-consent-profile]")) {
  onClick(button, async () => {
    for (const each of document.querySelectorAll<HTMLButtonElement>("button[data-consent-profile]"))
      each.disabled = true;
    try {
      show(
        (await chrome.runtime.sendMessage({
          type: "accept",
          profile: button.dataset.consentProfile,
          tab: forTab,
        })) as Status,
      );
    } finally {
      for (const each of document.querySelectorAll<HTMLButtonElement>(
        "button[data-consent-profile]",
      ))
        each.disabled = false;
    }
  });
}
