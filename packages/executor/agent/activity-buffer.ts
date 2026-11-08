import type { ActivitySettings } from "../generated/types.gen.js";

type Raw = Record<string, unknown>;
type Entry = { record: Raw; bytes: number };
type Deliver = (epoch: string, records: Raw[], flush: boolean) => Promise<unknown>;

/** One bounded sender. A revoked generation can never acquire the next generation's identity. */
export class ActivityBuffer {
  private epoch: string | undefined;
  private queue: Entry[] = [];
  private bytes = 0;
  private activeRecords = 0;
  private activeBytes = 0;
  private sending: Promise<void> | undefined;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private flushRequested = false;

  constructor(
    private readonly limits: ActivitySettings,
    private readonly deliver: Deliver,
  ) {}

  setEpoch(epoch: string | undefined): void {
    if (epoch === this.epoch) return;
    this.epoch = epoch;
    this.queue = [];
    this.bytes = 0;
    this.flushRequested = false;
    this.clearTimer();
  }

  offer(epoch: string, record: Raw): boolean {
    if (epoch !== this.epoch) return false;
    const bytes = Buffer.byteLength(JSON.stringify(record));
    if (
      bytes > this.limits.batchBytes ||
      this.queue.length + this.activeRecords >= this.limits.queueRecords ||
      this.bytes + this.activeBytes + bytes > this.limits.queueBytes
    )
      return false;
    this.queue.push({ record, bytes });
    this.bytes += bytes;
    if (this.queue.length >= this.limits.batchRecords || this.bytes >= this.limits.batchBytes)
      void this.send();
    else this.timer ??= setTimeout(() => void this.send(), this.limits.sendDelayMs);
    return true;
  }

  flush(): Promise<void> {
    if (!this.epoch) return Promise.resolve();
    this.flushRequested = true;
    return this.send();
  }

  private clearTimer(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = undefined;
  }

  private send(): Promise<void> {
    this.clearTimer();
    this.sending ??= this.drain().finally(() => {
      this.sending = undefined;
      if (this.epoch && (this.queue.length || this.flushRequested)) void this.send();
    });
    return this.sending;
  }

  private async drain(): Promise<void> {
    while (this.epoch && (this.queue.length > 0 || this.flushRequested)) {
      const epoch = this.epoch;
      const entries: Entry[] = [];
      let bytes = 0;
      while (
        this.queue[0] &&
        entries.length < this.limits.batchRecords &&
        bytes + this.queue[0].bytes <= this.limits.batchBytes
      ) {
        const entry = this.queue.shift();
        if (!entry) break;
        entries.push(entry);
        bytes += entry.bytes;
      }
      this.bytes -= bytes;
      const flush = this.flushRequested && this.queue.length === 0;
      if (flush) this.flushRequested = false;
      this.activeRecords = entries.length;
      this.activeBytes = bytes;
      // A failed/uncertain delivery is discarded, never automatically replayed.
      await this.deliver(
        epoch,
        entries.map((entry) => entry.record),
        flush,
      ).catch(() => undefined);
      this.activeRecords = 0;
      this.activeBytes = 0;
    }
  }
}
