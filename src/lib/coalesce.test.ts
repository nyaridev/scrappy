import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { coalesce } from "./coalesce";

describe("coalesce", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("runs once at once and once after a burst", () => {
    const run = vi.fn();
    const call = coalesce(run, 100);
    for (let n = 0; n < 50; n++) call();
    expect(run).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(100);
    expect(run).toHaveBeenCalledTimes(2);
    vi.advanceTimersByTime(500);
    expect(run).toHaveBeenCalledTimes(2);
    call();
    expect(run).toHaveBeenCalledTimes(3);
  });
});
