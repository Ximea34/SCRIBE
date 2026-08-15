import type { UnlistenFn } from "@tauri-apps/api/event";

import { commands, events } from "../types/bindings";

import { applySnapshot, applyUpdate } from "./store";

// Subscribe before asking for the snapshot. The store's sequence guard then discards whichever
// arrives out of order, so no update is missed and none is applied twice.
export async function connectBoard(): Promise<UnlistenFn> {
  const unlisten = await events.boardUpdated.listen((event) => {
    applyUpdate(event.payload);
  });

  const snapshot = await commands.boardSnapshot();
  if (snapshot.status === "ok") {
    applySnapshot(snapshot.data);
  } else {
    console.warn("board unavailable", snapshot.error);
  }
  return unlisten;
}
