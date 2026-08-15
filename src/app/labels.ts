// Every string the controller sees, in French, exactly as in the Figma. Nothing else in the
// front end may hold user-facing text.
export const LABELS = {
  tabs: {
    vigie: "VIGIE",
    ifr: "IFR",
    editeur: "EDITEUR",
    options: "OPTIONS",
  },
  columns: {
    awake: "ÉVEILLÉS",
    activated: "ACTIVÉS",
    transits: "TRANSITS",
  },
  window: {
    navigation: "Onglets",
    minimize: "Réduire",
    maximize: "Agrandir",
    close: "Fermer",
  },
} as const;
