import { useEffect, useState } from "react";

import { LABELS } from "../../app/labels";
import { fetchFlightDetail } from "../../app/ipc";
import { truncate } from "../../app/truncate";
import type { FlightDetail } from "../../types/bindings";

import styles from "./ActivationDialog.module.css";

// STUB — content only, no visual design. The real Figma replaces this one file; the focus trap,
// keyboard handling and the activation call all live outside it.
export const ACTIVATION_TITLE_ID = "activation-title";

// The dialog is unstyled, so this is simply a sane ceiling for the one free-text field.
const ROUTE_BUDGET_EM = 46;

interface ActivationDialogProps {
  callsign: string;
  onCancel: () => void;
  onConfirm: () => void;
}

export function ActivationDialog({ callsign, onCancel, onConfirm }: ActivationDialogProps) {
  const [detail, setDetail] = useState<FlightDetail | null>(null);

  useEffect(() => {
    let cancelled = false;
    setDetail(null);
    fetchFlightDetail(callsign)
      .then((loaded) => {
        if (!cancelled) setDetail(loaded);
      })
      .catch((error: unknown) => {
        console.error("cannot load the flight detail", error);
      });
    return () => {
      cancelled = true;
    };
  }, [callsign]);

  const rows: [string, string][] = detail
    ? [
        [LABELS.activation.aircraft, detail.aircraft],
        [LABELS.activation.wake, detail.wake],
        [LABELS.activation.rules, detail.rules],
        [LABELS.activation.flightType, detail.flightType],
        [LABELS.activation.dep, detail.dep],
        [LABELS.activation.arr, detail.arr],
        [LABELS.activation.alternate, detail.alternate],
        [LABELS.activation.eobt, detail.eobt],
        [LABELS.activation.cruiseLevel, detail.cruiseLevel],
        [LABELS.activation.squawk, detail.squawk],
        [LABELS.activation.assumedBy, detail.assumedBy],
        [LABELS.activation.stand, detail.stand],
        [LABELS.activation.route, truncate(detail.route, ROUTE_BUDGET_EM, 0)],
      ]
    : [];

  return (
    <div data-stub="activation-dialog">
      <h2 id={ACTIVATION_TITLE_ID} className={styles.title}>
        {LABELS.activation.title} — {callsign}
      </h2>

      {detail ? (
        <dl className={styles.fields}>
          {rows.map(([label, value]) => (
            <div key={label} className={styles.row}>
              <dt className={styles.label}>{label}</dt>
              <dd className={styles.value}>{value || "—"}</dd>
            </div>
          ))}
        </dl>
      ) : (
        <p>{LABELS.activation.loading}</p>
      )}

      <div className={styles.actions}>
        <button type="button" onClick={onCancel}>
          {LABELS.activation.cancel}
        </button>
        <button type="button" onClick={onConfirm} disabled={!detail}>
          {LABELS.activation.confirm}
        </button>
      </div>
    </div>
  );
}
