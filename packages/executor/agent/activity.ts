// What the executor sees for the activity journal (docs/observability.md): executions, the agent's
// Playwright actions, and each page's navigations, requests, console messages, errors and
// downloads. It collects only in the full observation profile and hands raw records to the host,
// whose Rust policy removes secrets, leaves noise out, folds repeats and writes the journal. No
// Playwright listener here changes a page's behavior: dialogs, which a listener would keep open,
// are journaled by the host from the browser's own events.
import type { BrowserContext, ConsoleMessage, Page, Request } from "playwright-core";
import { scope } from "../execution";
import { ActivityBuffer } from "./activity-buffer";
import { collectionEpoch, hostCommand, onObservation } from "./host";
import { secretField } from "./secret-field";
import { executorSettings } from "./settings";

const limits = executorSettings.activity;
const TEXTUAL = /json|text|xml|x-www-form-urlencoded|graphql|javascript/i;
// Agent actions worth a record; helpers' own evaluations and waits are not actions.
const ACTIONS = new Set([
  "check",
  "click",
  "close",
  "dblclick",
  "dispatchEvent",
  "dragAndDrop",
  "fill",
  "goBack",
  "goForward",
  "goto",
  "hover",
  "keyboardInsertText",
  "keyboardPress",
  "keyboardType",
  "mouseClick",
  "press",
  "reload",
  "selectOption",
  "setInputFiles",
  "tap",
  "type",
  "uncheck",
]);

type Raw = { [field: string]: unknown };

let started = false;
let exec: number | undefined;
let executionStamp: Stamp | undefined;
const buffer = new ActivityBuffer(limits, (epoch, records, flush) =>
  hostCommand("NDS.activity", { epoch, records, flush }),
);
type Stamp = { epoch: string; at: string; exec?: number };
let networkTasks = 0;
const secretTasks = new Set<Promise<void>>();
const requests = new WeakMap<Request, Stamp>();
const pages = new WeakMap<Page, number>();
let nextPage = 0;
const calls = new WeakMap<
  object,
  { at: number; record: Raw; stamp: Stamp; owner?: object | undefined; page?: Page | undefined }
>();

/** Identity is captured before asynchronous work, together with its execution and time. */
export function collectionStamp(): Stamp | undefined {
  const epoch = started ? collectionEpoch() : undefined;
  const operation = scope.getStore()?.id ?? exec;
  return epoch
    ? {
        epoch,
        at: new Date().toISOString(),
        ...(operation === undefined ? {} : { exec: operation }),
      }
    : undefined;
}

export function emit(record: Raw, stamp = collectionStamp()): void {
  if (!stamp || stamp.epoch !== collectionEpoch()) return;
  const { epoch, ...identity } = stamp;
  let full: Raw = { ...record, ...identity };
  if (Buffer.byteLength(JSON.stringify(full)) > limits.batchBytes && "body" in full) {
    const { body: _, ...rest } = full;
    full = { ...rest, bodyOmitted: "too-large" };
  }
  buffer.offer(epoch, full);
}

export function flush(): Promise<void> {
  return buffer.flush();
}

/** The journal's number of a page, 0 for one it never saw. */
export function pageId(page: Page): number {
  return pages.get(page) ?? 0;
}

type Zone = { owner?: object };
type Wrap = (
  this: object,
  func: (zone: Zone) => Promise<unknown>,
  options?: object,
) => Promise<unknown>;

// Playwright's instrumentation names no page for an action. The object whose outermost call
// starts it does, a frame or a page, so each call's zone keeps that object; the journal then
// knows which page, and so which site, an action is of. Without this, the journal treats an
// action as of any private site open.
function namePages(context: BrowserContext): void {
  const owner = Object.getPrototypeOf(Object.getPrototypeOf(context)) as {
    _wrapApiCall?: Wrap & { named?: true };
  };
  const wrap = owner._wrapApiCall;
  if (typeof wrap !== "function" || wrap.named) return;
  const named: Wrap & { named?: true } = function (this: object, func, options) {
    return wrap.call(
      this,
      (zone) => {
        zone.owner ??= this;
        return func(zone);
      },
      options,
    );
  };
  named.named = true;
  owner._wrapApiCall = named;
}

function pageOf(owner: object | undefined): Page | undefined {
  const target = owner as { page?: () => Page; mainFrame?: () => unknown } | undefined;
  if (typeof target?.page === "function") return target.page();
  if (typeof target?.mainFrame === "function") return target as unknown as Page;
  return undefined;
}

/** Starts collecting; the host keeps the journal of this executor's session. */
export function start(context: BrowserContext): void {
  started = true;
  namePages(context);
  // Off and standard collect nothing: records not yet sent are dropped.
  buffer.setEpoch(collectionEpoch());
  onObservation(() => buffer.setEpoch(collectionEpoch()));
  context.on("close", () => buffer.setEpoch(undefined));
}

/** The execution the following records belong to, until the next one starts. */
export function executing(id: number): void {
  exec = id;
  executionStamp = collectionStamp();
}

/** One execution's end, sent at once with everything before it. */
export function execution(record: Raw): Promise<void> {
  const stamp = executionStamp;
  executionStamp = undefined;
  if (stamp) emit({ type: "execution", ...record }, stamp);
  return flush();
}

