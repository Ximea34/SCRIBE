import { useCallback, useSyncExternalStore } from "react";

import { getColumn, getStrip, subscribeColumn, subscribeStrip, type ColumnId } from "../app/store";
import type { StripView } from "../types/bindings";

// A column subscribes to its ordered list of callsigns only — never to the flight objects.
export function useColumn(id: ColumnId): readonly string[] {
  const subscribe = useCallback((listener: () => void) => subscribeColumn(id, listener), [id]);
  const snapshot = useCallback(() => getColumn(id), [id]);
  return useSyncExternalStore(subscribe, snapshot);
}

// A strip subscribes to its own callsign, so a reorder elsewhere cannot wake it.
export function useStrip(callsign: string): StripView | undefined {
  const subscribe = useCallback(
    (listener: () => void) => subscribeStrip(callsign, listener),
    [callsign],
  );
  const snapshot = useCallback(() => getStrip(callsign), [callsign]);
  return useSyncExternalStore(subscribe, snapshot);
}
