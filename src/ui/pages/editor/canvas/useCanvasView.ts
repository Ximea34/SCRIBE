import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { StripSize } from "../../../../types/bindings";

import { centred, clampPan, fitScale, zoomAt, ZOOM_STEP, type Box, type View } from "./view";

export interface CanvasView {
  canvasRef: React.RefObject<HTMLDivElement | null>;
  canvas: Box;
  scale: number;
  strip: Box;
  view: View;
  zoomBy: (factor: number, anchor?: { x: number; y: number }) => void;
  panBy: (dx: number, dy: number) => void;
  reset: () => void;
}

export function useCanvasView(size: StripSize): CanvasView {
  const canvasRef = useRef<HTMLDivElement>(null);
  const [canvas, setCanvas] = useState<Box>({ width: 0, height: 0 });
  const [view, setView] = useState<View>({ zoom: 1, panX: 0, panY: 0 });

  useEffect(() => {
    const element = canvasRef.current;
    if (!element) return undefined;

    const observer = new ResizeObserver(() => {
      setCanvas({ width: element.clientWidth, height: element.clientHeight });
    });
    observer.observe(element);
    setCanvas({ width: element.clientWidth, height: element.clientHeight });
    return () => {
      observer.disconnect();
    };
  }, []);

  const scale = useMemo(() => fitScale(canvas, size), [canvas, size]);
  const strip = useMemo<Box>(
    () => ({ width: size.lengthMm * scale, height: size.widthMm * scale }),
    [size, scale],
  );

  // Re-centre whenever the canvas or the strip's proportions change.
  useEffect(() => {
    if (strip.width <= 0) return;
    setView(centred(canvas, strip));
  }, [canvas, strip]);

  const zoomBy = useCallback(
    (factor: number, anchor?: { x: number; y: number }) => {
      setView((current) => {
        const point = anchor ?? { x: canvas.width / 2, y: canvas.height / 2 };
        return clampPan(zoomAt(current, current.zoom * factor, point.x, point.y), canvas, strip);
      });
    },
    [canvas, strip],
  );

  const panBy = useCallback(
    (dx: number, dy: number) => {
      setView((current) =>
        clampPan({ ...current, panX: current.panX + dx, panY: current.panY + dy }, canvas, strip),
      );
    },
    [canvas, strip],
  );

  const reset = useCallback(() => {
    setView(centred(canvas, strip));
  }, [canvas, strip]);

  return { canvasRef, canvas, scale, strip, view, zoomBy, panBy, reset };
}

export { ZOOM_STEP };
