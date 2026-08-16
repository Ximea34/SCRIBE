import { memo } from "react";

import { LABELS } from "../../../../app/labels";
import type { CatalogueEntry, TemplateField } from "../../../../types/bindings";
import { SquarePlus, Trash } from "../../../icons/Icons";
import { Scrollbar } from "../../../components/Scrollbar";
import { cx } from "../../../cx";
import * as editor from "../state";

import { SizeStepper } from "./SizeStepper";

import styles from "./Panel.module.css";

interface FieldRowProps {
  entry: CatalogueEntry;
  field: TemplateField | undefined;
}

const FieldRow = memo(function FieldRow({ entry, field }: FieldRowProps) {
  const placed = field !== undefined && field.placements.length > 0;
  const fontSize = field?.fontSizePt ?? editor.DEFAULT_FONT_PT;

  return (
    <div className={cx(styles.row, placed && styles.used)}>
      <span className={styles.name}>{entry.label}</span>

      <SizeStepper
        value={fontSize}
        min={editor.MIN_FONT_PT}
        max={editor.MAX_FONT_PT}
        onChange={(next) => {
          if (placed) editor.setFontSize(entry.key, next);
        }}
      />

      <button
        type="button"
        className={cx(styles.action, styles.add, styles.addCell)}
        aria-label={`${LABELS.editor.add} ${entry.label}`}
        onClick={() => {
          editor.addPlacement(entry.key);
        }}
      >
        <SquarePlus />
      </button>

      <button
        type="button"
        className={cx(styles.action, styles.remove, styles.trashCell)}
        aria-label={`${LABELS.editor.remove} ${entry.label}`}
        disabled={!placed}
        onClick={() => {
          editor.clearField(entry.key);
        }}
      >
        <Trash />
      </button>
    </div>
  );
});

interface FieldPanelProps {
  catalogue: CatalogueEntry[];
  fields: TemplateField[];
}

// The whole catalogue, always: the rows are catalogue-driven, never user-built.
export function FieldPanel({ catalogue, fields }: FieldPanelProps) {
  const byKey = new Map(fields.map((field) => [field.key, field]));

  return (
    <>
      <div className={styles.headers}>
        <span className={styles.headerName}>{LABELS.editor.columnName}</span>
        <span className={styles.headerSize}>{LABELS.editor.columnSize}</span>
        <span />
        <span />
      </div>

      <Scrollbar className={styles.list}>
        {catalogue.map((entry) => (
          <FieldRow key={entry.key} entry={entry} field={byKey.get(entry.key)} />
        ))}
      </Scrollbar>
    </>
  );
}
