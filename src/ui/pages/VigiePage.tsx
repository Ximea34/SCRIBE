import { useCallback, useState } from "react";

import { activateFlight } from "../../app/ipc";
import { LABELS } from "../../app/labels";
import { useBoard } from "../../hooks/useBoard";
import { ActivationDialog, ACTIVATION_TITLE_ID } from "../components/ActivationDialog";
import { Column, Pane } from "../components/Column";
import { Modal } from "../components/Modal";
import { StripList } from "../components/StripList";

import styles from "./VigiePage.module.css";

export function VigiePage() {
  useBoard();
  const [pending, setPending] = useState<string | null>(null);

  // Stable identities, so handing them to a memoised strip costs nothing.
  const open = useCallback((callsign: string) => {
    setPending(callsign);
  }, []);
  const cancel = useCallback(() => {
    setPending(null);
  }, []);
  const confirm = useCallback(() => {
    if (pending === null) return;
    const callsign = pending;
    setPending(null);
    // The board moves itself when the engine emits the resulting diff.
    activateFlight(callsign).catch((error: unknown) => {
      console.error("activation failed", error);
    });
  }, [pending]);

  return (
    <div className={styles.board}>
      <Column label={LABELS.columns.awake}>
        <Pane>
          <StripList column="awake" onSelect={open} />
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

      {pending !== null && (
        <Modal labelledBy={ACTIVATION_TITLE_ID} onCancel={cancel} onConfirm={confirm}>
          <ActivationDialog callsign={pending} onCancel={cancel} onConfirm={confirm} />
        </Modal>
      )}
    </div>
  );
}
