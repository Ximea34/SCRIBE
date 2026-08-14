use scribe_lib::aurora::protocol::{
    self, Command, CommandName, InvalidCallsign, ParseError, Prefix, Response,
};

const FP_LINE: &str = "#FP;AFR1234;LFLL;LFPG;LFPO;1215;A320;M;I;S;SDE3FGHIRWY/LB1;F330;N0450;0230;0055;BEBIX UM976 MOROK;PBN/A1B1";
const TRPOS_LINE: &str = "#TRPOS;AFR1234;90;92;12500;310;45.725556;5.081111;7000;7001;BODRU8A 04R;F330;N0450;LFLL_APP;LFLL_CTR;0;1;0;A12;V;;-1200;B07";

fn parse(line: &str) -> Response<'_> {
    protocol::parse(line).expect("line should parse")
}

#[test]
fn parses_conn_and_seltfc() {
    assert_eq!(
        parse("#CONN;LFLL_TWR"),
        Response::Conn {
            station: "LFLL_TWR"
        }
    );
    assert_eq!(
        parse("#SELTFC;AFR1234"),
        Response::SelTfc {
            callsign: "AFR1234"
        }
    );
    assert_eq!(parse("#SELTFC;"), Response::SelTfc { callsign: "" });
    assert_eq!(parse("#SELTFC"), Response::SelTfc { callsign: "" });
}

#[test]
fn flight_plan_field_seven_is_rules_and_eight_is_flight_type() {
    let Response::FlightPlan(fp) = parse(FP_LINE) else {
        panic!("expected a flight plan");
    };
    assert_eq!(fp.callsign(), "AFR1234");
    assert_eq!(fp.dep(), "LFLL");
    assert_eq!(fp.arr(), "LFPG");
    assert_eq!(fp.alternate(), "LFPO");
    assert_eq!(fp.eobt(), "1215");
    assert_eq!(fp.aircraft(), "A320");
    assert_eq!(fp.wake(), "M");
    assert_eq!(fp.rules(), "I");
    assert_eq!(fp.flight_type(), "S");
    assert_eq!(fp.equipment(), "SDE3FGHIRWY/LB1");
    assert_eq!(fp.cruise_level(), "F330");
    assert_eq!(fp.cruise_speed(), "N0450");
    assert_eq!(fp.endurance(), "0230");
    assert_eq!(fp.eet(), "0055");
    assert_eq!(fp.route(), "BEBIX UM976 MOROK");
    assert_eq!(fp.remarks(), "PBN/A1B1");
}

#[test]
fn flight_plan_tolerates_empty_and_missing_fields() {
    let Response::FlightPlan(fp) = parse("#FP;AFR1234;;LFPG;;;;;V") else {
        panic!("expected a flight plan");
    };
    assert_eq!(fp.callsign(), "AFR1234");
    assert_eq!(fp.dep(), "");
    assert_eq!(fp.arr(), "LFPG");
    assert_eq!(fp.rules(), "V");
    assert_eq!(fp.route(), "");
    assert_eq!(fp.remarks(), "");
}

#[test]
fn flight_plan_tolerates_trailing_separators() {
    let Response::FlightPlan(fp) = parse("#FP;AFR1234;LFLL;LFPG;;;;;;;;;;;;;;;;") else {
        panic!("expected a flight plan");
    };
    assert_eq!(fp.arr(), "LFPG");
}

#[test]
fn flight_plan_without_a_callsign_is_rejected() {
    assert_eq!(
        protocol::parse("#FP;"),
        Err(ParseError::MissingCallsign {
            command: "FP".to_owned()
        })
    );
}

