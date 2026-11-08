// The recorder: what a person does in agent tabs, so an agent can turn a workflow into a skill.
// Playwright's own recorder, the one behind codegen, reports each action with the locator and the
// code codegen would write. They go into the activity journal (docs/observability.md) beside what
// the pages did meanwhile, live-region messages such as toasts, and a frame of each visual change.
// The host's Rust policy keeps a value typed into a secret field out of the journal; the executor
// tells it which fields are secret by what they are, not by their name. Reading is the host's:
// `recorder events` and `recorder frame`, also `recorder.events()` and `recorder.frame()` in code.
import type { BrowserContext, CDPSession, Page } from "playwright-core";
import type { CaptureReply, CaptureRequest } from "../generated/types.gen.js";
import { collectionStamp, emit, flush, pageId } from "./activity";
import { collectionEpoch, hostCommand, onObservation } from "./host";
import { secretField } from "./secret-field";
import { executorSettings } from "./settings";

const limits = executorSettings.recorder;
const TOASTS = "__ndsRecorderToast";

// Playwright 1.64 reports the action itself, with an aria snapshot of the page at that moment.
type Action = {
  name: string;
  ariaSnapshot?: string;
  selector?: string;
  text?: string;
  key?: string;
  options?: string[];
  files?: string[];
  clickCount?: number;
  button?: string;
  url?: string;
};
type Reported = Partial<Action> & {
  action?: Action;
  signal?: { name?: string; url?: string };
};
type Sink = {
  actionAdded(page: Page, data: Reported, code: string): void;
  actionUpdated(page: Page, data: Reported, code: string): void;
  signalAdded(page: Page, data: Reported): void;
};
type Recordable = BrowserContext & {
  _startRecording(params: { language: string }, sink: Sink): Promise<void>;
  _stopRecording(): Promise<void>;
};

type Frame = { at: number; data: string; size: number; scope: string; track: Track };
type Track = {
  page: Page;
  cdp: CDPSession;
  scope?: string | undefined;
  held?: Frame | undefined;
  last: number;
  wanted: boolean;
  transition?: Promise<void> | undefined;
  listener?: ((event: { data: string; sessionId: number }) => void) | undefined;
  changed: () => void;
  acks: Set<Promise<unknown>>;
};
type Recording = {
  id: string;
  epoch: string;
  dir: string;
  context: Recordable;
  started: number;
  events: number;
  frames: number;
  bytes: number;
  accepting: boolean;
  revoked: boolean;
  pages: Set<Page>;
  tracks: Track[];
  jobs: Set<Promise<void>>;
  queue: Frame[];
  queuedFrames: number;
  queuedBytes: number;
  writing?: Promise<void> | undefined;
  timer: ReturnType<typeof setTimeout>;
  order: Promise<void>;
  eventCount: number;
  eventBytes: number;
  secrets: Map<string, Promise<boolean>>;
  added: (page: Page) => void;
};
type Result = { id: string; path: string; reason: string; events: number; frames: number };
let current: Recording | undefined;
let closing: Promise<Result> | undefined;
let retiring: Recording | undefined;
let starting = false;
let next = 0;
const toasting = new WeakSet<Page>();

function valid(r: Recording): boolean {
  return !r.revoked && r.epoch === collectionEpoch();
}
function accepts(r: Recording): boolean {
  return current === r && r.accepting && valid(r);
}
function capture(request: CaptureRequest): Promise<CaptureReply | undefined> {
  return hostCommand<CaptureReply>("NDS.capture", request);
}

onObservation(() => {
  for (const r of new Set([current, retiring])) {
    if (r && !valid(r)) {
      revokeQueued(r);
      if (current === r) void finish(r, "policy-changed", false).catch(() => {});
    }
  }
});

