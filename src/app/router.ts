import { LABELS } from "./labels";

export const TABS = ["vigie", "ifr", "editeur", "options"] as const;

export type Tab = (typeof TABS)[number];

// VIGIE, IFR and EDITEUR are a left-anchored group; OPTIONS sits on the right.
export const LEFT_TABS = ["vigie", "ifr", "editeur"] as const satisfies readonly Tab[];

export function tabLabel(tab: Tab): string {
  return LABELS.tabs[tab];
}
