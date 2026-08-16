import { beforeEach, describe, expect, it } from "vitest";

import * as editor from "./state";

function doc() {
  return editor.getState().document;
}

function firstPlacement(key: string) {
  return doc().fields.find((field) => field.key === key)?.placements[0];
}

beforeEach(() => {
  editor.resetForTest();
});

describe("placements", () => {
  it("places a field at the centre of the strip", () => {
    editor.addPlacement("CALLSIGN");
    const placement = firstPlacement("CALLSIGN");
    expect(placement?.xMm).toBeCloseTo(203 / 2, 9);
    expect(placement?.yMm).toBeCloseTo(25 / 2, 9);
  });

  it("allows several placements of the same field under one entry", () => {
    editor.addPlacement("CALLSIGN");
    editor.addPlacement("CALLSIGN");
    expect(doc().fields).toHaveLength(1);
    expect(doc().fields[0]?.placements).toHaveLength(2);
  });

  it("clears every placement of a field but leaves the others", () => {
    editor.addPlacement("CALLSIGN");
    editor.addPlacement("CALLSIGN");
    editor.addPlacement("EOBT");
    editor.clearField("CALLSIGN");
    expect(doc().fields.map((field) => field.key)).toEqual(["EOBT"]);
  });

  it("removes one placement without touching its siblings", () => {
    editor.addPlacement("CALLSIGN");
    editor.addPlacement("CALLSIGN");
    const target = firstPlacement("CALLSIGN");
    editor.removePlacement("CALLSIGN", target?.id ?? "");
    expect(doc().fields[0]?.placements).toHaveLength(1);
  });

  it("drops the entry once its last placement goes", () => {
    editor.addPlacement("CALLSIGN");
    editor.removePlacement("CALLSIGN", firstPlacement("CALLSIGN")?.id ?? "");
    expect(doc().fields).toHaveLength(0);
  });

  it("keeps the font size on the field, applying to all its placements", () => {
    editor.addPlacement("CALLSIGN");
    editor.addPlacement("CALLSIGN");
    editor.setFontSize("CALLSIGN", 20);
    expect(doc().fields[0]?.fontSizePt).toBe(20);
  });

  it("holds the font size inside the printable range", () => {
    editor.addPlacement("CALLSIGN");
    editor.setFontSize("CALLSIGN", 2);
    expect(doc().fields[0]?.fontSizePt).toBe(editor.MIN_FONT_PT);
    editor.setFontSize("CALLSIGN", 500);
    expect(doc().fields[0]?.fontSizePt).toBe(editor.MAX_FONT_PT);
  });

  it("keeps a dragged origin inside the strip", () => {
    editor.addPlacement("CALLSIGN");
    const id = firstPlacement("CALLSIGN")?.id ?? "";
    editor.movePlacement("CALLSIGN", id, 5000, -20);
    expect(firstPlacement("CALLSIGN")?.xMm).toBe(203);
    expect(firstPlacement("CALLSIGN")?.yMm).toBe(0);
  });
});

describe("design elements", () => {
  it("adds one instance per click, each independently removable", () => {
    editor.addElement("line");
    editor.addElement("line");
    expect(doc().elements).toHaveLength(2);

    const first = doc().elements[0]?.id ?? "";
    editor.removeElement(first);
    expect(doc().elements).toHaveLength(1);
  });

  it("edits a single instance", () => {
    editor.addElement("text");
    const id = doc().elements[0]?.id ?? "";
    editor.updateElement(id, { content: "REMARQUES" });
    const element = doc().elements[0];
    expect(element?.kind === "text" ? element.content : "").toBe("REMARQUES");
  });
});