// Runs in the page: reports the text of live regions (alerts, status messages, toasts) once each.
function toasts(binding: string): void {
  const page = window as unknown as Record<string, unknown>;
  if (page.__ndsToasts) return;
  page.__ndsToasts = true;
  const said = new WeakMap<Element, string>();
  new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      const node =
        mutation.target instanceof Element ? mutation.target : mutation.target.parentElement;
      const region = node?.closest(
        '[role="alert"],[role="status"],[aria-live]:not([aria-live="off"])',
      );
      if (!(region instanceof HTMLElement)) continue;
      const text = (region.textContent ?? "").replace(/\s+/g, " ").trim().slice(0, 500);
      const report = page[binding];
      if (text && said.get(region) !== text && typeof report === "function") {
        said.set(region, text);
        report(text);
      }
    }
  }).observe(document, { subtree: true, childList: true, characterData: true });
}

function secret(r: Recording, page: Page, selector: string): Promise<boolean> {
  const key = `${pageId(page)} ${selector}`;
  // Bound the retained cache bytes as well as its entry count; unknown fields stay redacted.
  if (Buffer.byteLength(key) > Math.floor(limits.queuedBytes / limits.secretCache))
    return Promise.resolve(true);
  let known = r.secrets.get(key);
  if (!known) {
    if (r.secrets.size >= limits.secretCache) return Promise.resolve(true);
    known = page
      .locator(selector)
      .first()
      .evaluate(secretField, undefined, { timeout: limits.secretTimeoutMs })
      .catch(() => true)
      .then((hidden) => {
        if (!hidden) r.secrets.delete(key);
        return hidden;
      });
    r.secrets.set(key, known);
  }
  return known;
}

function person(r: Recording, page: Page, data: Reported, code: string, update: boolean): void {
  const action = data.action ?? (typeof data.name === "string" ? (data as Action) : undefined);
  const stamp = collectionStamp();
  if (!action || !accepts(r) || !stamp || stamp.epoch !== r.epoch) return;
  const size = Buffer.byteLength(JSON.stringify(data)) + Buffer.byteLength(code);
  if (
    size > executorSettings.activity.batchBytes ||
    r.eventCount >= limits.queuedEvents ||
    r.eventBytes + size > limits.queuedBytes
  ) {
    void finish(r, "capacity", false).catch(() => {});
    return;
  }
  r.eventCount++;
  r.eventBytes += size;
  r.order = r.order.then(async () => {
    try {
      if (!valid(r)) return;
      const hidden =
        action.name === "fill" && action.selector ? await secret(r, page, action.selector) : false;
      if (!valid(r)) return;
      r.events += update ? 0 : 1;
      emit(
        {
          type: "person-action",
          recording: r.id,
          page: pageId(page),
          action: action.name,
          code: code.trim(),
          update,
          secret: hidden,
          ...(action.ariaSnapshot && !hidden ? { aria: action.ariaSnapshot } : {}),
          ...(action.selector ? { locator: action.selector } : {}),
          ...(action.text === undefined ? {} : { value: action.text }),
          ...(action.key ? { key: action.key } : {}),
          ...(action.options ? { options: action.options } : {}),
          ...(action.files ? { files: action.files } : {}),
          ...(action.clickCount === undefined ? {} : { clickCount: action.clickCount }),
          ...(action.button ? { button: action.button } : {}),
          ...(action.url ? { url: action.url } : {}),
        },
        stamp,
      );
    } finally {
      r.eventCount--;
      r.eventBytes -= size;
    }
  });
}

function release(r: Recording, frame: Frame): void {
  r.queuedFrames--;
  r.queuedBytes -= frame.size;
}
function invalidate(r: Recording, track: Track): void {
  track.scope = undefined;
  if (track.held) release(r, track.held);
  track.held = undefined;
}