#[test]
fn traffic_position_maps_every_documented_field() {
    let Response::TrafficPosition(tp) = parse(TRPOS_LINE) else {
        panic!("expected a traffic position");
    };
    assert_eq!(tp.callsign(), "AFR1234");
    assert_eq!(tp.heading(), Some(90));
    assert_eq!(tp.track(), Some(92));
    assert_eq!(tp.altitude(), Some(12_500));
    assert_eq!(tp.ground_speed(), Some(310));
    assert!((tp.lat().expect("lat") - 45.725556).abs() < 1e-9);
    assert!((tp.lon().expect("lon") - 5.081111).abs() < 1e-9);
    assert_eq!(tp.squawk_set(), "7000");
    assert_eq!(tp.squawk_label(), "7001");
    assert_eq!(tp.wp_label(), "BODRU8A 04R");
    assert_eq!(tp.alt_label(), "F330");
    assert_eq!(tp.spd_label(), "N0450");
    assert_eq!(tp.assumed_by(), "LFLL_APP");
    assert_eq!(tp.next_station(), "LFLL_CTR");
    assert!(!tp.on_ground());
    assert!(tp.is_selected());
    assert!(!tp.was_selected());
    assert_eq!(tp.gate(), "A12");
    assert_eq!(tp.voice(), "V");
    assert_eq!(tp.vertical_speed(), Some(-1200));
    assert_eq!(tp.assigned_gate(), "B07");
}

#[test]
fn traffic_position_ignores_fields_past_the_known_range() {
    let line = format!("{TRPOS_LINE};EXTRA;MORE");
    let Response::TrafficPosition(tp) = parse(&line) else {
        panic!("expected a traffic position");
    };
    assert_eq!(tp.assigned_gate(), "B07");
}

#[test]
fn traffic_position_numbers_degrade_to_none() {
    let Response::TrafficPosition(tp) = parse("#TRPOS;AFR1234;;abc;-250;;;;") else {
        panic!("expected a traffic position");
    };
    assert_eq!(tp.heading(), None);
    assert_eq!(tp.track(), None);
    assert_eq!(tp.altitude(), Some(-250));
    assert_eq!(tp.ground_speed(), None);
    assert_eq!(tp.lat(), None);
    assert_eq!(tp.vertical_speed(), None);
    assert!(!tp.on_ground());
}

#[test]
fn traffic_position_accepts_decimal_headings() {
    let Response::TrafficPosition(tp) = parse("#TRPOS;AFR1234;359.6;0.4;12500.0") else {
        panic!("expected a traffic position");
    };
    assert_eq!(tp.heading(), Some(360));
    assert_eq!(tp.track(), Some(0));
    assert_eq!(tp.altitude(), Some(12_500));
}

#[test]
fn parses_the_traffic_list_and_skips_blanks() {
    let Response::TrafficList(list) = parse("#TR;AFR1234;;RYR33EK; FGEKO ") else {
        panic!("expected a traffic list");
    };
    assert_eq!(
        list.iter().collect::<Vec<_>>(),
        ["AFR1234", "RYR33EK", "FGEKO"]
    );

    let Response::TrafficList(empty) = parse("#TR;") else {
        panic!("expected a traffic list");
    };
    assert_eq!(empty.iter().count(), 0);
}

#[test]
fn parses_atc_pairs() {
    let Response::Atc(list) = parse("#ATC;LFLL_APP:120.500;LFLL_TWR:118.100;broken") else {
        panic!("expected an ATC list");
    };
    assert_eq!(
        list.iter().collect::<Vec<_>>(),
        [("LFLL_APP", "120.500"), ("LFLL_TWR", "118.100")]
    );
}

#[test]
fn refusal_names_the_offending_command_and_keeps_the_whole_reason() {
    let Response::Refusal(refusal) = parse("@ERR;#TRPOS;AFR1234;no such traffic; retry later")
    else {
        panic!("expected a refusal");
    };
    assert_eq!(refusal.command, "#TRPOS");
    assert_eq!(refusal.argument, "AFR1234");
    assert_eq!(refusal.reason, "no such traffic; retry later");
    assert_eq!(
        CommandName::from_wire(refusal.command),
        Some(CommandName::TrafficPosition)
    );
}

