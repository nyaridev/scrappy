import { describe, expect, it } from "vitest";
import { formatBytes, formatCount, formatDay, formatDuration, formatWhen, shortVersion } from "./format";

describe("formatBytes", () => {
  it("picks the unit and rounds", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(999)).toBe("999 B");
    expect(formatBytes(1500)).toBe("2 KB");
    expect(formatBytes(45_600_000)).toBe("45.6 MB");
    expect(formatBytes(195_110_184)).toBe("195.1 MB");
    expect(formatBytes(3e15)).toBe("3000.0 TB");
  });
});

describe("shortVersion", () => {
  it("drops build details", () => {
    expect(shortVersion("n9.0.2-39-g7adfbe40dc")).toBe("n9.0.2");
    expect(shortVersion("N-118858-g740d400965-20250318")).toBe("N-118858");
    expect(shortVersion("7.1-full_build-www.gyan.dev")).toBe("7.1");
    expect(shortVersion("1.33.0-dev:2026.11.10")).toBe("1.33.0");
    expect(shortVersion("2026.11.05.233012")).toBe("2026.11.05.233012");
    expect(shortVersion("2.9.8")).toBe("2.9.8");
  });
});

describe("formatDuration", () => {
  it("writes minutes and seconds, and hours when there are some", () => {
    expect(formatDuration(19)).toBe("0:19");
    expect(formatDuration(600)).toBe("10:00");
    expect(formatDuration(3725)).toBe("1:02:05");
  });
});

describe("formatWhen", () => {
  const now = new Date(2026, 10, 10, 15, 0);
  const at = (date: Date) => date.getTime() / 1000;

  it("names recent days and dates older ones", () => {
    expect(formatWhen(at(new Date(2026, 10, 10, 9, 5)), now)).toMatch(/9|09/);
    expect(formatWhen(at(new Date(2026, 10, 9, 23, 59)), now)).toBe("Yesterday");
    expect(formatWhen(at(new Date(2026, 10, 5, 12, 0)), now)).toBe(
      new Date(2026, 10, 5).toLocaleDateString(undefined, { weekday: "long" }),
    );
    expect(formatWhen(at(new Date(2026, 9, 1, 12, 0)), now)).toMatch(/2026/);
  });
});

describe("formatDay", () => {
  it("reads the day in local time", () => {
    expect(formatDay("2026-11-01")).toBe(
      new Date(2026, 10, 1).toLocaleDateString(undefined, { day: "numeric", month: "short" }),
    );
  });
});

describe("formatCount", () => {
  it("counts in singular, plural and open-ended", () => {
    expect(formatCount(1, "file")).toBe("1 file");
    expect(formatCount(40, "video")).toBe("40 videos");
    expect(formatCount(50, "file", true)).toBe("50+ files");
  });
});
