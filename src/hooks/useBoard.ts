import { useEffect } from "react";

import { connectBoard } from "../app/ipc";
import { resetBoard } from "../app/store";

// Held by VIGIE alone: leaving the tab unmounts it, which drops the listener and the data, so a
// hidden tab costs nothing.
export function useBoard(): void {
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    connectBoard()
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch((error: unknown) => {
        console.error("cannot connect to the board", error);
      });

    return () => {
      cancelled = true;
      unlisten?.();
      resetBoard();
    };
  }, []);
}
