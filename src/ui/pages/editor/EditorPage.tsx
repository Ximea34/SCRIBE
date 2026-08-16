import { useCallback, useEffect, useState, useSyncExternalStore } from "react";

import { LABELS } from "../../../app/labels";
import { commands } from "../../../types/bindings";
import type { CatalogueEntry, TemplateListing } from "../../../types/bindings";
import { Modal } from "../../components/Modal";
import { Pencil, Trash } from "../../icons/Icons";
import { Scrollbar } from "../../components/Scrollbar";
import { cx } from "../../cx";

import { StripCanvas } from "./canvas/StripCanvas";
import { DesignPanel } from "./panel/DesignPanel";
import { FieldPanel } from "./panel/FieldPanel";
import * as persistence from "./persistence";
import * as editor from "./state";

import panelStyles from "./panel/Panel.module.css";
import styles from "./Editor.module.css";

type PanelTab = "fields" | "design";
type Confirmation = { kind: "overwrite" } | { kind: "delete"; fileName: string };

const CONFIRM_TITLE_ID = "editor-confirm";

export function EditorPage() {
  const state = useSyncExternalStore(editor.subscribe, editor.getState);
  const [tab, setTab] = useState<PanelTab>("fields");
  const [catalogue, setCatalogue] = useState<CatalogueEntry[]>([]);
  const [listing, setListing] = useState<TemplateListing[]>([]);
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    persistence
      .list()
      .then(setListing)
      .catch((cause: unknown) => {
        console.error("cannot list templates", cause);
      });
  }, []);

  useEffect(() => {
    commands
      .getFieldCatalogue()
      .then(setCatalogue)
      .catch((cause: unknown) => {
        console.error("cannot load the field catalogue", cause);
      });
    refresh();
  }, [refresh]);

  // Leaving the EDITEUR tab unmounts this page, which is the flush trigger. Switching between
  // CHAMPS and DESIGN does not unmount anything, so it never writes.
  useEffect(() => {
    return () => {
      void persistence.flush();
    };
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!event.ctrlKey && !event.metaKey) return;
      const key = event.key.toLowerCase();
      if (key !== "z" && key !== "y") return;
      event.preventDefault();
      if (key === "y" || event.shiftKey) editor.redo();
      else editor.undo();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  const runSave = useCallback(
    (overwrite: boolean) => {
      persistence
        .save(overwrite)
        .then((result) => {
          if (result.status === "confirm") {
            setConfirmation({ kind: "overwrite" });
            return;
          }
          setError(result.status === "error" ? persistence.describeError(result.error) : null);
          setConfirmation(null);
          refresh();
        })
        .catch((cause: unknown) => {
          console.error("save failed", cause);
        });
    },
    [refresh],
  );

  const size = state.document.size;

  return (
    <div className={styles.page}>
      <div className={styles.sizeBox}>
        <SizeField
          label={LABELS.editor.length}
          value={size.lengthMm}
          onCommit={(value) => {
            editor.setSize(value, size.widthMm);
          }}
        />
        <SizeField
          label={LABELS.editor.width}
          value={size.widthMm}
          onCommit={(value) => {
            editor.setSize(size.lengthMm, value);
          }}
        />
      </div>

      <div className={styles.canvasSlot}>
        <StripCanvas state={state} catalogue={catalogue} />
      </div>

      {state.notice === "clamped" && <p className={styles.notice}>{LABELS.editor.clamped}</p>}

      <div className={styles.nameArea}>
        <div className={styles.nameLabel}>{LABELS.editor.name}</div>
        <div className={styles.nameRow}>
          <input
            className={styles.nameInput}
            value={state.name}
            spellCheck={false}
            onChange={(event) => {
              editor.setName(event.target.value);
            }}
          />
          <button
            type="button"
            className={styles.save}
            disabled={state.name.trim().length === 0}
            onClick={() => {
              runSave(false);
            }}
          >
            {LABELS.editor.save}
          </button>
        </div>
        {error !== null && <p className={styles.nameError}>{error}</p>}
      </div>

      <div className={styles.explorer}>
        <Scrollbar className={styles.explorerList}>
          {listing.map((entry) => (
            <div key={entry.fileName} className={styles.explorerRow}>
              <span className={cx(styles.explorerName, !entry.valid && styles.invalid)}>
                {entry.valid ? entry.name : `${entry.fileName} — ${entry.error ?? ""}`}
              </span>
              <button
                type="button"
                className={styles.explorerAction}
                aria-label={`${LABELS.editor.edit} ${entry.name}`}
                disabled={!entry.valid}
                onClick={() => {
                  void persistence.open(entry.fileName).then((failure) => {
                    if (failure) setError(persistence.describeError(failure));
                  });
                }}
              >
                <Pencil />
              </button>
              <button
                type="button"
                className={cx(styles.explorerAction, panelStyles.remove)}
                aria-label={`${LABELS.editor.remove} ${entry.name}`}
                onClick={() => {
                  setConfirmation({ kind: "delete", fileName: entry.fileName });
                }}
              >
                <Trash />
              </button>
            </div>
          ))}
        </Scrollbar>
      </div>

      <div className={cx(styles.panelSlot, panelStyles.panel)}>
        <div className={panelStyles.tabs}>
          <button
            type="button"
            className={cx(panelStyles.tab, tab === "fields" && panelStyles.tabActive)}
            onClick={() => {
              setTab("fields");
            }}
          >
            {LABELS.editor.tabFields}
          </button>
          <button
            type="button"
            className={cx(panelStyles.tab, tab === "design" && panelStyles.tabActive)}
            onClick={() => {
              setTab("design");
            }}
          >
            {LABELS.editor.tabDesign}
          </button>
        </div>
        <div className={panelStyles.body}>
          {tab === "fields" ? (
            <FieldPanel catalogue={catalogue} fields={state.document.fields} />
          ) : (
            <DesignPanel elements={state.document.elements} />
          )}
        </div>
      </div>

      {confirmation && (
        <Modal
          labelledBy={CONFIRM_TITLE_ID}
          onCancel={() => {
            setConfirmation(null);
          }}
          onConfirm={() => {
            if (confirmation.kind === "overwrite") {
              runSave(true);
            } else {
              const { fileName } = confirmation;
              setConfirmation(null);
              void persistence.remove(fileName).then(refresh);
            }
          }}
        >
          <h2 id={CONFIRM_TITLE_ID}>
            {confirmation.kind === "overwrite"
              ? LABELS.editor.confirmOverwrite
              : LABELS.editor.confirmDelete}
          </h2>
          <button
            type="button"
            onClick={() => {
              setConfirmation(null);
            }}
          >
            {LABELS.editor.cancel}
          </button>
          <button
            type="button"
            onClick={() => {
              if (confirmation.kind === "overwrite") {
                runSave(true);
              } else {
                const { fileName } = confirmation;
                setConfirmation(null);
                void persistence.remove(fileName).then(refresh);
              }
            }}
          >
            {LABELS.editor.confirm}
          </button>
        </Modal>
      )}
    </div>
  );
}

function SizeField({
  label,
  value,
  onCommit,
}: {
  label: string;
  value: number;
  onCommit: (value: number) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);

  return (
    <div className={styles.sizeGroup}>
      <div className={styles.sizeLabel}>{label}</div>
      <input
        className={styles.sizeInput}
        inputMode="decimal"
        value={draft ?? `${String(value)} ${LABELS.editor.millimetres}`}
        onFocus={() => {
          setDraft(String(value));
        }}
        onChange={(event) => {
          setDraft(event.target.value);
        }}
        onBlur={() => {
          const parsed = Number.parseFloat(draft ?? "");
          if (Number.isFinite(parsed)) onCommit(parsed);
          setDraft(null);
        }}
      />
    </div>
  );
}
