import { LABELS } from "../app/labels";
import { LEFT_TABS, tabLabel, type Tab } from "../app/router";

import { cx } from "./cx";

import styles from "./Tabs.module.css";

interface TabsProps {
  active: Tab;
  onSelect: (tab: Tab) => void;
}

interface TabButtonProps extends TabsProps {
  tab: Tab;
  narrow?: boolean;
}

function TabButton({ tab, active, narrow, onSelect }: TabButtonProps) {
  const selected = tab === active;
  return (
    <button
      type="button"
      role="tab"
      id={`tab-${tab}`}
      aria-selected={selected}
      className={cx(styles.tab, selected && styles.active, narrow && styles.narrow)}
      onClick={() => {
        onSelect(tab);
      }}
    >
      {tabLabel(tab)}
    </button>
  );
}

export function Tabs({ active, onSelect }: TabsProps) {
  // Tauri only drags when the event target itself carries the attribute, so every part of the
  // bar that is not a button has to opt in — including the gap before OPTIONS.
  return (
    <nav
      className={styles.tabs}
      role="tablist"
      aria-label={LABELS.window.navigation}
      data-tauri-drag-region
    >
      {LEFT_TABS.map((tab) => (
        <TabButton key={tab} tab={tab} active={active} onSelect={onSelect} />
      ))}
      <span className={styles.spacer} aria-hidden="true" data-tauri-drag-region />
      <TabButton tab="options" active={active} onSelect={onSelect} narrow />
    </nav>
  );
}
