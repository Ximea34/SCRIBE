import { memo } from "react";

import { truncate } from "../../app/truncate";
import { useStrip } from "../../hooks/useStoreSlice";

import styles from "./Strip.module.css";

const STRIP_WIDTH = 592;
const STRIP_FONT = 36;
const TRACKING_EM = 0.1;

interface Cell {
  style: { left: string; maxWidth: string };
  budgetEm: number;
}

// Cells are centred on fixed positions, so each one owns half the distance to its nearer
// neighbour boundary. Expressed as percentages and em, both of which survive any scale factor.
function cell(centre: number, budget: number): Cell {
  return {
    style: {
      left: `${((centre / STRIP_WIDTH) * 100).toFixed(3)}%`,
      maxWidth: `${(budget / STRIP_FONT).toFixed(3)}em`,
    },
    budgetEm: budget / STRIP_FONT,
  };
}

const CALLSIGN = cell(116, 179.5);
const ADEP = cell(295.5, 113);
const ADES = cell(408.5, 113);
const RULES = cell(531, 122);

export const Strip = memo(function Strip({ callsign }: { callsign: string }) {
  const view = useStrip(callsign);
  if (!view) return null;

  return (
    <div className={styles.strip}>
      <span className={styles.cell} style={CALLSIGN.style}>
        {truncate(view.callsign, CALLSIGN.budgetEm, TRACKING_EM)}
      </span>
      <span className={styles.cell} style={ADEP.style}>
        {truncate(view.adep, ADEP.budgetEm, TRACKING_EM)}
      </span>
      <span className={styles.cell} style={ADES.style}>
        {truncate(view.ades, ADES.budgetEm, TRACKING_EM)}
      </span>
      <span className={styles.cell} style={RULES.style}>
        {truncate(view.rules, RULES.budgetEm, TRACKING_EM)}
      </span>
    </div>
  );
});
