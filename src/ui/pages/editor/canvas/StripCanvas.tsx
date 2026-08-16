import { useCallback, useEffect, useRef, useState } from "react";

import type { CatalogueEntry } from "../../../../types/bindings";
import { cx } from "../../../cx";
import * as editor from "../state";
import type { EditorState } from "../state";

import { DesignElementView } from "./DesignElementView";
import { StripField } from "./StripField";
import { useCanvasView, ZOOM_STEP } from "./useCanvasView";

import styles from "./Canvas.module.css";

interface StripCanvasProps {
  state: EditorState;
  catalogue: CatalogueEntry[];
}

interface Drag {
  node: HTMLElement;
  target: editor.Selection;
  startX: number;
  startY: number;
  originMm: { xMm: number; yMm: number };
  moved: boolean;
}

export function StripCanvas({ state, catalogue }: StripCanvasProps) {
  const { canvasRef, scale, strip, view, zoomBy, panBy, reset } = useCanvasView(
    state.document.size,
  );
  const drag = useRef<Drag | null>(null);
  const pan = useRef<{ x: number; y: number } | null>(null);
  const frame = useRef(0);
  const [spaceHeld, setSpaceHeld] = useState(false);

  const labels = new Map(catalogue.map((entry) => [entry.key, entry.label]));

  const selectionCentre = useCallback(() => {
    const found = locate(state, state.selection);
    if (!found) return undefined;
    const pxPerMm = scale * view.zoom;
    return {
      x: view.panX + found.xMm * pxPerMm,
      y: view.panY + found.yMm * pxPerMm,
    };
  }, [state, scale, view]);

  // Wheel zoom is cursor-anchored: the point under the pointer stays under the pointer.
  const onWheel = useCallback(
    (event: React.WheelEvent<HTMLDivElement>) => {
      const box = canvasRef.current?.getBoundingClientRect();
      if (!box) return;
      zoomBy(event.deltaY < 0 ? ZOOM_STEP : 1 / ZOOM_STEP, {
        x: event.clientX - box.left,
        y: event.clientY - box.top,
      });
    },
    [canvasRef, zoomBy],
  );

  const onPointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const box = canvasRef.current?.getBoundingClientRect();
      if (!box) return;
      canvasRef.current?.focus();

      const node = (event.target as HTMLElement).closest<HTMLElement>("[data-drag]");
      const wantsPan = event.button === 1 || spaceHeld || !node;
      if (wantsPan) {
        pan.current = { x: event.clientX, y: event.clientY };
        event.currentTarget.setPointerCapture(event.pointerId);
        if (!node) editor.select(null);
        return;
      }

      const target = describe(node);
      if (!target) return;
      const origin = locate(state, target);
      if (!origin) return;

      editor.select(target);
      editor.beginGesture();
      drag.current = {
        node,
        target,
        startX: event.clientX,
        startY: event.clientY,
        originMm: origin,
        moved: false,
      };
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    [canvasRef, spaceHeld, state],
  );

  // Pointer moves write a transform straight to the node inside a frame; no React state, and
  // the millimetre commit happens once on release.
  const onPointerMove = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const panning = pan.current;
      if (panning) {
        const dx = event.clientX - panning.x;
        const dy = event.clientY - panning.y;
        pan.current = { x: event.clientX, y: event.clientY };
        panBy(dx, dy);
        return;
      }

      const active = drag.current;
      if (!active) return;
      active.moved = true;
      const pxPerMm = scale * view.zoom;
      if (pxPerMm <= 0) return;
      const dxMm = (event.clientX - active.startX) / pxPerMm;
      const dyMm = (event.clientY - active.startY) / pxPerMm;

      cancelAnimationFrame(frame.current);
      frame.current = requestAnimationFrame(() => {
        active.node.style.transform = `translate3d(${String(dxMm * scale)}px, ${String(dyMm * scale)}px, 0)`;
      });
    },
    [panBy, scale, view.zoom],
  );

  const onPointerUp = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      event.currentTarget.releasePointerCapture(event.pointerId);
      pan.current = null;

      const active = drag.current;
      drag.current = null;
      if (!active) return;

      cancelAnimationFrame(frame.current);
      active.node.style.transform = "";
      if (active.moved) {
        const pxPerMm = scale * view.zoom;
        const xMm = active.originMm.xMm + (event.clientX - active.startX) / pxPerMm;
        const yMm = active.originMm.yMm + (event.clientY - active.startY) / pxPerMm;
        if (active.target.kind === "placement") {
          editor.movePlacement(active.target.fieldKey, active.target.id, xMm, yMm);
        } else {
          editor.moveElement(active.target.id, xMm, yMm);
        }
      }
      editor.endGesture();
    },
    [scale, view.zoom],
  );

  const onKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.key === " ") {
        setSpaceHeld(true);
        return;
      }
      if (event.ctrlKey || event.metaKey) {
        if (event.key === "=" || event.key === "+") {
          event.preventDefault();
          zoomBy(ZOOM_STEP, selectionCentre());
        } else if (event.key === "-") {
          event.preventDefault();
          zoomBy(1 / ZOOM_STEP, selectionCentre());
        } else if (event.key === "0") {
          event.preventDefault();
          reset();
        }
        return;
      }
      if (event.key === "Delete" || event.key === "Backspace") {
        event.preventDefault();
        editor.removeSelected();
        return;
      }

      const step = event.shiftKey ? editor.FINE_NUDGE_MM : editor.NUDGE_MM;
      const nudge: Record<string, [number, number]> = {
        ArrowLeft: [-step, 0],
        ArrowRight: [step, 0],
        ArrowUp: [0, -step],
        ArrowDown: [0, step],
      };
      const delta = nudge[event.key];
      const target = state.selection;
      if (!delta || !target) return;
      const origin = locate(state, target);
      if (!origin) return;

      event.preventDefault();
      const [dx, dy] = delta;
      if (target.kind === "placement") {
        editor.movePlacement(target.fieldKey, target.id, origin.xMm + dx, origin.yMm + dy);
      } else {
        editor.moveElement(target.id, origin.xMm + dx, origin.yMm + dy);
      }
    },
    [reset, selectionCentre, state, zoomBy],
  );

  useEffect(() => {
    const release = (event: KeyboardEvent) => {
      if (event.key === " ") setSpaceHeld(false);
    };
    window.addEventListener("keyup", release);
    return () => {
      window.removeEventListener("keyup", release);
    };
  }, []);

  return (
    <div
      ref={canvasRef}
      className={cx(styles.canvas, spaceHeld && styles.panning)}
      tabIndex={0}
      role="application"
      onWheel={onWheel}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onKeyDown={onKeyDown}
      onDoubleClick={reset}
    >
      <div
        className={styles.strip}
        style={{
          width: `${String(strip.width)}px`,
          height: `${String(strip.height)}px`,
          transform: `translate3d(${String(view.panX)}px, ${String(view.panY)}px, 0) scale(${String(view.zoom)})`,
        }}
      >
        {state.document.fields.flatMap((field) =>
          field.placements.map((placement) => (
            <StripField
              key={placement.id}
              fieldKey={field.key}
              label={labels.get(field.key) ?? field.key}
              placement={placement}
              fontSizePt={field.fontSizePt}
              scale={scale}
              selected={
                state.selection?.kind === "placement" && state.selection.id === placement.id
              }
            />
          )),
        )}
        {state.document.elements.map((element) => (
          <DesignElementView
            key={element.id}
            element={element}
            scale={scale}
            selected={state.selection?.kind === "element" && state.selection.id === element.id}
          />
        ))}
      </div>
    </div>
  );
}

function describe(node: HTMLElement): editor.Selection | null {
  const id = node.dataset["id"];
  if (!id) return null;
  if (node.dataset["drag"] === "placement") {
    const fieldKey = node.dataset["field"];
    return fieldKey ? { kind: "placement", fieldKey, id } : null;
  }
  return { kind: "element", id };
}

function locate(
  state: EditorState,
  target: editor.Selection | null,
): { xMm: number; yMm: number } | undefined {
  if (!target) return undefined;
  if (target.kind === "element") {
    return state.document.elements.find((element) => element.id === target.id);
  }
  return state.document.fields
    .find((field) => field.key === target.fieldKey)
    ?.placements.find((placement) => placement.id === target.id);
}
