import type { Tab } from "../app/router";

import { Tabs } from "./Tabs";
import { WindowControls } from "./WindowControls";

import styles from "./Titlebar.module.css";

interface TitlebarProps {
  activeTab: Tab;
  onSelectTab: (tab: Tab) => void;
}

// The drag region sits on the bar itself; Tauri only drags when the event target carries the
// attribute, so the tabs and window controls stay clickable.
export function Titlebar({ activeTab, onSelectTab }: TitlebarProps) {
  return (
    <header className={styles.titlebar} data-tauri-drag-region>
      <Tabs active={activeTab} onSelect={onSelectTab} />
      <WindowControls />
    </header>
  );
}
