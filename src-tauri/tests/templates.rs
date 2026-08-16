use scribe_lib::templates::catalogue::{self, FieldSource};
use scribe_lib::templates::naming::{self, NameError};
use scribe_lib::templates::{
    DesignElement, LineElement, Orientation, Placement, StripSize, StripTemplate, TemplateField,
    SCHEMA_VERSION,
};

fn template() -> StripTemplate {
    StripTemplate {
        schema_version: SCHEMA_VERSION,
        name: "LFLL VIGIE DEPARTURE STRIP".to_owned(),
        icao: "LFLL".to_owned(),
        position: "VIGIE".to_owned(),
        kind: "DEPARTURE".to_owned(),
        size: StripSize {
            length_mm: 203.0,
            width_mm: 25.0,
        },
        fields: vec![TemplateField {
            key: "CALLSIGN".to_owned(),
            font_size_pt: 12.0,
            placements: vec![Placement {
                id: "p1".to_owned(),
                x_mm: 8.0,
                y_mm: 4.5,
            }],
        }],
        elements: vec![DesignElement::Line(LineElement {
            id: "e1".to_owned(),
            x_mm: 0.0,
            y_mm: 12.0,
            length_mm: 203.0,
            thickness_mm: 0.4,
            orientation: Orientation::Horizontal,
        })],
    }
}

#[test]
fn a_well_formed_name_parses_into_its_three_tokens() {
    let parsed = naming::parse("LFLL VIGIE DEPARTURE STRIP").expect("valid");
    assert_eq!(parsed.icao, "LFLL");
    assert_eq!(parsed.position, "VIGIE");
    assert_eq!(parsed.kind, "DEPARTURE");
    assert_eq!(parsed.normalised, "LFLL VIGIE DEPARTURE STRIP");
}

#[test]
fn every_documented_example_is_accepted() {
    for name in [
        "LFLL VIGIE DEPARTURE STRIP",
        "LFLL IFR ARRIVAL STRIP",
        "LFLL VIGIE TRANSIT STRIP",
    ] {
        assert!(naming::parse(name).is_ok(), "{name} should be valid");
    }
}

#[test]
fn input_is_upper_cased_and_whitespace_collapsed_before_validation() {
    let parsed = naming::parse("  lfll   vigie\tdeparture  strip ").expect("valid");
    assert_eq!(parsed.normalised, "LFLL VIGIE DEPARTURE STRIP");
    assert_eq!(parsed.icao, "LFLL");
}

#[test]
fn each_malformed_name_says_what_is_wrong() {
    assert_eq!(naming::parse("   "), Err(NameError::Empty));
    assert_eq!(
        naming::parse("LFLL VIGIE STRIP"),
        Err(NameError::WordCount(3))
    );
    assert_eq!(
        naming::parse("LFL VIGIE DEPARTURE STRIP"),
        Err(NameError::Icao("LFL".to_owned()))
    );
    assert_eq!(
        naming::parse("LF11 VIGIE DEPARTURE STRIP"),
        Err(NameError::Icao("LF11".to_owned()))
    );
    assert_eq!(
        naming::parse("LFLL TOWER DEPARTURE STRIP"),
        Err(NameError::Position("TOWER".to_owned()))
    );
    assert_eq!(
        naming::parse("LFLL VIGIE PUSHBACK STRIP"),
        Err(NameError::Kind("PUSHBACK".to_owned()))
    );
    assert_eq!(
        naming::parse("LFLL VIGIE DEPARTURE BANDE"),
        Err(NameError::Suffix("BANDE".to_owned()))
    );
}

#[test]
fn the_file_name_is_derived_from_the_normalised_name() {
    let parsed = naming::parse("lfll vigie departure strip").expect("valid");
    assert_eq!(
        naming::file_name(&parsed),
        "LFLL_VIGIE_DEPARTURE_STRIP.json"
    );
}

#[test]
fn a_template_survives_a_json_round_trip() {
    let original = template();
    let json = serde_json::to_string_pretty(&original).expect("serialise");
    let restored: StripTemplate = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(restored, original);
}

