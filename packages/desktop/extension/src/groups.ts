import type { GroupIdentity, LifecycleSettings } from "./generated/types.gen.js";

const STORAGE = "ownedGroups";
const PAGE = "group.html";
type Binding = {
  identity: GroupIdentity;
  group: number;
  anchor: number;
  live: boolean;
  finishing?: Promise<void>;
};

/** Chrome group IDs are session-local. A registry entry plus exactly one control tab proves
 * recovery ownership; neither a group title nor an old numeric ID is enough. */
export class Groups {
  private readonly browser = chrome;
  private readonly conflicts = new Set<string>();
  private readonly identities = new Map<string, GroupIdentity>();
  private readonly bindings = new Map<string, Binding>();
  private readonly anchors = new Map<string, number>();
  private readonly creating = new Map<string, Promise<void>>();
  private saving: Promise<void> = Promise.resolve();

  constructor(
    private readonly limits: LifecycleSettings,
    private readonly groups: Map<string, number>,
    private readonly stop: (project: string, identity: string) => void,
    private readonly failed: () => void,
  ) {}

  isControl(url: string | undefined): boolean {
    const chrome = this.browser;
    return url?.split("#", 1)[0] === chrome.runtime.getURL(PAGE);
  }

  isOwnerPage(url: string | undefined): boolean {
    return /^(chrome-extension|chrome|edge|devtools):\/\//.test(url ?? "");
  }

  isAnchor(tab: number): boolean {
    return (
      [...this.anchors.values()].includes(tab) ||
      [...this.bindings.values()].some((binding) => binding.anchor === tab)
    );
  }

  private marker(url: string | undefined): string | undefined {
    if (!this.isControl(url)) return;
    const id = url?.split("#")[1];
    return id && /^[a-f0-9]{32}$/.test(id) ? id : undefined;
  }

  private persist(): Promise<void> {
    const chrome = this.browser;
    // Serialize writes, but take the current snapshot when the write starts. A late older write
    // can never revive an identity that a completed cleanup removed.
    const write = this.saving.then(() =>
      chrome.storage.local.set({
        [STORAGE]: { schema: 1, groups: [...this.identities.values()] },
      }),
    );
    this.saving = write.catch(() => undefined);
    return write;
  }

  async load(): Promise<void> {
    const chrome = this.browser;
    const stored: unknown = (await chrome.storage.local.get(STORAGE))[STORAGE];
    if (stored !== undefined) {
      if (
        typeof stored !== "object" ||
        stored === null ||
        !("schema" in stored) ||
        stored.schema !== 1 ||
        !("groups" in stored) ||
        !Array.isArray(stored.groups) ||
        stored.groups.length > this.limits.groupsLimit
      )
        throw new Error("invalid_group_registry");
      for (const value of stored.groups) {
        if (
          typeof value !== "object" ||
          value === null ||
          !("id" in value) ||
          typeof value.id !== "string" ||
          !/^[a-f0-9]{32}$/.test(value.id) ||
          this.identities.has(value.id) ||
          !("project" in value) ||
          typeof value.project !== "string" ||
          !/^[a-zA-Z0-9_-][a-zA-Z0-9_.-]{0,47}$/.test(value.project) ||
          !("createdAtMs" in value) ||
          !Number.isSafeInteger(value.createdAtMs) ||
          Number(value.createdAtMs) < 0
        )
          throw new Error("invalid_group_registry");
        this.identities.set(value.id, {
          id: value.id,
          project: value.project,
          createdAtMs: Number(value.createdAtMs),
        });
      }
    }
    await this.discover();
  }

