import type { ColumnId } from "../../app/store";
import { useColumn } from "../../hooks/useStoreSlice";

import { Strip } from "./Strip";

import styles from "./StripList.module.css";

interface StripListProps {
  column: ColumnId;
  onSelect?: ((callsign: string) => void) | undefined;
}

// Subscribes to the ordered callsigns alone, so reordering never touches a strip's own data.
export function StripList({ column, onSelect }: StripListProps) {
  const callsigns = useColumn(column);

  return (
    <div className={styles.list}>
      {callsigns.map((callsign) => (
        <Strip key={callsign} callsign={callsign} onSelect={onSelect} />
      ))}
    </div>
  );
}
