import type { StripSize } from "../../../../types/bindings";

export interface Box {
  width: number;
  height: number;
}

export interface View {
  zoom: number;
  panX: number;
  panY: number;
}

export const MIN_ZOOM = 1;
export const MAX_ZOOM = 8;
export const ZOOM_STEP = 1.25;
/// 65 px at the 1406 px reference canvas.
export const MARGIN_RATIO = 65 / 1406;
export const MM_PER_POINT = 0.3528;

/// Derived from the canvas's measured size, never from `--s`, which knows nothing about the
/// panel's real layout.
export function fitScale(canvas: Box, size: StripSize): number {
  if (canvas.width <= 0 || canvas.height <= 0) return 0;
  const margin = canvas.width * MARGIN_RATIO;
  const usableWidth = Math.max(1, canvas.width - margin * 2);
  const usableHeight = Math.max(1, canvas.height - margin * 2);
  return Math.min(usableWidth / size.lengthMm, usableHeight / size.widthMm);
}

export function clampZoom(zoom: number): number {
  if (!Number.isFinite(zoom)) return MIN_ZOOM;
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom));
}

/// Keeps the anchor point under the pointer — the cursor for wheel zoom, the selection's centre
/// for keyboard zoom.
export function zoomAt(view: View, nextZoom: number, anchorX: number, anchorY: number): View {
  const zoom = clampZoom(nextZoom);
  const ratio = zoom / view.zoom;
  return {
    zoom,
    panX: anchorX - (anchorX - view.panX) * ratio,
    panY: anchorY - (anchorY - view.panY) * ratio,
  };
}

export function centred(canvas: Box, strip: Box): View {
  return {
    zoom: MIN_ZOOM,
    panX: (canvas.width - strip.width) / 2,
    panY: (canvas.height - strip.height) / 2,
  };
}

/// The strip can never be lost off-canvas: when it is smaller than the canvas it stays wholly
/// inside, and when it is larger it always covers the canvas.
export function clampPan(view: View, canvas: Box, strip: Box): View {
  const scaled = { width: strip.width * view.zoom, height: strip.height * view.zoom };
  const axis = (pan: number, available: number, extent: number) =>
    extent <= available
      ? Math.min(Math.max(pan, 0), available - extent)
      : Math.min(Math.max(pan, available - extent), 0);

  return {
    zoom: view.zoom,
    panX: axis(view.panX, canvas.width, scaled.width),
    panY: axis(view.panY, canvas.height, scaled.height),
  };
}

/// Canvas point to strip millimetres, for hit testing and drag maths.
export function toMillimetres(
  view: View,
  scale: number,
  canvasX: number,
  canvasY: number,
): { xMm: number; yMm: number } {
  const pxPerMm = scale * view.zoom;
  if (pxPerMm <= 0) return { xMm: 0, yMm: 0 };
  return {
    xMm: (canvasX - view.panX) / pxPerMm,
    yMm: (canvasY - view.panY) / pxPerMm,
  };
}

/// On-canvas text size, before the container's zoom transform is applied.
export function pointsToPixels(fontSizePt: number, scale: number): number {
  return fontSizePt * MM_PER_POINT * scale;
}
