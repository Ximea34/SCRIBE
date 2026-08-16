import { memo } from "react";

import type { Placement } from "../../../../types/bindings";
import { cx } from "../../../cx";

import { pointsToPixels } from "./view";

import styles from "./Canvas.module.css";

interface StripFieldProps {
  fieldKey: string;
  label: string;
  placement: Placement;
  fontSizePt: number;
  scale: number;
  selected: boolean;
}

// The canvas shows the field's label, never a sample value.
export const StripField = memo(function StripField({
  fieldKey,
  label,
  placement,
  fontSizePt,
  scale,
  selected,
}: StripFieldProps) {
  return (
    <div
      className={cx(styles.item, styles.text, selected && styles.selected)}
      data-drag="placement"
      data-field={fieldKey}
      data-id={placement.id}
      style={{
        left: `${String(placement.xMm * scale)}px`,
        top: `${String(placement.yMm * scale)}px`,
        fontSize: `${String(pointsToPixels(fontSizePt, scale))}px`,
      }}
    >
      {label}
    </div>
  );
});