#[test]
fn the_json_uses_the_documented_camel_case_shape() {
    let json = serde_json::to_string(&template()).expect("serialise");
    for expected in [
        "\"schemaVersion\"",
        "\"lengthMm\"",
        "\"widthMm\"",
        "\"fontSizePt\"",
        "\"xMm\"",
        "\"yMm\"",
        "\"kind\":\"line\"",
        "\"thicknessMm\"",
        "\"orientation\":\"horizontal\"",
    ] {
        assert!(
            json.contains(expected),
            "the JSON should contain {expected}"
        );
    }
}

#[test]
fn every_design_element_kind_round_trips_through_its_tag() {
    let json = r#"[
        {"kind":"line","id":"a","xMm":0,"yMm":0,"lengthMm":10,"thicknessMm":0.4,"orientation":"vertical"},
        {"kind":"frame","id":"b","xMm":1,"yMm":2,"widthMm":3,"heightMm":4,"thicknessMm":0.5},
        {"kind":"text","id":"c","xMm":5,"yMm":6,"content":"REMARQUES","fontSizePt":9},
        {"kind":"image","id":"d","xMm":7,"yMm":8,"widthMm":9,"heightMm":10,"mime":"image/png","data":"AA=="}
    ]"#;
    let elements: Vec<DesignElement> = serde_json::from_str(json).expect("deserialise");

    assert_eq!(elements.len(), 4);
    assert!(matches!(elements.first(), Some(DesignElement::Line(_))));
    assert!(matches!(elements.get(1), Some(DesignElement::Frame(_))));
    assert!(matches!(elements.get(2), Some(DesignElement::Text(_))));
    assert!(matches!(elements.get(3), Some(DesignElement::Image(_))));
    assert_eq!(elements.first().map(DesignElement::id), Some("a"));
}

#[test]
fn the_catalogue_is_in_protocol_order_and_never_alphabetical() {
    let entries = catalogue::catalogue();
    assert_eq!(entries.len(), 35, "16 flight plan, 17 position, 2 derived");

    let keys: Vec<&str> = entries.iter().map(|entry| entry.key.as_str()).collect();
    assert_eq!(keys.first(), Some(&"CALLSIGN"));
    assert_eq!(keys.get(1), Some(&"DEPARTURE"));
    assert_eq!(keys.get(15), Some(&"REMARKS"));
    assert_eq!(keys.get(16), Some(&"HEADING"));
    assert_eq!(keys.last(), Some(&"PRINT_TIME"));

    let mut alphabetical = keys.clone();
    alphabetical.sort_unstable();
    assert_ne!(keys, alphabetical, "protocol order, not alphabetical");
}

#[test]
fn catalogue_keys_are_unique_and_labels_are_readable() {
    let entries = catalogue::catalogue();
    let mut seen = std::collections::HashSet::new();
    for entry in &entries {
        assert!(
            seen.insert(entry.key.clone()),
            "duplicate key {}",
            entry.key
        );
        assert!(
            !entry.label.contains('_'),
            "{} should read as words on a strip",
            entry.label
        );
    }
    assert_eq!(
        entries
            .iter()
            .find(|entry| entry.key == "CRUISE_LEVEL")
            .map(|entry| entry.label.as_str()),
        Some("CRUISE LEVEL")
    );
}

#[test]
fn the_catalogue_excludes_the_fields_the_notes_rule_out() {
    for excluded in ["IS_SELECTED", "WAS_SELECTED", "ASSIGNED_GATE"] {
        assert!(!catalogue::contains(excluded), "{excluded} must not appear");
    }
    assert!(catalogue::contains("VERTICAL_SPEED"));
    assert!(catalogue::contains("CONTROLLER_STATION"));
    assert!(catalogue::contains("PRINT_TIME"));
}

#[test]
fn each_group_carries_its_source() {
    let entries = catalogue::catalogue();
    let source_of = |key: &str| {
        entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.source)
    };
    assert_eq!(source_of("CALLSIGN"), Some(FieldSource::FlightPlan));
    assert_eq!(source_of("HEADING"), Some(FieldSource::TrafficPosition));
    assert_eq!(source_of("PRINT_TIME"), Some(FieldSource::Derived));
}
