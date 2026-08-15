use std::path::Path;
use std::process::Command;

const BINDINGS: &str = "../src/types/bindings.ts";

/// The export runs through the real binary: a bare test process cannot load the webview
/// runtime that `tauri::Wry` drags in. Comparing before and after catches a stale commit.
#[test]
fn the_typescript_bindings_are_committed_and_current() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bindings = manifest.join(BINDINGS);
    let before = std::fs::read_to_string(&bindings).unwrap_or_default();

    let status = Command::new(env!("CARGO_BIN_EXE_scribe"))
        .arg("--export-bindings")
        .current_dir(manifest)
        .status()
        .expect("the export binary should run");
    assert!(status.success(), "exporting the bindings failed");

    let after = std::fs::read_to_string(&bindings).expect("bindings should exist");
    assert_eq!(
        before, after,
        "src/types/bindings.ts is stale; commit the regenerated file"
    );

    for expected in [
        "boardSnapshot",
        "activateFlight",
        "flightDetail",
        "boardUpdated",
        "BoardUpdate",
        "BoardSnapshot",
        "StripView",
        "Columns",
        "FlightDetail",
        "IpcError",
    ] {
        assert!(
            after.contains(expected),
            "the bindings should expose {expected}"
        );
    }
}
