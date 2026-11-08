import type { ExecutionSettings } from "../generated/types.gen.js";

type Step = { name: string; at: number };
type Finished = { name: string; ms: number };

/** A bounded explanation for this execution's caller; it holds no operation arguments. */
export class ExecutionSteps {
  private readonly active = new Map<object, Step>();
  private readonly recent: Finished[] = [];
  private omitted = 0;
  private closed = false;

  constructor(
    private readonly limits: Pick<ExecutionSettings, "pendingSteps" | "recentSteps">,
    private readonly now: () => number = () => performance.now(),
  ) {}

  begin(key: object, label: unknown): void {
    if (this.closed || this.active.has(key)) return;
    if (this.active.size >= this.limits.pendingSteps) {
      this.omitted++;
      return;
    }
    const name =
      typeof label === "string" && /^[A-Za-z][A-Za-z0-9_.]{0,95}$/.test(label)
        ? label
        : "browser.operation";
    this.active.set(key, { name, at: this.now() });
  }

  end(key: object): void {
    const step = this.active.get(key);
    if (!step) return;
    this.active.delete(key);
    if (this.recent.length === this.limits.recentSteps) {
      this.recent.shift();
      this.omitted++;
    }
    this.recent.push({ name: step.name, ms: Math.max(0, Math.round(this.now() - step.at)) });
  }

  describe(): string {
    const now = this.now();
    const completed = this.recent.map((step) => `${step.name} ${step.ms} ms`).join("; ");
    const running = [...this.active.values()]
      .map((step) => `${step.name} ${Math.max(0, Math.round(now - step.at))} ms`)
      .join("; ");
    return `Execution steps: completed [${completed}]; running [${running}].${this.omitted ? ` ${this.omitted} steps omitted by the configured bound.` : ""}`;
  }

  close(): void {
    this.closed = true;
    this.active.clear();
    this.recent.length = 0;
  }
}
