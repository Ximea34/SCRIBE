import { useCallback, useEffect, useRef } from "react";

import { LABELS } from "../../../../app/labels";
import { SquareMinus, SquarePlus } from "../../../icons/Icons";
import * as editor from "../state";

import styles from "./Panel.module.css";

const REPEAT_DELAY_MS = 350;
const REPEAT_EVERY_MS = 70;

interface SizeStepperProps {
  value: number;
  min: number;
  max: number;
  step?: number;
  largeStep?: number;
  onChange: (next: number) => void;
  format?: ((value: number) => string) | undefined;
}

// Shift steps by ten, and holding repeats — the whole run collapses into one undo entry.
export function SizeStepper({
  value,
  min,
  max,
  step = 1,
  largeStep,
  onChange,
  format,
}: SizeStepperProps) {
  const timers = useRef<{ delay?: number; repeat?: number }>({});

  const stop = useCallback(() => {
    window.clearTimeout(timers.current.delay);
    window.clearInterval(timers.current.repeat);
    timers.current = {};
    editor.endGesture();
  }, []);

  useEffect(() => stop, [stop]);

  const start = useCallback(
    (direction: number, shift: boolean) => {
      const size = shift ? (largeStep ?? step * editor.FONT_STEP_LARGE) : step;
      editor.beginGesture();
      let current = value;
      const apply = () => {
        // Rounded so a 0.1 mm thickness never drifts into 0.30000000000000004.
        const next = Math.min(max, Math.max(min, current + direction * size));
        current = Number(next.toFixed(2));
        onChange(current);
      };
      apply();
      timers.current.delay = window.setTimeout(() => {
        timers.current.repeat = window.setInterval(apply, REPEAT_EVERY_MS);
      }, REPEAT_DELAY_MS);
    },
    [largeStep, max, min, onChange, step, value],
  );

  return (
    <div className={styles.stepper}>
      <button
        type="button"
        className={styles.step}
        aria-label={LABELS.editor.decrease}
        disabled={value <= min}
        onPointerDown={(event) => {
          start(-1, event.shiftKey);
        }}
        onPointerUp={stop}
        onPointerLeave={stop}
      >
        <SquareMinus />
      </button>

      <span className={styles.value}>{format ? format(value) : value}</span>

      <button
        type="button"
        className={styles.step}
        aria-label={LABELS.editor.increase}
        disabled={value >= max}
        onPointerDown={(event) => {
          start(1, event.shiftKey);
        }}
        onPointerUp={stop}
        onPointerLeave={stop}
      >
        <SquarePlus />
      </button>
    </div>
  );
}