function revokeQueued(r: Recording): void {
  r.revoked = true;
  for (const track of r.tracks) invalidate(r, track);
  for (const frame of r.queue.splice(0)) release(r, frame);
  r.secrets.clear();
}
function drain(r: Recording): Promise<void> {
  r.writing ??= (async () => {
    while (r.queue.length) {
      const frame = r.queue.shift();
      if (!frame) break;
      try {
        if (!valid(r) || frame.track.scope !== frame.scope) continue;
        const result = await capture({
          action: "frame",
          id: r.id,
          epoch: r.epoch,
          scope: frame.scope,
          page: pageId(frame.track.page),
          at: frame.at,
          data: frame.data,
        });
        if (result?.kept) {
          r.frames = result.frames;
          r.bytes = result.bytes;
        } else if (result?.error === "document_changed") {
          invalidate(r, frame.track);
          if (accepts(r)) void transition(r, frame.track);
        } else if (accepts(r)) {
          void finish(r, result?.error ?? "unavailable", false).catch(() => {});
        }
      } finally {
        release(r, frame);
      }
    }
  })().finally(() => {
    r.writing = undefined;
    if (r.queue.length) void drain(r);
  });
  return r.writing;
}
function retain(r: Recording, track: Track, scope: string, data: string): void {
  if (!accepts(r) || track.scope !== scope) return;
  if (data.length > Math.ceil(limits.frameBytes / 3) * 4) {
    void finish(r, "frame-limit", false).catch(() => {});
    return;
  }
  if (track.held) release(r, track.held);
  track.held = undefined;
  if (r.queuedFrames >= limits.queuedFrames || r.queuedBytes + data.length > limits.queuedBytes) {
    void finish(r, "capacity", false).catch(() => {});
    return;
  }
  const frame = { at: Date.now() - r.started, data, size: data.length, scope, track };
  r.queuedFrames++;
  r.queuedBytes += frame.size;
  if (frame.at - track.last < limits.frameIntervalMs) track.held = frame;
  else {
    track.last = frame.at;
    r.queue.push(frame);
    void drain(r);
  }
}

function transition(r: Recording, track: Track): Promise<void> {
  r.secrets.clear();
  invalidate(r, track);
  track.wanted = true;
  track.transition ??= (async () => {
    while (track.wanted && accepts(r) && !track.page.isClosed()) {
      track.wanted = false;
      // Drain the old stream before authorizing another document. Its listener keeps only ACKs.
      await track.cdp.send("Page.stopScreencast").catch(() => {});
      if (track.listener) track.cdp.off("Page.screencastFrame", track.listener);
      track.listener = undefined;
      if (!accepts(r)) break;
      const target = await track.cdp.send("Target.getTargetInfo").catch(() => undefined);
      if (!target || !accepts(r)) break;
      const answer = await capture({
        action: "page",
        id: r.id,
        epoch: r.epoch,
        page: pageId(track.page),
        target: target.targetInfo.targetId,
      });
      if (!accepts(r)) break;
      if (answer?.error === "document_changed" || track.wanted) {
        track.wanted = true;
        continue;
      }
      if (answer?.error === "private_page") break;
      if (!answer?.kept || !answer.scope) {
        void finish(r, answer?.error ?? "unavailable", false).catch(() => {});
        break;
      }
      const scope = answer.scope;
      track.scope = scope;
      track.listener = ({ data, sessionId }) => {
        if (track.acks.size >= 3) {
          void finish(r, "capacity", false).catch(() => {});
          return;
        }
        const ack = track.cdp.send("Page.screencastFrameAck", { sessionId }).catch(() => {});
        track.acks.add(ack);
        void ack.finally(() => track.acks.delete(ack));
        retain(r, track, scope, data);
      };
      track.cdp.on("Page.screencastFrame", track.listener);
      await track.cdp
        .send("Page.startScreencast", {
          format: "jpeg",
          quality: limits.jpegQuality,
          maxWidth: limits.maxEdge,
          maxHeight: limits.maxEdge,
        })
        .catch(() => {
          void finish(r, "unavailable", false).catch(() => {});
        });
    }
  })().finally(() => {
    track.transition = undefined;
    if (track.wanted && accepts(r)) void transition(r, track);
  });
  return track.transition;
}

