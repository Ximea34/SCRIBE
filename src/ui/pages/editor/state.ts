import type {
  DesignElement,
  StripSize,
  StripTemplate,
  TemplateField,
} from "../../../types/bindings";

export const DEFAULT_SIZE: StripSize = { lengthMm: 203, widthMm: 25 };
export const MIN_FONT_PT = 6;
export const MAX_FONT_PT = 72;
export const DEFAULT_FONT_PT = 12;
export const FONT_STEP_LARGE = 10;
export const MIN_LENGTH_MM = 20;
export const MAX_LENGTH_MM = 400;
export const MIN_WIDTH_MM = 10;
export const MAX_WIDTH_MM = 200;
export const NUDGE_MM = 0.5;
export const FINE_NUDGE_MM = 0.1;
const UNDO_LIMIT = 100;

export interface EditorDocument {
  size: StripSize;
  fields: TemplateField[];
  elements: DesignElement[];
}

export type Selection =
  { kind: "placement"; fieldKey: string; id: string } | { kind: "element"; id: string };

export interface EditorState {
  document: EditorDocument;
  /// Outside the undo stack and never autosaved: it is committed by SAVE alone.
  name: string;
  bound: string | null;
  dirty: boolean;
  selection: Selection | null;
  canUndo: boolean;
  canRedo: boolean;
  notice: string | null;
}

export type ElementKind = DesignElement["kind"];

let identifiers = 0;

function newId(): string {
  identifiers += 1;
  return `${Date.now().toString(36)}-${identifiers.toString(36)}`;
}

export function emptyDocument(): EditorDocument {
  return { size: { ...DEFAULT_SIZE }, fields: [], elements: [] };
}

function clamp(value: number, low: number, high: number): number {
  return Math.min(high, Math.max(low, value));
}

let document_: EditorDocument = emptyDocument();
let name = "";
let bound: string | null = null;
let dirty = false;
let selection: Selection | null = null;
let notice: string | null = null;
let past: EditorDocument[] = [];
let future: EditorDocument[] = [];
let gesture = false;

const listeners = new Set<() => void>();
let snapshot: EditorState = build();

function build(): EditorState {
  return {
    document: document_,
    name,
    bound,
    dirty,
    selection,
    canUndo: past.length > 0,
    canRedo: future.length > 0,
    notice,
  };
}

function publish(): void {
  snapshot = build();
  for (const listener of listeners) listener();
}

export function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function getState(): EditorState {
  return snapshot;
}

/// One undo entry per gesture: the snapshot is pushed when the gesture opens, and the
/// mutations that follow edit the present in place.
export function beginGesture(): void {
  if (gesture) return;
  gesture = true;
  push();
}

export function endGesture(): void {
  gesture = false;
}

function push(): void {
  past = [...past.slice(-(UNDO_LIMIT - 1)), document_];
  future = [];
}

function mutate(next: (current: EditorDocument) => EditorDocument): void {
  if (!gesture) push();
  document_ = next(document_);
  dirty = true;
  publish();
}

export function undo(): void {
  const previous = past.at(-1);
  if (previous === undefined) return;
  past = past.slice(0, -1);
  future = [document_, ...future];
  document_ = previous;
  dirty = true;
  selection = null;
  publish();
}

export function redo(): void {
  const [next, ...rest] = future;
  if (next === undefined) return;
  future = rest;
  past = [...past, document_];
  document_ = next;
  dirty = true;
  selection = null;
  publish();
}

export function setName(value: string): void {
  name = value;
  publish();
}

export function select(next: Selection | null): void {
  selection = next;
  publish();
}

export function dismissNotice(): void {
  notice = null;
  publish();
}

export function addPlacement(key: string): void {
  mutate((current) => {
    const placement = {
      id: newId(),
      xMm: current.size.lengthMm / 2,
      yMm: current.size.widthMm / 2,
    };
    const existing = current.fields.find((field) => field.key === key);
    const fields = existing
      ? current.fields.map((field) =>
          field.key === key ? { ...field, placements: [...field.placements, placement] } : field,
        )
      : [...current.fields, { key, fontSizePt: DEFAULT_FONT_PT, placements: [placement] }];
    return { ...current, fields };
  });
}

/// The panel trash clears the whole entry; the row itself is catalogue-driven and stays.
export function clearField(key: string): void {
  mutate((current) => ({
    ...current,
    fields: current.fields.filter((field) => field.key !== key),
  }));
}

export function setFontSize(key: string, pt: number): void {
  const clamped = clamp(Math.round(pt), MIN_FONT_PT, MAX_FONT_PT);
  mutate((current) => ({
    ...current,
    fields: current.fields.map((field) =>
      field.key === key ? { ...field, fontSizePt: clamped } : field,
    ),
  }));
}

