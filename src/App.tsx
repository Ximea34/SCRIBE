import { useState } from "react";

import type { Tab } from "./app/router";
import { Titlebar } from "./ui/Titlebar";
import { EditorPage } from "./ui/pages/EditorPage";
import { IfrPage } from "./ui/pages/IfrPage";
import { OptionsPage } from "./ui/pages/OptionsPage";
import { VigiePage } from "./ui/pages/VigiePage";

import styles from "./App.module.css";

// Only the active tab is mounted, so a hidden tab holds no subscriptions and renders nothing.
function Page({ tab }: { tab: Tab }) {
  switch (tab) {
    case "vigie":
      return <VigiePage />;
    case "ifr":
      return <IfrPage />;
    case "editeur":
      return <EditorPage />;
    case "options":
      return <OptionsPage />;
  }
}

export function App() {
  const [tab, setTab] = useState<Tab>("vigie");

  return (
    <div className={styles.window}>
      <Titlebar activeTab={tab} onSelectTab={setTab} />
      <main className={styles.content} role="tabpanel" aria-labelledby={`tab-${tab}`}>
        <Page tab={tab} />
      </main>
    </div>
  );
}