#[test]
fn a_dollar_prefix_is_still_a_refusal() {
    let Response::Refusal(refusal) = parse("$ERR;#FP;AFR1234;nope") else {
        panic!("expected a refusal");
    };
    assert_eq!(refusal.command, "#FP");
}

#[test]
fn an_at_prefix_is_not_an_error_by_itself() {
    let response = parse("@BAY;No data in bay");
    assert_eq!(
        response,
        Response::Unknown {
            prefix: Prefix::At,
            command: "BAY",
            body: "No data in bay",
        }
    );
    assert!(!matches!(response, Response::Refusal(_)));
}

#[test]
fn unknown_commands_are_reported_not_guessed() {
    assert_eq!(
        parse("#WHAT;1;2"),
        Response::Unknown {
            prefix: Prefix::Hash,
            command: "WHAT",
            body: "1;2",
        }
    );
}

#[test]
fn malformed_lines_are_rejected() {
    assert_eq!(protocol::parse(""), Err(ParseError::Empty));
    assert_eq!(protocol::parse("\r\n"), Err(ParseError::Empty));
    assert_eq!(protocol::parse("CONN;X"), Err(ParseError::MissingPrefix));
    assert_eq!(protocol::parse("#"), Err(ParseError::MissingCommand));
    assert_eq!(protocol::parse("#;X"), Err(ParseError::MissingCommand));
}

#[test]
fn command_names_round_trip_and_do_not_collide() {
    for (index, name) in CommandName::ALL.into_iter().enumerate() {
        assert_eq!(name.index(), index);
        assert_eq!(CommandName::from_wire(name.as_str()), Some(name));
        assert_eq!(
            CommandName::from_wire(&format!("#{}", name.as_str())),
            Some(name)
        );
    }
    assert_eq!(CommandName::from_wire("TR"), Some(CommandName::TrafficList));
    assert_eq!(
        CommandName::from_wire("TRPOS"),
        Some(CommandName::TrafficPosition)
    );
    assert_eq!(CommandName::from_wire("TRPATHL"), None);
}

#[test]
fn seltfc_substitution_can_never_be_sent() {
    assert_eq!(
        Command::traffic_position("%SELTFC%"),
        Err(InvalidCallsign::ReservedChar('%'))
    );
    assert_eq!(
        Command::flight_plan("%SELTFC%"),
        Err(InvalidCallsign::ReservedChar('%'))
    );
}

#[test]
fn callsign_validation_rejects_injection_and_nonsense() {
    assert_eq!(Command::flight_plan(""), Err(InvalidCallsign::Empty));
    assert_eq!(
        Command::flight_plan("AFR;1234"),
        Err(InvalidCallsign::ReservedChar(';'))
    );
    assert_eq!(
        Command::flight_plan("AFR\r\n#CONN"),
        Err(InvalidCallsign::ReservedChar('\r'))
    );
    assert_eq!(
        Command::flight_plan("A".repeat(17).as_str()),
        Err(InvalidCallsign::TooLong)
    );
    assert!(Command::flight_plan("RYR33EK").is_ok());
}

#[test]
fn commands_serialise_to_the_wire_form() {
    let mut out = String::new();
    Command::Conn.write_into(&mut out);
    assert_eq!(out, "#CONN\r\n");

    out.clear();
    Command::TrafficList.write_into(&mut out);
    assert_eq!(out, "#TR\r\n");

    out.clear();
    Command::flight_plan("AFR1234")
        .expect("valid callsign")
        .write_into(&mut out);
    assert_eq!(out, "#FP;AFR1234\r\n");

    out.clear();
    Command::traffic_position("RYR33EK")
        .expect("valid callsign")
        .write_into(&mut out);
    assert_eq!(out, "#TRPOS;RYR33EK\r\n");
}
