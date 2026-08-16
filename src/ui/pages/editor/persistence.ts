import { LABELS } from "../../../app/labels";
import { commands } from "../../../types/bindings";
import type { NameError, StorageError, TemplateListing } from "../../../types/bindings";

import * as editor from "./state";

/// Rust owns the validation, so the UI only turns its verdict into something a controller can
/// read — the reason is always shown, never swallowed.
export function describeError(error: StorageError): string {
  return error.kind === "name" ? describeNameError(error.detail) : error.detail;
}

function describeNameError(error: NameError): string {
  const messages = LABELS.editor.nameErrors;
  switch (error.kind) {
    case "empty":
      return messages.empty;
    case "wordCount":
      return messages.wordCount;
    case "icao":
      return `${messages.icao} ${error.detail}`;
    case "position":
      return `${messages.position} ${error.detail}`;
    case "kind":
      return `${messages.kind} ${error.detail}`;
    case "suffix":
      return `${messages.suffix} ${error.detail}`;
  }
}

/// A crash during a long template session should cost seconds, not the session.
const IDLE_MS = 30_000;

let idle: ReturnType<typeof setTimeout> | undefined;

export type SaveResult =
  | { status: "saved" }
  | { status: "confirm"; fileName: string }
  | { status: "error"; error: StorageError };

// Armed by any change once the document is bound; leaving the tab flushes regardless.
editor.subscribe(() => {
  const state = editor.getState();
  clearTimeout(idle);
  if (!state.dirty || !state.bound) return;
  idle = setTimeout(() => {
    void flush();
  }, IDLE_MS);
});

/// Writes only when bound and dirty, so leaving the tab without changing anything, or before
/// the first SAVE, never touches the disk.
export async function flush(): Promise<void> {
  clearTimeout(idle);
  const state = editor.getState();
  if (!state.bound || !state.dirty) return;

  const result = await commands.saveTemplate(editor.toTemplate(), state.bound, true);
  if (result.status === "ok" && result.data.outcome === "saved") {
    editor.markSaved(result.data.fileName);
  } else if (result.status === "error") {
    console.warn("autosave failed", result.error);
  }
}

/// SAVE is the only thing that commits the name: create on an unbound template, rename on a
/// bound one whose name changed.
export async function save(overwrite = false): Promise<SaveResult> {
  const state = editor.getState();
  const result = await commands.saveTemplate(editor.toTemplate(), state.bound, overwrite);
  if (result.status === "error") return { status: "error", error: result.error };

  if (result.data.outcome === "needsConfirmation") {
    return { status: "confirm", fileName: result.data.fileName };
  }
  editor.markSaved(result.data.fileName);
  return { status: "saved" };
}

export async function list(): Promise<TemplateListing[]> {
  const result = await commands.listTemplates();
  if (result.status === "error") {
    console.warn("cannot list templates", result.error);
    return [];
  }
  return result.data;
}

/// Flush, then act — the one line that stops the editor quietly eating work.
export async function open(fileName: string): Promise<StorageError | null> {
  await flush();
  const result = await commands.loadTemplate(fileName);
  if (result.status === "error") return result.error;
  editor.loadTemplate(result.data, fileName);
  return null;
}

export async function remove(fileName: string): Promise<StorageError | null> {
  await flush();
  const result = await commands.deleteTemplate(fileName);
  if (result.status === "error") return result.error;

  // Deleting the bound file unbinds the editor: the content stays, autosave stops, and SAVE
  // will recreate it.
  if (editor.getState().bound === fileName) editor.unbind();
  return null;
}