async function follow(r: Recording, page: Page): Promise<void> {
  if (!accepts(r) || r.pages.has(page)) return;
  r.pages.add(page);
  if (!toasting.has(page)) {
    toasting.add(page);
    await page
      .exposeBinding(TOASTS, ({ page: source }, text: string) => {
        const active = current;
        if (active && accepts(active)) emit({ type: "toast", page: pageId(source as Page), text });
      })
      .catch(() => {});
    if (!accepts(r)) return;
    await page.addInitScript(toasts, TOASTS).catch(() => {});
  }
  if (!accepts(r)) return;
  await page.evaluate(toasts, TOASTS).catch(() => {});
  if (!accepts(r) || page.isClosed()) return;
  const cdp = await page
    .context()
    .newCDPSession(page)
    .catch(() => undefined);
  if (!cdp) return;
  if (!accepts(r)) {
    await cdp.detach().catch(() => {});
    return;
  }
  const track: Track = {
    page,
    cdp,
    last: Number.NEGATIVE_INFINITY,
    wanted: false,
    acks: new Set(),
    changed: () => {
      if (accepts(r)) void transition(r, track);
    },
  };
  r.tracks.push(track);
  // Any frame can contain private pixels, including a child iframe of a public page.
  page.on("framenavigated", track.changed);
  page.on("framedetached", track.changed);
  page.on("frameattached", track.changed);
  await transition(r, track);
}

function followJob(r: Recording, page: Page): Promise<void> {
  const job = follow(r, page).catch(() => {
    void finish(r, "unavailable", false).catch(() => {});
  });
  r.jobs.add(job);
  void job.finally(() => r.jobs.delete(job));
  return job;
}

async function start(
  { outputDir }: { outputDir?: string } = {},
  context: BrowserContext,
): Promise<{ id: string; path: string }> {
  if (current || closing || starting) throw new Error("a recording is active or still stopping");
  const epoch = collectionEpoch();
  if (!epoch)
    throw new Error(
      "The recorder requires the owner's Full observation consent in the extension popup",
    );
  starting = true;
  const id = `${Date.now().toString(36)}${++next}`;
  let r: Recording | undefined;
  try {
    const result = await capture({
      action: "begin",
      id,
      epoch,
      ...(outputDir ? { directory: outputDir } : {}),
    });
    if (!result?.kept || !result.directory || epoch !== collectionEpoch()) {
      await capture({ action: "stop", id });
      throw new Error(`recorder could not start: ${result?.error ?? "policy_changed"}`);
    }
    const recording: Recording = {
      id,
      epoch,
      dir: result.directory,
      context: context as Recordable,
      started: Date.now(),
      events: 0,
      frames: 0,
      bytes: 0,
      accepting: true,
      revoked: false,
      pages: new Set(),
      tracks: [],
      jobs: new Set(),
      queue: [],
      queuedFrames: 0,
      queuedBytes: 0,
      timer: setTimeout(() => {
        void finish(recording, "max-duration", true).catch(() => {});
      }, limits.maxMs),
      order: Promise.resolve(),
      eventCount: 0,
      eventBytes: 0,
      secrets: new Map(),
      added: (page) => {
        if (accepts(recording)) void followJob(recording, page);
      },
    };
    r = recording;
    current = r;
    emit({ type: "recording", event: "started", recording: id, dir: r.dir });
    await r.context._startRecording(
      { language: "javascript" },
      {
        actionAdded: (page, data, code) => person(recording, page, data, code, false),
        actionUpdated: (page, data, code) => person(recording, page, data, code, true),
        signalAdded: (page, data) => {
          if (!accepts(recording)) return;
          emit({
            type: "person-signal",
            recording: id,
            page: pageId(page),
            ...((data.signal?.name ?? data.name) ? { signal: data.signal?.name ?? data.name } : {}),
            ...((data.signal?.url ?? data.url) ? { url: data.signal?.url ?? data.url } : {}),
          });
        },
      },
    );
    if (!accepts(r)) throw new Error("recorder policy changed while starting");
    context.on("page", r.added);
    for (const page of context.pages()) await followJob(r, page);
    if (!accepts(r)) throw new Error("recorder stopped while starting");
    return { id, path: r.dir };
  } catch (error) {
    if (r && current === r) await finish(r, "start-failed", false);
    throw error;
  } finally {
    starting = false;
  }
}

