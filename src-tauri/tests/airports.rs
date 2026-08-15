use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use scribe_lib::airports::{
    load_selected, parse_airports, AirportError, AirportLineError, IssueKind,
};

const TOLERANCE: f64 = 1e-6;

fn temp_file(contents: &str) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let name = format!(
        "scribe-airports-{}-{}.txt",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, contents).expect("temp file should be writable");
    path
}

#[test]
fn parses_the_documented_lfll_line() {
    let (registry, issues) = parse_airports(
        "# DISCUS airport configuration\n\
         # ICAO;NAME;LAT;LON;ELEV_FT\n\
         LFLL;LYON SAINT EXUPERY;45°43'32\"N;005°04'52\"E;821\n",
    );

    assert!(issues.is_empty(), "{issues:?}");
    let airport = registry.get("LFLL").expect("LFLL should be defined");
    assert_eq!(&*airport.name, "LYON SAINT EXUPERY");
    assert!((airport.lat - 45.725556).abs() < TOLERANCE);
    assert!((airport.lon - 5.081111).abs() < TOLERANCE);
    assert_eq!(airport.elevation_ft, 821);
}

#[test]
fn skips_comments_and_blank_lines() {
    let (registry, issues) = parse_airports(
        "\n\
         # a comment\n\
         \n\
           # an indented comment\n\
         LFLL;LYON;45.0;5.0;821\n\
         \n",
    );
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(registry.len(), 1);
}

#[test]
fn tolerates_a_byte_order_mark() {
    let (registry, issues) = parse_airports("\u{feff}LFLL;LYON;45.0;5.0;821");
    assert!(issues.is_empty(), "{issues:?}");
    assert!(registry.get("LFLL").is_some());
}

#[test]
fn lower_case_icao_is_normalised() {
    let (registry, _) = parse_airports("lfll;Lyon;45.0;5.0;821");
    assert!(registry.get("LFLL").is_some());
}

#[test]
fn a_malformed_line_is_skipped_with_its_line_number() {
    let (registry, issues) = parse_airports(
        "LFLL;LYON;45.0;5.0;821\n\
         LFPG;PARIS;not-a-latitude;2.5;392\n\
         LFPO;ORLY;48.7;2.4;291\n",
    );

    assert_eq!(registry.len(), 2, "the good lines must still load");
    assert_eq!(issues.len(), 1);
    let issue = issues.first().expect("one issue");
    assert_eq!(issue.line, 2);
    assert!(matches!(
        issue.kind,
        IssueKind::Malformed(AirportLineError::Latitude(_))
    ));
}

#[test]
fn rejects_bad_icao_short_lines_and_elevations() {
    let (registry, issues) = parse_airports(
        "LFL;TOO SHORT;45.0;5.0;821\n\
         LF11;DIGITS;45.0;5.0;821\n\
         LFPG;MISSING FIELDS;45.0\n\
         LFPO;BAD ELEVATION;48.7;2.4;high\n",
    );

    assert!(registry.is_empty());
    let kinds: Vec<&IssueKind> = issues.iter().map(|issue| &issue.kind).collect();
    assert!(matches!(
        kinds.first(),
        Some(IssueKind::Malformed(AirportLineError::BadIcao(_)))
    ));
    assert!(matches!(
        kinds.get(1),
        Some(IssueKind::Malformed(AirportLineError::BadIcao(_)))
    ));
    assert!(matches!(
        kinds.get(2),
        Some(IssueKind::Malformed(AirportLineError::TooFewFields(3)))
    ));
    assert!(matches!(
        kinds.get(3),
        Some(IssueKind::Malformed(AirportLineError::BadElevation(_)))
    ));
}

#[test]
fn an_out_of_range_coordinate_is_a_line_error_not_a_file_error() {
    let (registry, issues) = parse_airports(
        "LFLL;LYON;95.0;5.0;821\n\
         LFPG;PARIS;49.0;2.5;392\n",
    );
    assert_eq!(registry.len(), 1);
    assert_eq!(issues.len(), 1);
}

#[test]
fn a_duplicate_icao_keeps_the_last_definition_and_warns() {
    let (registry, issues) = parse_airports(
        "LFLL;FIRST;45.0;5.0;800\n\
         LFLL;SECOND;46.0;6.0;900\n",
    );

    let airport = registry.get("LFLL").expect("LFLL should be defined");
    assert_eq!(&*airport.name, "SECOND");
    assert_eq!(airport.elevation_ft, 900);
    assert_eq!(issues.len(), 1);
    assert!(matches!(
        issues.first().map(|i| &i.kind),
        Some(IssueKind::DuplicateIcao(_))
    ));
}

#[test]
fn extra_columns_are_accepted_and_only_noted() {
    let (registry, issues) = parse_airports("LFLL;LYON;45.0;5.0;821;RESERVED;MORE");
    assert!(registry.get("LFLL").is_some());
    assert_eq!(
        issues.first().map(|i| &i.kind),
        Some(&IssueKind::ExtraColumns(2))
    );
}

#[test]
fn a_negative_elevation_is_valid() {
    let (registry, issues) = parse_airports("EHAM;SCHIPHOL;52.3086;4.7639;-11");
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(
        registry.get("EHAM").map(|airport| airport.elevation_ft),
        Some(-11)
    );
}

#[test]
fn loading_a_selected_airport_succeeds_and_normalises_the_icao() {
    let path = temp_file("LFLL;LYON SAINT EXUPERY;45°43'32\"N;005°04'52\"E;821\n");
    let airport = load_selected(&path, "lfll").expect("LFLL should load");
    assert_eq!(&*airport.icao, "LFLL");
    assert_eq!(airport.elevation_ft, 821);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_missing_selected_airport_is_a_hard_error() {
    let path = temp_file("LFLL;LYON;45.0;5.0;821\n");
    let error = load_selected(&path, "LFPG").expect_err("LFPG is not in the file");
    assert!(matches!(error, AirportError::SelectedNotFound { .. }));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_file_with_no_usable_airport_is_a_hard_error() {
    let path = temp_file("# only comments\n\nnonsense\n");
    let error = load_selected(&path, "LFLL").expect_err("nothing is defined");
    assert!(matches!(error, AirportError::Empty { .. }));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_missing_file_is_reported_as_unreadable() {
    let path = std::env::temp_dir().join("scribe-airports-does-not-exist.txt");
    let error = load_selected(&path, "LFLL").expect_err("file is absent");
    assert!(matches!(error, AirportError::Read { .. }));
}
