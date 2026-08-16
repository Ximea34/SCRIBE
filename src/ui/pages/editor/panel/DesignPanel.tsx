import { LABELS } from "../../../../app/labels";
import type { DesignElement } from "../../../../types/bindings";
import { SquarePlus, Trash } from "../../../icons/Icons";
import { Scrollbar } from "../../../components/Scrollbar";
import { cx } from "../../../cx";
import * as editor from "../state";

import { SizeStepper } from "./SizeStepper";

import styles from "./Panel.module.css";

const PALETTE = [
  { kind: "line", label: LABELS.editor.palette.line },
  { kind: "frame", label: LABELS.editor.palette.frame },
  { kind: "text", label: LABELS.editor.palette.text },
  { kind: "image", label: LABELS.editor.palette.image },
] as const satisfies readonly { kind: editor.ElementKind; label: string }[];

const MAX_MM = 400;
const MIN_THICKNESS_MM = 0.1;
const MAX_THICKNESS_MM = 5;
const millimetres = (value: number) => `${String(value)} ${LABELS.editor.millimetres}`;

interface Property {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  format?: (value: number) => string;
  apply: (next: number) => void;
}

function properties(element: DesignElement): Property[] {
  const set = (patch: Partial<DesignElement>) => {
    editor.updateElement(element.id, patch);
  };
  const sizes = LABELS.editor.elementSize;

  switch (element.kind) {
    case "line":
      return [
        {
          label: sizes.length,
          value: element.lengthMm,
          min: 1,
          max: MAX_MM,
          format: millimetres,
          apply: (lengthMm) => {
            set({ lengthMm });
          },
        },
        {
          label: sizes.thickness,
          value: element.thicknessMm,
          min: MIN_THICKNESS_MM,
          max: MAX_THICKNESS_MM,
          step: 0.1,
          format: millimetres,
          apply: (thicknessMm) => {
            set({ thicknessMm });
          },
        },
      ];
    case "frame":
      return [
        {
          label: sizes.width,
          value: element.widthMm,
          min: 1,
          max: MAX_MM,
          format: millimetres,
          apply: (widthMm) => {
            set({ widthMm });
          },
        },
        {
          label: sizes.height,
          value: element.heightMm,
          min: 1,
          max: MAX_MM,
          format: millimetres,
          apply: (heightMm) => {
            set({ heightMm });
          },
        },
        {
          label: sizes.thickness,
          value: element.thicknessMm,
          min: MIN_THICKNESS_MM,
          max: MAX_THICKNESS_MM,
          step: 0.1,
          format: millimetres,
          apply: (thicknessMm) => {
            set({ thicknessMm });
          },
        },
      ];
    case "text":
      return [
        {
          label: sizes.fontSize,
          value: element.fontSizePt,
          min: editor.MIN_FONT_PT,
          max: editor.MAX_FONT_PT,
          apply: (fontSizePt) => {
            set({ fontSizePt });
          },
        },
      ];
    case "image":
      return [
        {
          label: sizes.width,
          value: element.widthMm,
          min: 1,
          max: MAX_MM,
          format: millimetres,
          apply: (widthMm) => {
            // Height follows the source aspect ratio.
            const ratio = element.widthMm > 0 ? element.heightMm / element.widthMm : 1;
            set({ widthMm, heightMm: Number((widthMm * ratio).toFixed(2)) });
          },
        },
      ];
  }
}

function describe(element: DesignElement): string {
  switch (element.kind) {
    case "line":
      return LABELS.editor.palette.line;
    case "frame":
      return LABELS.editor.palette.frame;
    case "text":
      return element.content;
    case "image":
      return LABELS.editor.palette.image;
  }
}

// A line turns by swapping its axis, a frame by swapping its two sides. Neither needs a rotation
// angle in the template, which keeps the printed shape a plain rectangle.
function turn(element: DesignElement): (() => void) | null {
  if (element.kind === "line") {
    return () => {
      editor.updateElement(element.id, {
        orientation: element.orientation === "horizontal" ? "vertical" : "horizontal",
      });
    };
  }
  if (element.kind === "frame") {
    return () => {
      editor.updateElement(element.id, {
        widthMm: element.heightMm,
        heightMm: element.widthMm,
      });
    };
  }
  return null;
}

function orientationLabel(element: DesignElement): string {
  const short = LABELS.editor.orientationShort;
  if (element.kind === "line") {
    return element.orientation === "horizontal" ? short.horizontal : short.vertical;
  }
  return short.turn;
}

// Unlike CHAMPS, the trash here removes that single instance: three lines are three objects.
export function DesignPanel({ elements }: { elements: DesignElement[] }) {
  return (
    <>
      <div className={styles.headers}>
        <span className={styles.headerName}>{LABELS.editor.columnElement}</span>
        <span className={styles.headerSize}>{LABELS.editor.columnSize}</span>
        <span />
        <span />
      </div>

      <div className={styles.palette}>
        {PALETTE.map((item) => (
          <div key={item.kind} className={styles.row}>
            <span className={styles.name}>{item.label}</span>
            <button
              type="button"
              className={cx(styles.action, styles.add, styles.addCell)}
              aria-label={`${LABELS.editor.add} ${item.label}`}
              onClick={() => {
                editor.addElement(item.kind);
              }}
            >
              <SquarePlus />
            </button>
          </div>
        ))}
      </div>

      <Scrollbar className={styles.list}>
        {elements.map((element) => {
          const rotate = turn(element);
          return (
            <div key={element.id}>
              <div className={cx(styles.row, styles.used)}>
                {element.kind === "text" ? (
                  <input
                    className={styles.nameInput}
                    value={element.content}
                    onFocus={editor.beginGesture}
                    onBlur={editor.endGesture}
                    onChange={(event) => {
                      editor.updateElement(element.id, { content: event.target.value });
                    }}
                  />
                ) : (
                  <span className={styles.name}>{describe(element)}</span>
                )}

                {rotate ? (
                  <button
                    type="button"
                    className={cx(styles.value, styles.turn)}
                    aria-label={`${LABELS.editor.elementSize.orientation} ${describe(element)}`}
                    onClick={rotate}
                  >
                    {orientationLabel(element)}
                  </button>
                ) : (
                  <span />
                )}

                <button
                  type="button"
                  className={cx(styles.action, styles.remove, styles.trashCell)}
                  aria-label={`${LABELS.editor.remove} ${describe(element)}`}
                  onClick={() => {
                    editor.removeElement(element.id);
                  }}
                >
                  <Trash />
                </button>
              </div>

              {properties(element).map((property) => (
                <div key={property.label} className={cx(styles.row, styles.property)}>
                  <span className={cx(styles.name, styles.propertyLabel)}>{property.label}</span>
                  <SizeStepper
                    value={property.value}
                    min={property.min}
                    max={property.max}
                    step={property.step ?? 1}
                    onChange={property.apply}
                    format={property.format}
                  />
                </div>
              ))}
            </div>
          );
        })}
      </Scrollbar>
    </>
  );
}
