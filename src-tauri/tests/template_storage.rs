use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use scribe_lib::templates::logo::{self, LogoError, MAX_BYTES};
use scribe_lib::templates::storage::{self, SaveOutcome, StorageError};
use scribe_lib::templates::{StripSize, StripTemplate, SCHEMA_VERSION};

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let path = std::env::temp_dir().join(format!(
        "scribe-templates-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).expect("temp dir");
    path
}

fn named(name: &str) -> StripTemplate {
    StripTemplate {
        schema_version: SCHEMA_VERSION,
        name: name.to_owned(),
        icao: String::new(),
        position: String::new(),
        kind: String::new(),
        size: StripSize {
            length_mm: 203.0,
            width_mm: 25.0,
        },
        fields: Vec::new(),
        elements: Vec::new(),
    }
}

fn entries(directory: &PathBuf) -> Vec<String> {
    std::fs::read_dir(directory)
        .expect("readable")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn saving_parses_the_name_into_explicit_fields() {
    let directory = temp_dir();
    let outcome = storage::save(
        &directory,
        &named("lfll vigie departure strip"),
        None,
        false,
    )
    .expect("save");
    assert_eq!(
        outcome,
        SaveOutcome::Saved("LFLL_VIGIE_DEPARTURE_STRIP.json".to_owned())
    );

    let loaded = storage::load(&directory, "LFLL_VIGIE_DEPARTURE_STRIP.json").expect("load");
    assert_eq!(loaded.name, "LFLL VIGIE DEPARTURE STRIP");
    assert_eq!(loaded.icao, "LFLL");
    assert_eq!(loaded.position, "VIGIE");
    assert_eq!(loaded.kind, "DEPARTURE");
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn an_invalid_name_is_refused_before_anything_is_written() {
    let directory = temp_dir();
    let error = storage::save(
        &directory,
        &named("LFLL TOWER DEPARTURE STRIP"),
        None,
        false,
    )
    .expect_err("invalid name");

    assert!(matches!(error, StorageError::Name(_)));
    assert!(entries(&directory).is_empty(), "nothing should be written");
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn an_existing_target_asks_before_overwriting_a_colleagues_template() {
    let directory = temp_dir();
    storage::save(
        &directory,
        &named("LFLL VIGIE DEPARTURE STRIP"),
        None,
        false,
    )
    .expect("first");

    let outcome = storage::save(
        &directory,
        &named("LFLL VIGIE DEPARTURE STRIP"),
        None,
        false,
    )
    .expect("second");
    assert!(matches!(outcome, SaveOutcome::NeedsConfirmation(_)));

    let confirmed = storage::save(&directory, &named("LFLL VIGIE DEPARTURE STRIP"), None, true)
        .expect("confirmed");
    assert!(matches!(confirmed, SaveOutcome::Saved(_)));
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn saving_over_the_bound_file_never_asks() {
    let directory = temp_dir();
    let file = "LFLL_VIGIE_DEPARTURE_STRIP.json";
    storage::save(
        &directory,
        &named("LFLL VIGIE DEPARTURE STRIP"),
        None,
        false,
    )
    .expect("first");

    let outcome = storage::save(
        &directory,
        &named("LFLL VIGIE DEPARTURE STRIP"),
        Some(file),
        false,
    )
    .expect("resave");
    assert!(matches!(outcome, SaveOutcome::Saved(_)));
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn saving_under_a_new_name_creates_a_second_template_and_keeps_the_first() {
    let directory = temp_dir();
    let bound = "LFLL_VIGIE_DEPARTURE_STRIP.json";
    storage::save(
        &directory,
        &named("LFLL VIGIE DEPARTURE STRIP"),
        None,
        false,
    )
    .expect("create");

    let outcome = storage::save(
        &directory,
        &named("LFLL IFR ARRIVAL STRIP"),
        Some(bound),
        false,
    )
    .expect("save under a new name");

    assert_eq!(
        outcome,
        SaveOutcome::Saved("LFLL_IFR_ARRIVAL_STRIP.json".to_owned())
    );
    let mut remaining = entries(&directory);
    remaining.sort();
    assert_eq!(
        remaining,
        vec![
            "LFLL_IFR_ARRIVAL_STRIP.json".to_owned(),
            "LFLL_VIGIE_DEPARTURE_STRIP.json".to_owned()
        ],
        "building one strip from another must never destroy the original"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn an_atomic_write_leaves_no_temporary_behind() {
    let directory = temp_dir();
    for _ in 0..5 {
        storage::save(&directory, &named("LFLL VIGIE DEPARTURE STRIP"), None, true).expect("save");
    }
    assert_eq!(entries(&directory), vec!["LFLL_VIGIE_DEPARTURE_STRIP.json"]);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_missing_directory_lists_nothing_rather_than_failing() {
    let directory = temp_dir().join("not-created-yet");
    assert!(storage::list(&directory).expect("list").is_empty());
}

#[test]
fn a_corrupt_file_is_listed_as_invalid_and_stays_deletable() {
    let directory = temp_dir();
    storage::save(
        &directory,
        &named("LFLL VIGIE DEPARTURE STRIP"),
        None,
        false,
    )
    .expect("save");
    std::fs::write(directory.join("BROKEN.json"), "{ not json").expect("write");

    let listing = storage::list(&directory).expect("list");
    assert_eq!(listing.len(), 2, "the corrupt file must not be hidden");

    let broken = listing
        .iter()
        .find(|entry| entry.file_name == "BROKEN.json")
        .expect("the corrupt file is listed");
    assert!(!broken.valid);
    assert!(broken.error.is_some());

    let good = listing
        .iter()
        .find(|entry| entry.file_name == "LFLL_VIGIE_DEPARTURE_STRIP.json")
        .expect("the good file is listed");
    assert!(good.valid);
    assert_eq!(good.name, "LFLL VIGIE DEPARTURE STRIP");

    storage::delete(&directory, "BROKEN.json").expect("a corrupt file still deletes");
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_newer_schema_version_refuses_to_load_with_a_clear_message() {
    let directory = temp_dir();
    let future = format!(
        r#"{{"schemaVersion":{},"name":"LFLL VIGIE DEPARTURE STRIP","icao":"LFLL","position":"VIGIE","kind":"DEPARTURE","size":{{"lengthMm":203,"widthMm":25}},"fields":[],"elements":[]}}"#,
        SCHEMA_VERSION + 1
    );
    std::fs::write(directory.join("FUTURE.json"), future).expect("write");

    let error = storage::load(&directory, "FUTURE.json").expect_err("newer schema");
    assert!(matches!(error, StorageError::Schema(_)));
    assert!(error.to_string().contains("schema version"));
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_file_name_cannot_escape_the_strips_directory() {
    let directory = temp_dir();
    for hostile in ["../secret.json", "sub/dir.json", "..\\up.json", ""] {
        assert!(
            storage::load(&directory, hostile).is_err(),
            "{hostile:?} must be refused"
        );
        assert!(storage::delete(&directory, hostile).is_err());
    }
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn listings_are_alphabetical() {
    let directory = temp_dir();
    for name in [
        "LFPG VIGIE TRANSIT STRIP",
        "LFLL IFR ARRIVAL STRIP",
        "LFLL VIGIE DEPARTURE STRIP",
    ] {
        storage::save(&directory, &named(name), None, false).expect("save");
    }
    let names: Vec<String> = storage::list(&directory)
        .expect("list")
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    assert_eq!(
        names,
        [
            "LFLL IFR ARRIVAL STRIP",
            "LFLL VIGIE DEPARTURE STRIP",
            "LFPG VIGIE TRANSIT STRIP"
        ]
    );
    let _ = std::fs::remove_dir_all(&directory);
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    bytes.extend_from_slice(&13u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes
}

fn jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08];
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&[0; 8]);
    bytes
}

#[test]
fn a_png_logo_is_read_with_its_dimensions() {
    let directory = temp_dir();
    let path = directory.join("logo.png");
    std::fs::write(&path, png(240, 80)).expect("write");

    let imported = logo::import(&path).expect("import");
    assert_eq!(imported.mime, "image/png");
    assert_eq!((imported.width_px, imported.height_px), (240, 80));
    assert!(!imported.data.is_empty());
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_jpeg_logo_is_read_with_its_dimensions() {
    let directory = temp_dir();
    let path = directory.join("logo.jpg");
    std::fs::write(&path, jpeg(128, 64)).expect("write");

    let imported = logo::import(&path).expect("import");
    assert_eq!(imported.mime, "image/jpeg");
    assert_eq!((imported.width_px, imported.height_px), (128, 64));
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn the_type_comes_from_the_magic_bytes_not_the_extension() {
    let directory = temp_dir();
    let path = directory.join("actually-a-png.jpg");
    std::fs::write(&path, png(10, 10)).expect("write");

    assert_eq!(logo::import(&path).expect("import").mime, "image/png");
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn an_oversized_or_unsupported_image_is_refused_clearly() {
    let directory = temp_dir();

    let big = directory.join("big.png");
    let mut bytes = png(10, 10);
    bytes.resize(MAX_BYTES + 1, 0);
    std::fs::write(&big, bytes).expect("write");
    assert!(matches!(
        logo::import(&big),
        Err(LogoError::TooLarge { .. })
    ));

    let gif = directory.join("logo.gif");
    std::fs::write(&gif, b"GIF89a").expect("write");
    assert_eq!(logo::import(&gif), Err(LogoError::Unsupported));

    let missing = directory.join("nothing.png");
    assert!(matches!(logo::import(&missing), Err(LogoError::Read(_))));
    let _ = std::fs::remove_dir_all(&directory);
}