function finish(r: Recording, reason: string, normal: boolean): Promise<Result> {
  if (closing) return closing;
  if (current === r) current = undefined;
  r.accepting = false;
  r.revoked = !normal || !valid(r);
  clearTimeout(r.timer);
  r.context.off("page", r.added);
  if (r.revoked) revokeQueued(r);
  retiring = r;
  closing = (async () => {
    try {
      await r.context._stopRecording().catch(() => {});
      await Promise.all(r.jobs);
      for (const track of r.tracks) {
        track.page.off("framenavigated", track.changed);
        track.page.off("framedetached", track.changed);
        track.page.off("frameattached", track.changed);
        await track.transition;
        await track.cdp.send("Page.stopScreencast").catch(() => {});
        if (track.listener) track.cdp.off("Page.screencastFrame", track.listener);
        await Promise.all(track.acks);
        await track.cdp.detach().catch(() => {});
        if (track.held) {
          r.queue.push(track.held);
          track.held = undefined;
        }
      }
      await drain(r);
      await r.order;
      if (valid(r)) {
        emit({ type: "recording", event: "stopped", recording: r.id, reason });
        await flush();
      }
    } finally {
      await capture({ action: "stop", id: r.id });
    }
    return { id: r.id, path: r.dir, reason, events: r.events, frames: r.frames };
  })().finally(() => {
    closing = undefined;
    retiring = undefined;
  });
  return closing;
}
async function stop(): Promise<Result> {
  if (closing) return closing;
  if (!current) throw new Error("no recording is active");
  return finish(current, "stopped", true);
}

type Lines = { lines?: string[]; error?: string };

async function ask(frame: boolean, words: string[]): Promise<string[]> {
  if (current) await flush();
  const answer = await hostCommand<Lines>("NDS.recording", { frame, words });
  if (!answer?.lines) {
    throw new Error(
      answer?.error === "not_found"
        ? "the activity journal holds no such recording, event or frame"
        : "the activity journal could not be read",
    );
  }
  return answer.lines;
}

/**
 * The events of a recording, the latest by default: a thin timeline, only one type, only errors
 * (failed requests, console and page errors), without requests matching `exclude`, or the full
 * records of some ids. One JSON object per line.
 */
async function events({
  id,
  type,
  errors,
  full,
  ids,
  exclude,
}: {
  id?: string;
  type?: string;
  errors?: boolean;
  full?: boolean;
  ids?: number[];
  exclude?: string;
} = {}): Promise<string> {
  const words = [
    ...(id ? ["--recording", id] : []),
    ...(type ? ["--type", type] : []),
    ...(errors ? ["--errors"] : []),
    ...(full ? ["--full"] : []),
    ...(exclude ? ["--exclude", exclude] : []),
    ...(ids ?? []).map(String),
  ];
  return (await ask(false, words)).join("\n") || "No events";
}

/** The frame of an event's page at the event plus `offset` milliseconds, to see that screen. */
async function frame({
  eventId,
  offset = 0,
  id,
}: {
  eventId: number;
  offset?: number;
  id?: string;
}): Promise<string> {
  const lines = await ask(true, [
    String(eventId),
    "--offset",
    String(offset),
    ...(id ? ["--recording", id] : []),
  ]);
  return lines[0] ?? "";
}

export function recorderFor(context: BrowserContext, _artifacts: string) {
  return {
    start: (options: { outputDir?: string } = {}) => start(options, context),
    stop,
    status: () => (current ? { id: current.id, events: current.events, path: current.dir } : null),
    events,
    frame,
  };
}
