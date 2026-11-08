import { expect, test } from "vitest";
import { ActivityBuffer } from "../agent/activity-buffer";
import { executorSettings } from "../agent/settings";

function deferred() {
  let resolve: () => void = () => {};
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

test("revocation drops queued records and a late result cannot enter a new Full generation", async () => {
  const pending = deferred();
  const deliveries: { epoch: string; records: Record<string, unknown>[] }[] = [];
  const buffer = new ActivityBuffer(
    { ...executorSettings.activity, batchRecords: 1 },
    async (epoch, records) => {
      deliveries.push({ epoch, records });
      if (deliveries.length === 1) await pending.promise;
    },
  );
  buffer.setEpoch("old");
  expect(buffer.offer("old", { value: "already sending" })).toBe(true);
  expect(buffer.offer("old", { value: "must be discarded" })).toBe(true);
  buffer.setEpoch(undefined);
  buffer.setEpoch("new");
  expect(buffer.offer("old", { value: "late private response" })).toBe(false);
  expect(buffer.offer("new", { value: "new consent" })).toBe(true);
  pending.resolve();
  await buffer.flush();
  expect(deliveries).toEqual([
    { epoch: "old", records: [{ value: "already sending" }] },
    { epoch: "new", records: [{ value: "new consent" }] },
  ]);
  buffer.setEpoch(undefined);
});

test("the count and byte budgets include the one in-flight batch", async () => {
  for (const budget of ["records", "bytes"] as const) {
    const pending = deferred();
    let active = 0;
    let peak = 0;
    const record = { value: "fixture" };
    const size = Buffer.byteLength(JSON.stringify(record));
    const buffer = new ActivityBuffer(
      {
        ...executorSettings.activity,
        batchRecords: 1,
        queueRecords: budget === "records" ? 2 : 20,
        queueBytes: budget === "bytes" ? 2 * size : 4096,
      },
      async () => {
        active++;
        peak = Math.max(peak, active);
        await pending.promise;
        active--;
      },
    );
    buffer.setEpoch("full");
    expect(buffer.offer("full", record)).toBe(true);
    expect(buffer.offer("full", record)).toBe(true);
    expect(buffer.offer("full", record)).toBe(false);
    pending.resolve();
    await buffer.flush();
    expect(peak).toBe(1);
    buffer.setEpoch(undefined);
  }
});

test("uncertain delivery is discarded without retry", async () => {
  let attempts = 0;
  const buffer = new ActivityBuffer(executorSettings.activity, async () => {
    attempts++;
    throw new Error("fixture transport closed after admission");
  });
  buffer.setEpoch("full");
  buffer.offer("full", { value: "fixture" });
  await buffer.flush();
  expect(attempts).toBe(1);
  buffer.setEpoch(undefined);
});
