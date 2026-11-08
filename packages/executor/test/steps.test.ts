import { expect, test } from "bun:test";
import { ExecutionSteps } from "../agent/steps";

test("concurrent browser steps keep their own durations and completion order", () => {
  let clock = 0;
  const steps = new ExecutionSteps({ pendingSteps: 2, recentSteps: 2 }, () => clock);
  const navigation = {};
  const capture = {};
  steps.begin(navigation, "page.goto");
  clock = 3;
  steps.begin(capture, "page.screenshot");
  clock = 5;
  steps.end(capture);
  expect(steps.describe()).toContain("completed [page.screenshot 2 ms]");
  expect(steps.describe()).toContain("running [page.goto 5 ms]");
  clock = 9;
  steps.end(navigation);
  expect(steps.describe()).toContain("page.screenshot 2 ms; page.goto 9 ms");
});

test("deadline context bounds active/recent calls and never formats argument-like labels", () => {
  let clock = 0;
  const steps = new ExecutionSteps({ pendingSteps: 1, recentSteps: 1 }, () => clock);
  const first = {};
  steps.begin(first, "https://example.test/?token=secret");
  steps.begin({}, "page.screenshot");
  expect(steps.describe()).toContain("browser.operation");
  expect(steps.describe()).not.toContain("token");
  expect(steps.describe()).not.toContain("page.screenshot");
  clock = 7;
  steps.end(first);
  const next = {};
  steps.begin(next, "page.goto");
  clock = 11;
  steps.end(next);
  expect(steps.describe()).toContain("completed [page.goto 4 ms]");
  expect(steps.describe()).not.toContain("browser.operation");
  expect(steps.describe()).toContain("2 steps omitted");
});

test("late callbacks cannot restore a closed execution's step data", () => {
  const steps = new ExecutionSteps({ pendingSteps: 2, recentSteps: 2 }, () => 1);
  const key = {};
  steps.begin(key, "page.goto");
  steps.close();
  steps.end(key);
  steps.begin({}, "page.screenshot");
  expect(steps.describe()).toBe("Execution steps: completed []; running [].");
});
