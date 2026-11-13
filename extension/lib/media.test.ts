import { describe, expect, it } from "vitest";
import { bestStream, type Found, sizeGuess } from "./media";

const video = (url: string, format: string | null, size?: number): Found => ({ url, kind: "video", format, size });

describe("bestStream", () => {
  it("takes the first playlist", () => {
    const found = [video("a.mp4", "MP4", 9), video("m.m3u8", "HLS"), video("v.m3u8", "HLS")];
    expect(bestStream(found)?.url).toBe("m.m3u8");
  });

  it("else the largest file, never an image", () => {
    const found = [video("s.mp4", "MP4", 5), video("b.mp4", "MP4", 50), { url: "i.jpg", kind: "image", format: "JPG" }];
    expect(bestStream(found as Found[])?.url).toBe("b.mp4");
  });

  it("finds none in an empty list", () => {
    expect(bestStream([])).toBeUndefined();
  });
});

describe("sizeGuess", () => {
  it("takes the known size, else guesses from pixels and length", () => {
    expect(sizeGuess(video("a.mp4", "MP4", 9))).toEqual({ bytes: 9, estimated: false });
    const image: Found = { url: "i.jpg", kind: "image", format: "JPG", width: 1000, height: 1000 };
    expect(sizeGuess(image)).toEqual({ bytes: 250_000, estimated: true });
    const clip: Found = { ...video("c.mp4", "MP4"), width: 1920, height: 1080, duration: 10 };
    expect(sizeGuess(clip)?.bytes).toBeCloseTo(7_776_000);
  });

  it("has nothing to go on without a size or pixels", () => {
    expect(sizeGuess(video("m.m3u8", "HLS"))).toBeNull();
    expect(sizeGuess({ ...video("v.mp4", "MP4"), width: 640, height: 360 })).toBeNull();
  });
});
