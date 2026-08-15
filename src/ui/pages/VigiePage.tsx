import { LABELS } from "../../app/labels";
import { Column, Pane } from "../components/Column";

import styles from "./VigiePage.module.css";

export function VigiePage() {
  return (
    <div className={styles.board}>
      <Column label={LABELS.columns.awake}>
        <Pane />
      </Column>

      <Column label={LABELS.columns.activated} split>
        <Pane />
        <Pane />
      </Column>

      <Column label={LABELS.columns.transits}>
        <Pane />
      </Column>
    </div>
  );
}
