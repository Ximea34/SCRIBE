import { getCurrentWindow } from "@tauri-apps/api/window";

import { LABELS } from "../app/labels";

import { cx } from "./cx";

import styles from "./WindowControls.module.css";

// Drawn rather than exported: at 24 px these are three strokes, and inline SVG stays crisp at
// every scale factor and follows the text colour.
const STROKE = { fill: "none", stroke: "currentColor", strokeWidth: 1.25 } as const;

function run(action: () => Promise<unknown>): void {
  action().catch((error: unknown) => {
    console.error("window control failed", error);
  });
}

const minimize = () => {
  run(() => getCurrentWindow().minimize());
};
const toggleMaximize = () => {
  run(() => getCurrentWindow().toggleMaximize());
};
const close = () => {
  run(() => getCurrentWindow().close());
};

export function WindowControls() {
  return (
    <div className={styles.controls} data-tauri-drag-region>
      <button
        type="button"
        className={styles.control}
        aria-label={LABELS.window.minimize}
        onClick={minimize}
      >
        <svg className={styles.icon} viewBox="0 0 24 24" aria-hidden="true">
          <line x1="7" y1="12" x2="17" y2="12" {...STROKE} />
        </svg>
      </button>

      <button
        type="button"
        className={styles.control}
        aria-label={LABELS.window.maximize}
        onClick={toggleMaximize}
      >
        <svg className={styles.icon} viewBox="0 0 24 24" aria-hidden="true">
          <rect x="8" y="8" width="8" height="8" {...STROKE} />
        </svg>
      </button>

      <button
        type="button"
        className={cx(styles.control, styles.close)}
        aria-label={LABELS.window.close}
        onClick={close}
      >
        <svg className={styles.icon} viewBox="0 0 24 24" aria-hidden="true">
          <path d="M7 7 L17 17 M17 7 L7 17" {...STROKE} />
        </svg>
      </button>
    </div>
  );
}