  private async discover(): Promise<void> {
    const chrome = this.browser;
    const missing = [...this.identities.values()].filter(
      (identity) => !this.bindings.has(identity.id) && !this.creating.has(identity.project),
    );
    if (missing.length === 0) return;
    const tabs = await chrome.tabs.query({});
    for (const identity of missing) {
      const matches = tabs.filter((tab) => this.marker(tab.url ?? tab.pendingUrl) === identity.id);
      if (matches.length > 1) {
        this.conflicts.add(identity.id);
        continue;
      }
      const anchor = matches.length === 1 ? matches[0] : undefined;
      if (anchor?.id === undefined || anchor.groupId < 0) continue;
      const markers = tabs.filter((tab) => {
        const id = this.marker(tab.url ?? tab.pendingUrl);
        return tab.groupId === anchor.groupId && id !== undefined && this.identities.has(id);
      });
      if (markers.length !== 1) {
        this.conflicts.add(identity.id);
        continue;
      }
      this.conflicts.delete(identity.id);
      this.bindings.set(identity.id, {
        identity,
        group: anchor.groupId,
        anchor: anchor.id,
        live: false,
      });
    }
  }

  identity(id: string): GroupIdentity | undefined {
    return this.identities.get(id);
  }

  conflicted(id: string): boolean {
    return this.conflicts.has(id);
  }

  live(project: string): GroupIdentity | undefined {
    return [...this.bindings.values()].find(
      (binding) => binding.live && binding.identity.project === project && !binding.finishing,
    )?.identity;
  }

  async ensure(project: string): Promise<void> {
    const current = this.creating.get(project);
    if (current) {
      await current;
      if (this.live(project)) return;
      return this.ensure(project);
    }
    const work = this.create(project);
    this.creating.set(project, work);
    try {
      await work;
    } finally {
      if (this.creating.get(project) === work) this.creating.delete(project);
    }
  }

  private async create(project: string, retain = true): Promise<void> {
    const chrome = this.browser;
    if (this.live(project)) return;
    if (
      [...this.identities.values()].some(
        (identity) => identity.project === project && this.conflicts.has(identity.id),
      )
    )
      throw new Error("group_ownership_conflict");
    for (const binding of [...this.bindings.values()]) {
      if (binding.identity.project === project) await this.finish(binding, true);
    }
    if (this.identities.size >= this.limits.groupsLimit) throw new Error("group_capacity");
    const identity = {
      id: crypto.randomUUID().replaceAll("-", ""),
      project,
      createdAtMs: Date.now(),
    };
    this.identities.set(identity.id, identity);
    // Persist before creating a browser resource, so a worker failure leaves recoverable authority.
    await this.persist();
    const existing = this.groups.get(project);
    const previous =
      existing === undefined
        ? undefined
        : await chrome.tabGroups.get(existing).catch(() => undefined);
    const tab = await chrome.tabs.create({
      url: `${chrome.runtime.getURL(PAGE)}#${identity.id}`,
      active: false,
      ...(previous ? { windowId: previous.windowId } : {}),
    });
    if (tab.id === undefined) throw new Error("group_unavailable");
    // Chrome can still report about:blank until the requested control page commits. Its tab ID
    // is already reserved for the owner, including group events emitted before group() returns.
    this.anchors.set(identity.id, tab.id);
    const group = previous
      ? await chrome.tabs.group({ tabIds: [tab.id], groupId: previous.id })
      : await chrome.tabs.group({ tabIds: [tab.id] });
    const binding = { identity, group, anchor: tab.id, live: true };
    this.bindings.set(identity.id, binding);
    this.groups.set(project, group);
    if (!previous)
      await chrome.tabGroups.update(group, { title: project, color: "cyan", collapsed: false });
    if (!retain) await this.finish(binding, true);
  }

  async close(project: string, closeTabs: boolean): Promise<void> {
    await this.creating.get(project)?.catch(() => undefined);
    for (const binding of [...this.bindings.values()]) {
      if (binding.identity.project === project) await this.finish(binding, closeTabs);
    }
  }

  async closeId(id: string, closeTabs: boolean): Promise<void> {
    const binding = this.bindings.get(id);
    if (binding) await this.finish(binding, closeTabs);
  }

  private finish(binding: Binding, closeTabs: boolean): Promise<void> {
    if (binding.finishing) return binding.finishing;
    binding.live = false;
    binding.finishing = this.release(binding, closeTabs).finally(() => {
      delete binding.finishing;
    });
    return binding.finishing;
  }

