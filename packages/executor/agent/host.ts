// The executor's own browser-level CDP session to the host. The host answers its NDS commands
// there (a download's kept file, the owner's observation profile) and reports profile changes on
// it, so collection follows the owner's switch between executions too (docs/observability.md).
import type { Browser, CDPSession } from "playwright-core";

export type Observation = "full" | "standard" | "off";

let session: Promise<CDPSession | undefined> | undefined;
let current: Observation = "standard";
let epoch: string | undefined;
const listeners = new Set<(profile: Observation) => void>();

function parse(value: unknown): Observation {
  return value === "full" || value === "off" ? value : "standard";
}

function set(value: { profile?: unknown; epoch?: unknown } | undefined): void {
  const token =
    typeof value?.epoch === "string" && /^[0-9a-f]{16}$/.test(value.epoch)
      ? value.epoch
      : undefined;
  let profile = parse(value?.profile);
  // An older host cannot revoke already-running work, so never collect through it.
  if (profile === "full" && !token) profile = "standard";
  if (profile === current && token === epoch) return;
  current = profile;
  epoch = token;
  for (const listener of listeners) listener(profile);
}

/** Opens the session once; an older host without it leaves the standard profile. */
export function connect(browser: Browser): Promise<CDPSession | undefined> {
  session ??= browser
    .newBrowserCDPSession()
    .then(async (cdp) => {
      (
        cdp as unknown as {
          on(
            event: string,
            listener: (params: { profile?: unknown; epoch?: unknown }) => void,
          ): void;
        }
      ).on("NDS.observation", set);
      const answer = await ask<{ profile?: unknown; epoch?: unknown }>(cdp, "NDS.observation", {});
      set(answer);
      browser.on("disconnected", () => set({ profile: "off" }));
      return cdp;
    })
    .catch(() => undefined);
  return session;
}

/** The host's answer to one NDS command, or undefined when the host has no such session. */
export async function hostCommand<T>(method: string, params: object): Promise<T | undefined> {
  const cdp = await session;
  return cdp ? ask<T>(cdp, method, params) : undefined;
}

function ask<T>(cdp: CDPSession, method: string, params: object): Promise<T | undefined> {
  const send = cdp.send.bind(cdp) as unknown as (method: string, params: object) => Promise<T>;
  return send(method, params).catch(() => undefined);
}

/** Opaque host generation, valid only while Full remains unchanged. Never retain across awaits. */
export function collectionEpoch(): string | undefined {
  return current === "full" ? epoch : undefined;
}

/** Calls `listener` with every later profile. */
export function onObservation(listener: (profile: Observation) => void): void {
  listeners.add(listener);
}
