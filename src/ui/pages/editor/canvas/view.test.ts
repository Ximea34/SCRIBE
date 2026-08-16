import { describe, expect, it } from "vitest";

import {
  centred,
  clampPan,
  clampZoom,
  fitScale,
  MAX_ZOOM,
  MIN_ZOOM,
  pointsToPixels,
  toMillimetres,
  zoomAt,
} from "./view";

const CANVAS = { width: 1406, height: 528 };
const STRIP = { lengthMm: 203, widthMm: 25 };

describe("fit scale", () => {
  it("leaves the documented margin on the constraining axis", () => {
    const scale = fitScale(CANVAS, STRIP);
    const margin = CANVAS.width * (65 / 1406);
    expect(scale * STRIP.lengthMm).toBeCloseTo(CANVAS.width - margin * 2, 6);
  });

  it("keeps the strip inside the canvas on both axes", () => {
    const scale = fitScale(CANVAS, STRIP);
    expect(scale * STRIP.lengthMm).toBeLessThanOrEqual(CANVAS.width);
    expect(scale * STRIP.widthMm).toBeLessThanOrEqual(CANVAS.height);
  });

  it("honours the millimetre ratio rather than the illustrative Figma rectangle", () => {
    const scale = fitScale(CANVAS, STRIP);
    const rendered = (scale * STRIP.lengthMm) / (scale * STRIP.widthMm);
    expect(rendered).toBeCloseTo(STRIP.lengthMm / STRIP.widthMm, 9);
    expect(rendered).not.toBeCloseTo(1275 / 162, 2);
  });

  it("is zero for an unmeasured canvas rather than infinite", () => {
    expect(fitScale({ width: 0, height: 0 }, STRIP)).toBe(0);
  });
});

describe("zoom", () => {
  it("stays between 1x and 8x", () => {
    expect(clampZoom(0.2)).toBe(MIN_ZOOM);
    expect(clampZoom(99)).toBe(MAX_ZOOM);
    expect(clampZoom(Number.NaN)).toBe(MIN_ZOOM);
  });

  it("keeps the anchor point exactly under the cursor", () => {
    const before = { zoom: 1, panX: 100, panY: 40 };
    const anchor = { x: 500, y: 200 };
    const after = zoomAt(before, 2.5, anchor.x, anchor.y);

    const contentBefore = (anchor.x - before.panX) / before.zoom;
    const contentAfter = (anchor.x - after.panX) / after.zoom;
    expect(contentAfter).toBeCloseTo(contentBefore, 9);

    const verticalBefore = (anchor.y - before.panY) / before.zoom;
    const verticalAfter = (anchor.y - after.panY) / after.zoom;
    expect(verticalAfter).toBeCloseTo(verticalBefore, 9);
  });

  it("holds the anchor across a zoom in and back out", () => {
    const start = { zoom: 1, panX: 0, panY: 0 };
    const inward = zoomAt(start, 4, 300, 120);
    const outward = zoomAt(inward, 1, 300, 120);
    expect(outward.panX).toBeCloseTo(start.panX, 9);
    expect(outward.panY).toBeCloseTo(start.panY, 9);
  });

  it("refuses to exceed the range even when asked", () => {
    expect(zoomAt({ zoom: 4, panX: 0, panY: 0 }, 100, 0, 0).zoom).toBe(MAX_ZOOM);
  });
});

describe("pan", () => {
  const strip = { width: 1276, height: 157 };

  it("centres the strip at fit scale", () => {
    const view = centred(CANVAS, strip);
    expect(view.zoom).toBe(MIN_ZOOM);
    expect(view.panX).toBeCloseTo((CANVAS.width - strip.width) / 2, 9);
    expect(view.panY).toBeCloseTo((CANVAS.height - strip.height) / 2, 9);
  });

  it("keeps a smaller strip wholly inside the canvas", () => {
    const pushed = clampPan({ zoom: 1, panX: 9000, panY: -9000 }, CANVAS, strip);
    expect(pushed.panX).toBeGreaterThanOrEqual(0);
    expect(pushed.panX).toBeLessThanOrEqual(CANVAS.width - strip.width);
    expect(pushed.panY).toBeGreaterThanOrEqual(0);
    expect(pushed.panY).toBeLessThanOrEqual(CANVAS.height - strip.height);
  });

  it("keeps a zoomed strip covering the canvas so it is never lost", () => {
    const view = clampPan({ zoom: 8, panX: 9000, panY: 9000 }, CANVAS, strip);
    expect(view.panX).toBeLessThanOrEqual(0);
    expect(view.panX).toBeGreaterThanOrEqual(CANVAS.width - strip.width * 8);
    expect(view.panY).toBeLessThanOrEqual(0);
  });
});

describe("units", () => {
  it("maps a canvas point back to millimetres", () => {
    const view = { zoom: 2, panX: 50, panY: 10 };
    const scale = 4;
    const { xMm, yMm } = toMillimetres(view, scale, 50 + 8 * 12, 10 + 8 * 3);
    expect(xMm).toBeCloseTo(12, 9);
    expect(yMm).toBeCloseTo(3, 9);
  });

  it("converts points to pixels through millimetres", () => {
    expect(pointsToPixels(12, 1)).toBeCloseTo(12 * 0.3528, 9);
  });
});
