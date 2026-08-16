import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";

import { LABELS } from "../../app/labels";
import { ArrowBadgeUp } from "../icons/Icons";
import { cx } from "../cx";

import styles from "./Scrollbar.module.css";

const ARROW_STEP_PX = 40;
const MIN_THUMB_PX = 20;

interface Metrics {
  thumbTop: number;
  thumbHeight: number;
  scrollable: boolean;
}

// Native CSS scrollbars cannot render arrow buttons portably, so the chrome is drawn over a
// natively scrollable container with the native bar hidden.
export function Scrollbar({
  children,
  className,
}: {
  children: ReactNode;
  className?: string | undefined;
}) {
  const viewport = useRef<HTMLDivElement>(null);
  const track = useRef<HTMLDivElement>(null);
  const [metrics, setMetrics] = useState<Metrics>({
    thumbTop: 0,
    thumbHeight: 0,
    scrollable: false,
  });

  const measure = useCallback(() => {
    const view = viewport.current;
    const rail = track.current;
    if (!view || !rail) return;

    const trackHeight = rail.clientHeight;
    const overflow = view.scrollHeight - view.clientHeight;
    if (overflow <= 0 || trackHeight <= 0) {
      setMetrics({ thumbTop: 0, thumbHeight: 0, scrollable: false });
      return;
    }
    const ratio = view.clientHeight / view.scrollHeight;
    const thumbHeight = Math.max(MIN_THUMB_PX, trackHeight * ratio);
    const thumbTop = (view.scrollTop / overflow) * (trackHeight - thumbHeight);
    setMetrics({ thumbTop, thumbHeight, scrollable: true });
  }, []);

  useEffect(() => {
    const view = viewport.current;
    if (!view) return undefined;

    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(view);
    for (const child of view.children) observer.observe(child);
    view.addEventListener("scroll", measure, { passive: true });

    return () => {
      observer.disconnect();
      view.removeEventListener("scroll", measure);
    };
  }, [measure]);

  const scrollBy = useCallback((delta: number) => {
    viewport.current?.scrollBy({ top: delta });
  }, []);

  const onThumbDown = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    const view = viewport.current;
    const rail = track.current;
    if (!view || !rail) return;

    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    const startY = event.clientY;
    const startScroll = view.scrollTop;
    const overflow = view.scrollHeight - view.clientHeight;
    const travel = rail.clientHeight - event.currentTarget.clientHeight;

    const onMove = (move: PointerEvent) => {
      if (travel <= 0) return;
      view.scrollTop = startScroll + ((move.clientY - startY) / travel) * overflow;
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }, []);

  return (
    <div className={cx(styles.shell, className)}>
      <div ref={viewport} className={styles.viewport}>
        {children}
      </div>
      <div className={styles.bar}>
        <button
          type="button"
          className={styles.arrow}
          aria-label={LABELS.editor.scrollUp}
          onClick={() => {
            scrollBy(-ARROW_STEP_PX);
          }}
        >
          <ArrowBadgeUp />
        </button>

        <div ref={track} className={styles.track}>
          <div
            className={cx(styles.thumb, !metrics.scrollable && styles.hidden)}
            style={{
              top: `${String(metrics.thumbTop)}px`,
              height: `${String(metrics.thumbHeight)}px`,
            }}
            onPointerDown={onThumbDown}
          />
        </div>

        <button
          type="button"
          className={cx(styles.arrow, styles.down)}
          aria-label={LABELS.editor.scrollDown}
          onClick={() => {
            scrollBy(ARROW_STEP_PX);
          }}
        >
          <ArrowBadgeUp />
        </button>
      </div>
    </div>
  );
}
