const MARKER = ">";
const MEASURE_SIZE = 100;
const MEASURE_FONT = `700 ${String(MEASURE_SIZE)}px "Inria Sans", system-ui, sans-serif`;

let context: CanvasRenderingContext2D | null | undefined;
const emWidths = new Map<string, number>();
const truncations = new Map<string, string>();

function measurer(): CanvasRenderingContext2D | null {
  if (context === undefined) {
    context = document.createElement("canvas").getContext("2d");
    if (context) context.font = MEASURE_FONT;
  }
  return context;
}

// Measured once in em. Because both the cell budget and the font size scale with the same
// factor, one measurement holds at every viewport size — no re-measuring on resize.
function emWidth(text: string): number | null {
  const cached = emWidths.get(text);
  if (cached !== undefined) return cached;

  const canvas = measurer();
  if (!canvas) return null;
  const width = canvas.measureText(text).width / MEASURE_SIZE;
  emWidths.set(text, width);
  return width;
}

function renderedEm(text: string, trackingEm: number): number | null {
  const base = emWidth(text);
  if (base === null) return null;
  return base + Math.max(0, text.length - 1) * trackingEm;
}

// Shortens `text` to fit `budgetEm`, marking the cut with `>` — never an ellipsis, never a wrap.
export function truncate(text: string, budgetEm: number, trackingEm: number): string {
  if (text.length === 0) return text;

  const key = `${String(budgetEm)}|${String(trackingEm)}|${text}`;
  const cached = truncations.get(key);
  if (cached !== undefined) return cached;

  let result = text;
  const full = renderedEm(text, trackingEm);
  if (full !== null && full > budgetEm) {
    let keep = text.length - 1;
    while (keep > 0) {
      const width = renderedEm(text.slice(0, keep) + MARKER, trackingEm);
      if (width === null || width <= budgetEm) break;
      keep -= 1;
    }
    result = text.slice(0, keep) + MARKER;
  }

  truncations.set(key, result);
  return result;
}

// Measurements taken before the bundled font is ready would be wrong; drop them once it is.
if (typeof document !== "undefined") {
  document.fonts.ready
    .then(() => {
      emWidths.clear();
      truncations.clear();
    })
    .catch(() => {
      /* keep the fallback measurements */
    });
}