  private async release(binding: Binding, closeTabs: boolean): Promise<void> {
    const chrome = this.browser;
    const tabs = await chrome.tabs.query({ groupId: binding.group });
    // Multiple control tabs mean conflicting/duplicated ownership. Keep the area intact.
    if (
      tabs.filter((tab) => {
        const id = this.marker(tab.url ?? tab.pendingUrl);
        return id !== undefined && this.identities.has(id);
      }).length > 1
    ) {
      this.conflicts.add(binding.identity.id);
      throw new Error("group_ownership_conflict");
    }
    this.conflicts.delete(binding.identity.id);
    for (const tab of tabs) {
      if (
        tab.id === undefined ||
        tab.id === binding.anchor ||
        this.isOwnerPage(tab.url ?? tab.pendingUrl)
      )
        continue;
      const current = await chrome.tabs.get(tab.id).catch(() => undefined);
      if (current?.groupId !== binding.group) continue;
      if (closeTabs) await chrome.tabs.remove(tab.id);
      else await chrome.tabs.ungroup(tab.id);
    }
    const anchor = await chrome.tabs.get(binding.anchor).catch(() => undefined);
    if (
      anchor?.groupId === binding.group &&
      this.marker(anchor.pendingUrl ?? anchor.url) === binding.identity.id
    )
      await chrome.tabs.remove(binding.anchor);
    // Any tab added after the closing snapshot belongs to the owner, not the retired life.
    const remaining = await chrome.tabs.query({ groupId: binding.group });
    for (const tab of remaining) {
      if (tab.id === undefined) continue;
      const current = await chrome.tabs.get(tab.id).catch(() => undefined);
      if (current?.groupId === binding.group) await chrome.tabs.ungroup(tab.id);
    }
    this.bindings.delete(binding.identity.id);
    this.anchors.delete(binding.identity.id);
    this.identities.delete(binding.identity.id);
    if (this.groups.get(binding.identity.project) === binding.group)
      this.groups.delete(binding.identity.project);
    await this.persist();
  }

  removed(tab: number): void {
    const binding = [...this.bindings.values()].find((entry) => entry.anchor === tab);
    if (!binding || binding.finishing) return;
    this.stop(binding.identity.project, binding.identity.id);
    void this.finish(binding, true).catch(this.failed);
  }

  moved(tab: number, group: number): void {
    const binding = [...this.bindings.values()].find((entry) => entry.anchor === tab);
    if (!binding || binding.group === group || binding.finishing) return;
    this.stop(binding.identity.project, binding.identity.id);
    void this.finish(binding, true).catch(this.failed);
  }

  navigated(tab: number, url: string): void {
    const binding = [...this.bindings.values()].find((entry) => entry.anchor === tab);
    if (!binding || this.marker(url) === binding.identity.id || binding.finishing) return;
    this.stop(binding.identity.project, binding.identity.id);
    void this.finish(binding, true).catch(this.failed);
  }

  async recover(): Promise<void> {
    await this.discover();
    // storage.session belongs to this browser lifetime. Its legacy mapping is still valid
    // authority during upgrade, unlike a numeric ID restored from disk after browser restart.
    // Mark then retire only idle legacy areas; an admitted claim serializes on the same promise.
    const legacy = [...this.groups.keys()]
      .filter(
        (project) =>
          /^[a-zA-Z0-9_-][a-zA-Z0-9_.-]{0,47}$/.test(project) &&
          !this.creating.has(project) &&
          ![...this.identities.values()].some((identity) => identity.project === project),
      )
      .slice(0, this.limits.groupsLimit);
    for (const project of legacy) {
      if (this.creating.has(project) || this.live(project)) continue;
      const work = this.create(project, false);
      this.creating.set(project, work);
      try {
        await work;
      } finally {
        if (this.creating.get(project) === work) this.creating.delete(project);
      }
    }
    const attempts = await Promise.allSettled(
      [...this.bindings.values()]
        .filter((binding) => !binding.live)
        .map((binding) => this.finish(binding, true)),
    );
    // Chrome can restore a closed window later. A temporarily missing marker grants no browser
    // authority, but does not erase the bounded recovery record or make us forget its group.
    const failed = attempts.find((attempt) => attempt.status === "rejected");
    if (failed?.status === "rejected") throw failed.reason;
  }
}
