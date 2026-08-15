import { LABELS } from "../../app/labels";
import { useBoard } from "../../hooks/useBoard";
import { Column, Pane } from "../components/Column";
import { StripList } from "../components/StripList";

import styles from "./VigiePage.module.css";

export function VigiePage() {
  useBoard();

  return (
    <div className={styles.board}>
      <Column label={LABELS.columns.awake}>
        <Pane>
          <StripList column="awake" />
        </Pane>
      </Column>

      <Column label={LABELS.columns.activated} split>
        <Pane>
          <StripList column="activatedDepartures" />
        </Pane>
        <Pane>
          <StripList column="arrivals" />
        </Pane>
      </Column>

      <Column label={LABELS.columns.transits}>
        <Pane>
          <StripList column="transits" />
        </Pane>
      </Column>
    </div>
  );
}
