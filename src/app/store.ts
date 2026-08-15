import type { BoardSnapshot, BoardUpdate, Columns, StripView } from "../types/bindings";

export type ColumnId = keyof Columns;

export const COLUMN_IDS = [
  "awake",
  "activatedDepartures",
  "arrivals",
  "transits",
] as const satisfies readonly ColumnId[];

type Listener = () => void;

const EMPTY: readonly string[] = Object.freeze([]);

// Module level, deliberately not React context: every consumer would re-render on every tick.
let strips: ReadonlyMap<string, StripView> = new Map();
let columns: Record<ColumnId, readonly string[]> = {
  awake: EMPTY,
  activatedDepartures: EMPTY,
  arrivals: EMPTY,
  transits: EMPTY,
};
let seq = 0;

const stripListeners = new Map<string, Set<Listener>>();
const columnListeners = new Map<ColumnId, Set<Listener>>();

function subscribeTo<K>(registry: Map<K, Set<Listener>>, key: K, listener: Listener): () => void {
  let listeners = registry.get(key);
  if (!listeners) {
    listeners = new Set();
    registry.set(key, listeners);
  }
  listeners.add(listener);
  return () => {
    const current = registry.get(key);
    if (!current) return;
    current.delete(listener);
    if (current.size === 0) registry.delete(key);
  };
}

function notify<K>(registry: Map<K, Set<Listener>>, key: K): void {
  const listeners = registry.get(key);
  if (!listeners) return;
  for (const listener of listeners) listener();
}

export function subscribeStrip(callsign: string, listener: Listener): () => void {
  return subscribeTo(stripListeners, callsign, listener);
}

export function subscribeColumn(id: ColumnId, listener: Listener): () => void {
  return subscribeTo(columnListeners, id, listener);
}

// Identity is stable while a flight's rendered data is unchanged, which is what lets a memoised
// strip skip re-rendering when only the ordering moved.
export function getStrip(callsign: string): StripView | undefined {
  return strips.get(callsign);
}

export function getColumn(id: ColumnId): readonly string[] {
  return columns[id];
}

function sameOrder(a: readonly string[], b: readonly string[]): boolean {
  if (a.length !== b.length) return false;
  for (let index = 0; index < a.length; index += 1) {
    if (a[index] !== b[index]) return false;
  }
  return true;
}

function replaceColumns(next: Columns): ColumnId[] {
  const changed: ColumnId[] = [];
  const merged = { ...columns };
  for (const id of COLUMN_IDS) {
    if (!sameOrder(columns[id], next[id])) {
      merged[id] = Object.freeze(next[id]);
      changed.push(id);
    }
  }
  if (changed.length > 0) columns = merged;
  return changed;
}

export function applySnapshot(snapshot: BoardSnapshot): void {
  if (snapshot.seq < seq) return;
  seq = snapshot.seq;

  const previous = strips;
  strips = new Map(snapshot.strips.map((strip) => [strip.callsign, strip]));
  const changedColumns = replaceColumns(snapshot.columns);

  for (const callsign of previous.keys()) {
    if (!strips.has(callsign)) notify(stripListeners, callsign);
  }
  for (const [callsign, strip] of strips) {
    if (previous.get(callsign) !== strip) notify(stripListeners, callsign);
  }
  for (const id of changedColumns) notify(columnListeners, id);
}

// Applies a diff and wakes only the slices it actually touched.
export function applyUpdate(update: BoardUpdate): void {
  if (update.seq <= seq) return;
  seq = update.seq;

  const touched: string[] = [];
  if (update.upserted.length > 0 || update.removed.length > 0) {
    const next = new Map(strips);
    for (const strip of update.upserted) {
      next.set(strip.callsign, strip);
      touched.push(strip.callsign);
    }
    for (const callsign of update.removed) {
      next.delete(callsign);
      touched.push(callsign);
    }
    strips = next;
  }

  const changedColumns = update.columns ? replaceColumns(update.columns) : [];

  for (const callsign of touched) notify(stripListeners, callsign);
  for (const id of changedColumns) notify(columnListeners, id);
}

export function resetBoard(): void {
  const previous = strips;
  strips = new Map();
  const changedColumns = replaceColumns({
    awake: [],
    activatedDepartures: [],
    arrivals: [],
    transits: [],
  });
  seq = 0;
  for (const callsign of previous.keys()) notify(stripListeners, callsign);
  for (const id of changedColumns) notify(columnListeners, id);
}