describe("undo and redo", () => {
  it("covers every kind of mutation", () => {
    editor.addPlacement("CALLSIGN");
    editor.setFontSize("CALLSIGN", 24);
    editor.addElement("frame");
    editor.setSize(150, 20);

    expect(doc().size.lengthMm).toBe(150);
    editor.undo();
    expect(doc().size.lengthMm).toBe(203);
    editor.undo();
    expect(doc().elements).toHaveLength(0);
    editor.undo();
    expect(doc().fields[0]?.fontSizePt).toBe(editor.DEFAULT_FONT_PT);
    editor.undo();
    expect(doc().fields).toHaveLength(0);
  });

  it("redoes what it undid", () => {
    editor.addPlacement("CALLSIGN");
    editor.undo();
    expect(doc().fields).toHaveLength(0);
    editor.redo();
    expect(doc().fields).toHaveLength(1);
  });

  it("does nothing at either end of the stack", () => {
    expect(() => {
      editor.undo();
      editor.redo();
    }).not.toThrow();
    expect(doc().fields).toHaveLength(0);
  });

  it("collapses a whole drag into a single entry", () => {
    editor.addPlacement("CALLSIGN");
    const id = firstPlacement("CALLSIGN")?.id ?? "";

    editor.beginGesture();
    for (let step = 1; step <= 25; step += 1) {
      editor.movePlacement("CALLSIGN", id, step, step / 4);
    }
    editor.endGesture();

    expect(firstPlacement("CALLSIGN")?.xMm).toBe(25);
    editor.undo();
    expect(firstPlacement("CALLSIGN")?.xMm).toBeCloseTo(203 / 2, 9);
    expect(doc().fields).toHaveLength(1);
  });

  it("collapses a held stepper repeat into a single entry", () => {
    editor.addPlacement("CALLSIGN");
    editor.beginGesture();
    for (let pt = 13; pt <= 30; pt += 1) editor.setFontSize("CALLSIGN", pt);
    editor.endGesture();

    expect(doc().fields[0]?.fontSizePt).toBe(30);
    editor.undo();
    expect(doc().fields[0]?.fontSizePt).toBe(editor.DEFAULT_FONT_PT);
  });

  it("is cleared by loading a template", () => {
    editor.addPlacement("CALLSIGN");
    editor.loadTemplate(
      {
        schemaVersion: 1,
        name: "LFLL VIGIE DEPARTURE STRIP",
        icao: "LFLL",
        position: "VIGIE",
        kind: "DEPARTURE",
        size: { lengthMm: 203, widthMm: 25 },
        fields: [],
        elements: [],
      },
      "LFLL_VIGIE_DEPARTURE_STRIP.json",
    );
    expect(editor.getState().canUndo).toBe(false);
    expect(editor.getState().dirty).toBe(false);
  });
});

describe("strip size", () => {
  it("pulls anything now outside the new bounds back in and says so", () => {
    editor.addPlacement("CALLSIGN");
    const id = firstPlacement("CALLSIGN")?.id ?? "";
    editor.movePlacement("CALLSIGN", id, 190, 24);

    editor.setSize(100, 15);
    expect(firstPlacement("CALLSIGN")?.xMm).toBe(100);
    expect(firstPlacement("CALLSIGN")?.yMm).toBe(15);
    expect(editor.getState().notice).toBe("clamped");
  });

  it("says nothing when everything already fits", () => {
    editor.addPlacement("CALLSIGN");
    editor.setSize(300, 40);
    expect(editor.getState().notice).toBeNull();
  });

  it("refuses absurd dimensions at the input", () => {
    editor.setSize(0, 0);
    expect(doc().size.lengthMm).toBe(editor.MIN_LENGTH_MM);
    expect(doc().size.widthMm).toBe(editor.MIN_WIDTH_MM);

    editor.setSize(5000, 5000);
    expect(doc().size.lengthMm).toBe(editor.MAX_LENGTH_MM);
    expect(doc().size.widthMm).toBe(editor.MAX_WIDTH_MM);
  });
});

describe("the name", () => {
  it("is outside the undo stack and never marks the document dirty", () => {
    editor.setName("LFLL VIGIE DEPARTURE STRIP");
    expect(editor.getState().dirty).toBe(false);
    expect(editor.getState().canUndo).toBe(false);
  });

  it("is carried into the template handed to Rust for validation", () => {
    editor.setName("lfll vigie departure strip");
    expect(editor.toTemplate().name).toBe("lfll vigie departure strip");
  });
});
