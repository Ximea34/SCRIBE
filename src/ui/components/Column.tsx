import type { ReactNode } from "react";

import { cx } from "../cx";

import styles from "./Column.module.css";

interface ColumnProps {
  label: string;
  split?: boolean;
  children: ReactNode;
}

// One board column: a header band, a 13 px gap, then the bordered body.
export function Column({ label, split = false, children }: ColumnProps) {
  return (
    <section className={styles.column}>
      <h2 className={styles.header}>
        <span className={styles.label}>{label}</span>
      </h2>
      <div className={cx(styles.body, split && styles.split)}>{children}</div>
    </section>
  );
}

interface PaneProps {
  children?: ReactNode;
}

// A scrolling area for strips. Overflow is the normal case, not an edge case.
export function Pane({ children }: PaneProps) {
  return <div className={styles.pane}>{children}</div>;
}
