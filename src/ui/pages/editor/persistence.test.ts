import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const saveTemplate = vi.fn();
const loadTemplate = vi.fn();
const deleteTemplate = vi.fn();
const listTemplates = vi.fn();

vi.mock("../../../types/bindings", () => ({
  commands: {
    saveTemplate: (...args: unknown[]) => saveTemplate(...args) as unknown,
    loadTemplate: (...args: unknown[]) => loadTemplate(...args) as unknown,
    deleteTemplate: (...args: unknown[]) => deleteTemplate(...args) as unknown,
    listTemplates: (...args: unknown[]) => listTemplates(...args) as unknown,
  },
  events: {},
}));

// Imported after the mock so persistence.ts binds to the stubbed commands.
const editor = await import("./state");
const persistence = await import("./persistence");

const BOUND = "LFLL_VIGIE_DEPARTURE_STRIP.json";

function bind() {
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
    BOUND,
  );
}

beforeEach(() => {
  vi.useFakeTimers();
  saveTemplate.mockReset();
  loadTemplate.mockReset();
  deleteTemplate.mockReset();
  listTemplates.mockReset();
  saveTemplate.mockResolvedValue({
    status: "ok",
    data: { outcome: "saved", fileName: BOUND },
  });
  editor.resetForTest();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("the four write triggers", () => {
  it("writes on flush when bound and changed", async () => {
    bind();
    editor.addPlacement("CALLSIGN");
    await persistence.flush();
    expect(saveTemplate).toHaveBeenCalledTimes(1);
    expect(editor.getState().dirty).toBe(false);
  });

  it("never writes while the template is unbound", async () => {
    editor.addPlacement("CALLSIGN");
    await persistence.flush();
    expect(saveTemplate).not.toHaveBeenCalled();
  });

  it("never writes when nothing changed", async () => {
    bind();
    await persistence.flush();
    expect(saveTemplate).not.toHaveBeenCalled();
  });

  it("is not triggered by typing in the name field", async () => {
    bind();
    editor.setName("LFLL IFR ARRIVAL STRIP");
    await vi.advanceTimersByTimeAsync(60_000);
    expect(saveTemplate).not.toHaveBeenCalled();
  });

  it("writes once the document has been idle", async () => {
    bind();
    editor.addPlacement("CALLSIGN");
    await vi.advanceTimersByTimeAsync(29_000);
    expect(saveTemplate).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(2_000);
    expect(saveTemplate).toHaveBeenCalledTimes(1);
  });

  it("restarts the idle countdown on every further change", async () => {
    bind();
    editor.addPlacement("CALLSIGN");
    await vi.advanceTimersByTimeAsync(20_000);
    editor.addPlacement("EOBT");
    await vi.advanceTimersByTimeAsync(20_000);
    expect(saveTemplate).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(11_000);
    expect(saveTemplate).toHaveBeenCalledTimes(1);
  });
});

describe("flush before anything destructive", () => {
  it("flushes before loading another template", async () => {
    bind();
    editor.addPlacement("CALLSIGN");
    loadTemplate.mockResolvedValue({
      status: "ok",
      data: {
        schemaVersion: 1,
        name: "LFLL IFR ARRIVAL STRIP",
        icao: "LFLL",
        position: "IFR",
        kind: "ARRIVAL",
        size: { lengthMm: 203, widthMm: 25 },
        fields: [],
        elements: [],
      },
    });

    await persistence.open("LFLL_IFR_ARRIVAL_STRIP.json");
    expect(saveTemplate).toHaveBeenCalledTimes(1);
    expect(editor.getState().bound).toBe("LFLL_IFR_ARRIVAL_STRIP.json");
  });

  it("flushes before deleting, then unbinds while keeping the content", async () => {
    bind();
    editor.addPlacement("CALLSIGN");
    deleteTemplate.mockResolvedValue({ status: "ok", data: null });

    await persistence.remove(BOUND);
    expect(saveTemplate).toHaveBeenCalledTimes(1);
    expect(editor.getState().bound).toBeNull();
    expect(editor.getState().document.fields).toHaveLength(1);
  });

  it("leaves another template's deletion bound as it was", async () => {
    bind();
    deleteTemplate.mockResolvedValue({ status: "ok", data: null });
    await persistence.remove("SOMETHING_ELSE.json");
    expect(editor.getState().bound).toBe(BOUND);
  });
});

describe("SAVE", () => {
  it("asks before overwriting a template it is not bound to", async () => {
    saveTemplate.mockResolvedValue({
      status: "ok",
      data: { outcome: "needsConfirmation", fileName: BOUND },
    });
    editor.setName("LFLL VIGIE DEPARTURE STRIP");

    const result = await persistence.save();
    expect(result).toEqual({ status: "confirm", fileName: BOUND });
    expect(editor.getState().bound).toBeNull();
  });

  it("binds the editor once the file exists", async () => {
    editor.setName("LFLL VIGIE DEPARTURE STRIP");
    const result = await persistence.save();
    expect(result).toEqual({ status: "saved" });
    expect(editor.getState().bound).toBe(BOUND);
  });

  it("surfaces a validation refusal in French rather than swallowing it", async () => {
    saveTemplate.mockResolvedValue({
      status: "error",
      error: { kind: "name", detail: { kind: "position", detail: "TOWER" } },
    });
    editor.setName("LFLL TOWER DEPARTURE STRIP");

    const result = await persistence.save();
    expect(result.status).toBe("error");
    if (result.status === "error") {
      expect(persistence.describeError(result.error)).toContain("TOWER");
    }
  });
});