export function movePlacement(key: string, id: string, xMm: number, yMm: number): void {
  mutate((current) => ({
    ...current,
    fields: current.fields.map((field) =>
      field.key === key
        ? {
            ...field,
            placements: field.placements.map((placement) =>
              placement.id === id
                ? {
                    ...placement,
                    xMm: clamp(xMm, 0, current.size.lengthMm),
                    yMm: clamp(yMm, 0, current.size.widthMm),
                  }
                : placement,
            ),
          }
        : field,
    ),
  }));
}

export function removePlacement(key: string, id: string): void {
  mutate((current) => ({
    ...current,
    fields: current.fields
      .map((field) =>
        field.key === key
          ? { ...field, placements: field.placements.filter((p) => p.id !== id) }
          : field,
      )
      .filter((field) => field.placements.length > 0),
  }));
}

function defaultElement(kind: ElementKind, size: StripSize): DesignElement {
  const id = newId();
  const xMm = size.lengthMm / 2;
  const yMm = size.widthMm / 2;
  switch (kind) {
    case "line":
      return {
        kind,
        id,
        xMm,
        yMm,
        lengthMm: Math.min(40, size.lengthMm),
        thicknessMm: 0.4,
        orientation: "horizontal",
      };
    case "frame":
      return { kind, id, xMm, yMm, widthMm: 30, heightMm: 10, thicknessMm: 0.4 };
    case "text":
      return { kind, id, xMm, yMm, content: "TEXTE", fontSizePt: DEFAULT_FONT_PT };
    case "image":
      return { kind, id, xMm, yMm, widthMm: 20, heightMm: 20, mime: "image/png", data: "" };
  }
}

export function addElement(kind: ElementKind): void {
  mutate((current) => ({
    ...current,
    elements: [...current.elements, defaultElement(kind, current.size)],
  }));
}

export function updateElement(id: string, patch: Partial<DesignElement>): void {
  mutate((current) => ({
    ...current,
    elements: current.elements.map((element) =>
      element.id === id ? ({ ...element, ...patch } as DesignElement) : element,
    ),
  }));
}

export function moveElement(id: string, xMm: number, yMm: number): void {
  mutate((current) => ({
    ...current,
    elements: current.elements.map((element) =>
      element.id === id
        ? {
            ...element,
            xMm: clamp(xMm, 0, current.size.lengthMm),
            yMm: clamp(yMm, 0, current.size.widthMm),
          }
        : element,
    ),
  }));
}

/// Unlike the panel trash in CHAMPS, this removes a single instance.
export function removeElement(id: string): void {
  mutate((current) => ({
    ...current,
    elements: current.elements.filter((element) => element.id !== id),
  }));
}

export function removeSelected(): void {
  const target = selection;
  if (!target) return;
  selection = null;
  if (target.kind === "placement") removePlacement(target.fieldKey, target.id);
  else removeElement(target.id);
}

/// Placements keep their millimetres; anything now outside the new bounds is pulled back in
/// and the controller is told it happened.
export function setSize(lengthMm: number, widthMm: number): void {
  const size: StripSize = {
    lengthMm: clamp(lengthMm, MIN_LENGTH_MM, MAX_LENGTH_MM),
    widthMm: clamp(widthMm, MIN_WIDTH_MM, MAX_WIDTH_MM),
  };
  const beyond = (xMm: number, yMm: number) => xMm > size.lengthMm || yMm > size.widthMm;
  const moved =
    document_.fields.some((field) =>
      field.placements.some((placement) => beyond(placement.xMm, placement.yMm)),
    ) || document_.elements.some((element) => beyond(element.xMm, element.yMm));

  const pull = (xMm: number, yMm: number) => ({
    xMm: clamp(xMm, 0, size.lengthMm),
    yMm: clamp(yMm, 0, size.widthMm),
  });

  mutate((current) => ({
    size,
    fields: current.fields.map((field) => ({
      ...field,
      placements: field.placements.map((placement) => ({
        ...placement,
        ...pull(placement.xMm, placement.yMm),
      })),
    })),
    elements: current.elements.map((element) => ({
      ...element,
      ...pull(element.xMm, element.yMm),
    })),
  }));

  notice = moved ? "clamped" : null;
  publish();
}

export function loadTemplate(template: StripTemplate, fileName: string): void {
  document_ = {
    size: template.size,
    fields: template.fields,
    elements: template.elements,
  };
  name = template.name;
  bound = fileName;
  dirty = false;
  selection = null;
  notice = null;
  past = [];
  future = [];
  gesture = false;
  publish();
}

export function markSaved(fileName: string): void {
  bound = fileName;
  dirty = false;
  publish();
}

export function unbind(): void {
  bound = null;
  publish();
}

export function toTemplate(): StripTemplate {
  return {
    schemaVersion: 1,
    name,
    icao: "",
    position: "",
    kind: "",
    size: document_.size,
    fields: document_.fields,
    elements: document_.elements,
  };
}

/// Test seam; the running app never resets except by loading.
export function resetForTest(): void {
  document_ = emptyDocument();
  name = "";
  bound = null;
  dirty = false;
  selection = null;
  notice = null;
  past = [];
  future = [];
  gesture = false;
  publish();
}