/** Playwright's instrumentation listener: the agent's actions with their raw values. */
export const actions = {
  onApiCallBegin(zone: object, channel: { type: string; method: string; params?: Raw }): void {
    const stamp = collectionStamp();
    if (!stamp || !ACTIONS.has(channel.method)) return;
    const params = channel.params ?? {};
    const value = ["value", "text"]
      .map((name) => params[name])
      .find((each) => typeof each === "string");
    const page = pageOf((zone as Zone).owner);
    const record: Raw = {
      type: "action",
      ...(page && pages.has(page) ? { page: pageId(page) } : {}),
      api: (zone as { apiName?: string }).apiName ?? channel.method,
      ...(typeof params.selector === "string" ? { locator: params.selector } : {}),
      ...(typeof params.url === "string" ? { url: params.url } : {}),
      ...(typeof params.key === "string" ? { key: params.key } : {}),
      ...(typeof value === "string" ? { value } : {}),
      ...(Array.isArray(params.localPaths) ? { files: params.localPaths } : {}),
    };
    calls.set(zone, { at: Date.now(), record, stamp, owner: (zone as Zone).owner, page });
  },
  onApiCallEnd(zone: object): void {
    const call = calls.get(zone);
    if (!call) return;
    calls.delete(zone);
    const error = (zone as { error?: Error }).error;
    const record = {
      ...call.record,
      ms: Date.now() - call.at,
      ...(error ? { error: error.message } : {}),
    };
    if (typeof call.record.value !== "string") {
      emit(record, call.stamp);
      return;
    }
    const owner = call.owner as { locator?: Page["locator"] } | undefined;
    const target = typeof owner?.locator === "function" ? owner : call.page;
    if (
      !target?.locator ||
      typeof call.record.locator !== "string" ||
      networkTasks + secretTasks.size >= limits.networkTasks
    ) {
      emit({ ...record, secret: true }, call.stamp);
      return;
    }
    const task = target
      .locator(call.record.locator)
      .first()
      .evaluate(secretField, undefined, { timeout: executorSettings.recorder.secretTimeoutMs })
      .catch(() => true)
      .then((secret) => emit({ ...record, secret }, call.stamp));
    secretTasks.add(task);
    void task.finally(() => secretTasks.delete(task));
  },
};

async function network(page: Page, id: number, request: Request, failure?: string): Promise<void> {
  const stamp = requests.get(request);
  requests.delete(request);
  if (
    !stamp ||
    stamp.epoch !== collectionEpoch() ||
    networkTasks + secretTasks.size >= limits.networkTasks
  )
    return;
  networkTasks++;
  try {
    const pageUrl = page.url();
    const response = failure ? null : await request.response().catch(() => null);
    if (stamp.epoch !== collectionEpoch()) return;
    const type = response?.headers()["content-type"] ?? "";
    const record: Raw = {
      type: "network",
      page: id,
      pageUrl,
      method: request.method(),
      url: request.url(),
      resource: request.resourceType(),
      status: response?.status() ?? 0,
      ...(failure ? { failure } : {}),
      ...(type ? { contentType: type } : {}),
      headers: request.headers(),
    };
    const requestHeaders = request.headers();
    const requestLength = requestHeaders["content-length"];
    if (
      requestLength &&
      /^\d+$/.test(requestLength) &&
      Number(requestLength) <= limits.bodyBytes &&
      !requestHeaders["transfer-encoding"] &&
      !requestHeaders["content-encoding"]
    ) {
      const post = request.postData();
      if (post !== null && Buffer.byteLength(post) <= limits.bodyBytes) {
        record.postData = post;
        record.requestType = requestHeaders["content-type"] ?? "";
      }
    }
    if (response && TEXTUAL.test(type) && request.resourceType() !== "document") {
      const headers = response.headers();
      const length = headers["content-length"];
      const known =
        length !== undefined && /^\d+$/.test(length) && Number.isSafeInteger(Number(length));
      if (
        !known ||
        headers["transfer-encoding"] ||
        response.fromServiceWorker() ||
        (headers["content-encoding"] && headers["content-encoding"] !== "identity")
      ) {
        record.bodyOmitted = "unbounded";
      } else if (Number(length) > limits.bodyBytes) {
        record.bodyOmitted = "too-large";
      } else if (stamp.epoch === collectionEpoch()) {
        const body = await response.text().catch(() => undefined);
        if (body !== undefined && Buffer.byteLength(body) <= limits.bodyBytes) record.body = body;
        else if (body !== undefined) record.bodyOmitted = "too-large";
      }
    }
    emit(record, stamp);
  } finally {
    networkTasks--;
  }
}

/** Collects one page's navigations, requests, console, errors, downloads and closing. */
export function attach(page: Page): void {
  if (pages.has(page) || !started) return;
  const id = ++nextPage;
  pages.set(page, id);
  emit({ type: "page", page: id, event: "opened", url: page.url() });
  page.on("framenavigated", (frame) => {
    if (frame === page.mainFrame()) emit({ type: "navigation", page: id, url: frame.url() });
  });
  page.on("close", () => emit({ type: "page", page: id, event: "closed" }));
  page.on("console", (message: ConsoleMessage) =>
    emit({ type: "console", page: id, level: message.type(), text: message.text() }),
  );
  page.on("pageerror", (error) =>
    emit({ type: "page-error", page: id, message: String(error.message ?? error) }),
  );
  page.on("download", (download) =>
    emit({ type: "download", page: id, url: download.url(), name: download.suggestedFilename() }),
  );
  page.on("request", (request) => {
    const stamp = collectionStamp();
    if (stamp) requests.set(request, stamp);
  });
  page.on("requestfinished", (request) => void network(page, id, request));
  page.on(
    "requestfailed",
    (request) => void network(page, id, request, request.failure()?.errorText ?? "failed"),
  );
}
