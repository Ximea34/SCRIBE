mod common;

use common::{admit, column_of, offset, plan, position, store, Where, LFLL_LAT, LFLL_LON};
use scribe_lib::domain::activations::ActivationRecord;
use scribe_lib::domain::strip::ArchiveReason;
use scribe_lib::domain::{ActivationError, Column, Store, StripState};

const NOON: u16 = 12 * 60;
const GRACE: u64 = 30_000;
const PARKED_HOLD: u64 = 15_000;
const ARCHIVE_TTL: u64 = 120_000;
const UNIX_NOW: u64 = 1_760_000_000_000;

fn tick(store: &mut Store, callsign: &str, at: Option<&Where>, now: u64) {
    store.observe_radar([callsign], now);
    if let Some(at) = at {
        store.observe_position(callsign, position(callsign, at), now);
    }
    store.apply(now, NOON);
}

fn state_of(store: &Store, callsign: &str) -> Option<StripState> {
    store.flight(callsign).map(|flight| flight.state)
}

#[test]
fn a_callsign_without_a_flight_plan_never_reaches_the_board() {
    let mut store = store();
    store.observe_radar(["FGXYZ"], 0);
    store.observe_missing_flight_plan("FGXYZ", 0);
    store.apply(0, NOON);

    assert_eq!(store.tracked(), 0);
    assert!(store.board().columns.callsigns().next().is_none());
    assert!(store.board().views.is_empty());
}

#[test]
fn an_empty_flight_plan_counts_as_no_flight_plan() {
    let mut store = store();
    store.observe_radar(["FGXYZ"], 0);
    store.observe_flight_plan("FGXYZ", plan("FGXYZ", "", "", "", ""), 0);
    store.apply(0, NOON);

    assert_eq!(store.tracked(), 0);
    assert_eq!(store.callsigns_awaiting_flight_plan(3_000).len(), 1);
}

#[test]
fn a_position_for_a_planless_callsign_is_ignored() {
    let mut store = store();
    store.observe_radar(["FGXYZ"], 0);
    store.observe_position(
        "FGXYZ",
        position("FGXYZ", &Where::airborne(LFLL_LAT, LFLL_LON, 3000)),
        0,
    );
    store.apply(0, NOON);

    assert_eq!(store.tracked(), 0);
}

#[test]
fn flight_plan_attempts_are_bounded_and_a_reconnect_rearms_them() {
    let mut store = store();
    store.observe_radar(["FGXYZ"], 0);

    let mut now = 0;
    for _ in 0..5 {
        store.observe_missing_flight_plan("FGXYZ", now);
        now += 20_000;
    }
    assert!(
        store.callsigns_awaiting_flight_plan(now).is_empty(),
        "five refusals is enough; stop spending the request budget"
    );

    store.rearm_flight_plans(now);
    assert_eq!(store.callsigns_awaiting_flight_plan(now).len(), 1);
}

#[test]
fn a_retry_waits_for_its_backoff() {
    let mut store = store();
    store.observe_radar(["FGXYZ"], 0);
    store.observe_missing_flight_plan("FGXYZ", 0);

    assert!(store.callsigns_awaiting_flight_plan(1_999).is_empty());
    assert_eq!(store.callsigns_awaiting_flight_plan(2_000).len(), 1);
}

#[test]
fn a_pending_callsign_is_forgotten_after_a_dropout_so_a_return_retries_it() {
    let mut store = store();
    store.observe_radar(["FGXYZ"], 0);
    store.observe_missing_flight_plan("FGXYZ", 0);
    for _ in 0..5 {
        store.observe_missing_flight_plan("FGXYZ", 0);
    }
    assert!(store.callsigns_awaiting_flight_plan(2_000).is_empty());

    store.apply(GRACE + 1, NOON);
    store.observe_radar(["FGXYZ"], GRACE + 1);
    assert_eq!(store.callsigns_awaiting_flight_plan(GRACE + 1).len(), 1);
}

#[test]
fn a_departure_lands_in_eveilles_and_activation_moves_it_across() {
    let mut store = store();
    admit(
        &mut store,
        "AFR1234",
        plan("AFR1234", "LFLL", "LFPG", "1215", "I"),
        None,
        0,
    );
    store.apply(0, NOON);
    assert_eq!(column_of(&store, "AFR1234"), Some(Column::Awake));

    store
        .activate("AFR1234", 0)
        .expect("an awake strip activates");
    store.apply(0, NOON);

    assert_eq!(
        column_of(&store, "AFR1234"),
        Some(Column::ActivatedDeparture)
    );
    assert!(store.board().columns.awake.is_empty());
    assert_eq!(store.board().columns.activated_departures.len(), 1);
}

#[test]
fn a_transit_can_never_be_activated() {
    let mut store = store();
    let (lat, lon) = offset(90.0, 8.0);
    admit(
        &mut store,
        "FGEKO",
        plan("FGEKO", "LFNE", "LFMU", "1200", "V"),
        Some(&Where::airborne(lat, lon, 7000)),
        0,
    );
    store.apply(0, NOON);
    assert_eq!(column_of(&store, "FGEKO"), Some(Column::Transit));

    assert!(matches!(
        store.activate("FGEKO", 0),
        Err(ActivationError::Transition(_))
    ));
    assert_eq!(column_of(&store, "FGEKO"), Some(Column::Transit));
}

#[test]
fn activating_an_untracked_callsign_is_rejected() {
    let mut store = store();
    assert!(matches!(
        store.activate("NOPE123", 0),
        Err(ActivationError::UnknownCallsign(_))
    ));
}

#[test]
fn an_arrival_enters_automatically_without_activation() {
    let mut store = store();
    let (lat, lon) = offset(180.0, 9.0);
    admit(
        &mut store,
        "AFR9876",
        plan("AFR9876", "LFPG", "LFLL", "1100", "I"),
        Some(&Where::airborne(lat, lon, 5000)),
        0,
    );
    store.apply(0, NOON);

    assert_eq!(column_of(&store, "AFR9876"), Some(Column::Arrival));
    assert!(matches!(
        store.activate("AFR9876", 0),
        Err(ActivationError::Transition(_))
    ));
}

#[test]
fn a_departure_leaves_only_once_it_is_airborne_and_outside_the_ring() {
    let mut store = store();
    admit(
        &mut store,
        "AFR1234",
        plan("AFR1234", "LFLL", "LFPG", "1215", "I"),
        Some(&Where::grounded(LFLL_LAT, LFLL_LON)),
        0,
    );
    store.apply(0, NOON);
    assert_eq!(column_of(&store, "AFR1234"), Some(Column::Awake));

    let (lat, lon) = offset(0.0, 10.0);
    tick(
        &mut store,
        "AFR1234",
        Some(&Where::airborne(lat, lon, 6000)),
        1_000,
    );
    assert_eq!(
        column_of(&store, "AFR1234"),
        Some(Column::Awake),
        "airborne but still inside the ring"
    );

    let (lat, lon) = offset(0.0, 25.0);
    tick(
        &mut store,
        "AFR1234",
        Some(&Where::airborne(lat, lon, 14_000)),
        2_000,
    );
    assert_eq!(column_of(&store, "AFR1234"), None);
    assert_eq!(
        state_of(&store, "AFR1234"),
        Some(StripState::Archived(ArchiveReason::Departed))
    );
}

#[test]
fn a_departure_still_on_the_ground_outside_the_ring_stays() {
    let mut store = store();
    let (lat, lon) = offset(0.0, 25.0);
    admit(
        &mut store,
        "AFR1234",
        plan("AFR1234", "LFLL", "LFPG", "1215", "I"),
        Some(&Where::grounded(lat, lon)),
        0,
    );
    store.apply(0, NOON);

    assert_eq!(column_of(&store, "AFR1234"), Some(Column::Awake));
}

#[test]
fn an_arrival_leaves_as_soon_as_a_stand_is_reported() {
    let mut store = store();
    let (lat, lon) = offset(180.0, 6.0);
    admit(
        &mut store,
        "AFR9876",
        plan("AFR9876", "LFPG", "LFLL", "1100", "I"),
        Some(&Where::airborne(lat, lon, 4000)),
        0,
    );
    store.apply(0, NOON);
    assert_eq!(column_of(&store, "AFR9876"), Some(Column::Arrival));

    tick(
        &mut store,
        "AFR9876",
        Some(&Where::grounded(LFLL_LAT, LFLL_LON).gate("A12")),
        1_000,
    );
    assert_eq!(
        state_of(&store, "AFR9876"),
        Some(StripState::Archived(ArchiveReason::Parked))
    );
}

#[test]
fn an_arrival_leaves_through_the_ground_fallback_when_no_stand_is_reported() {
    let mut store = store();
    let (lat, lon) = offset(180.0, 6.0);
    admit(
        &mut store,
        "AFR9876",
        plan("AFR9876", "LFPG", "LFLL", "1100", "I"),
        Some(&Where::airborne(lat, lon, 4000)),
        0,
    );
    store.apply(0, NOON);

    let stopped = Where::grounded(LFLL_LAT, LFLL_LON);
    tick(&mut store, "AFR9876", Some(&stopped), 1_000);
    assert_eq!(column_of(&store, "AFR9876"), Some(Column::Arrival));

    tick(
        &mut store,
        "AFR9876",
        Some(&stopped),
        1_000 + PARKED_HOLD - 1,
    );
    assert_eq!(column_of(&store, "AFR9876"), Some(Column::Arrival));

    tick(&mut store, "AFR9876", Some(&stopped), 1_000 + PARKED_HOLD);
    assert_eq!(
        state_of(&store, "AFR9876"),
        Some(StripState::Archived(ArchiveReason::ParkedInferred))
    );
}

#[test]
fn the_ground_fallback_restarts_when_the_aircraft_moves_again() {
    let mut store = store();
    let (lat, lon) = offset(180.0, 6.0);
    admit(
        &mut store,
        "AFR9876",
        plan("AFR9876", "LFPG", "LFLL", "1100", "I"),
        Some(&Where::airborne(lat, lon, 4000)),
        0,
    );
    store.apply(0, NOON);

    let stopped = Where::grounded(LFLL_LAT, LFLL_LON);
    let taxiing = Where::grounded(LFLL_LAT, LFLL_LON).speed(18);

    tick(&mut store, "AFR9876", Some(&stopped), 1_000);
    tick(&mut store, "AFR9876", Some(&taxiing), 5_000);
    tick(&mut store, "AFR9876", Some(&stopped), 6_000);
    tick(
        &mut store,
        "AFR9876",
        Some(&stopped),
        6_000 + PARKED_HOLD - 1,
    );
    assert_eq!(
        column_of(&store, "AFR9876"),
        Some(Column::Arrival),
        "the hold restarts from the moment it stopped again"
    );

    tick(&mut store, "AFR9876", Some(&stopped), 6_000 + PARKED_HOLD);
    assert_eq!(column_of(&store, "AFR9876"), None);
}

#[test]
fn a_radar_dropout_waits_for_the_grace_period_before_archiving() {
    let mut store = store();
    admit(
        &mut store,
        "AFR1234",
        plan("AFR1234", "LFLL", "LFPG", "1215", "I"),
        None,
        0,
    );
    store.apply(0, NOON);

    store.apply(GRACE, NOON);
    assert_eq!(column_of(&store, "AFR1234"), Some(Column::Awake));

    store.apply(GRACE + 1, NOON);
    assert_eq!(
        state_of(&store, "AFR1234"),
        Some(StripState::Archived(ArchiveReason::RadarDropout))
    );
}

#[test]
fn an_archived_flight_is_kept_briefly_then_dropped() {
    let mut store = store();
    admit(
        &mut store,
        "AFR1234",
        plan("AFR1234", "LFLL", "LFPG", "1215", "I"),
        None,
        0,
    );
    store.apply(0, NOON);
    store.apply(GRACE + 1, NOON);
    assert_eq!(store.tracked(), 1, "kept so a quick return is recognised");

    store.apply(GRACE + 1 + ARCHIVE_TTL + 1, NOON);
    assert_eq!(store.tracked(), 0);
}

#[test]
fn the_diff_stays_silent_when_nothing_changed() {
    let mut store = store();
    admit(
        &mut store,
        "AFR1234",
        plan("AFR1234", "LFLL", "LFPG", "1215", "I"),
        None,
        0,
    );
    store.apply(0, NOON);

    let first = store.take_update().expect("the first board is a change");
    assert_eq!(first.seq, 1);
    assert_eq!(first.upserted.len(), 1);
    assert_eq!(first.columns.map(|columns| columns.awake.len()), Some(1));

    store.apply(0, NOON);
    assert!(store.take_update().is_none());
    store.apply(1_000, NOON);
    assert!(store.take_update().is_none());
}

#[test]
fn the_diff_carries_only_what_moved() {
    let mut store = store();
    admit(
        &mut store,
        "AFR1234",
        plan("AFR1234", "LFLL", "LFPG", "1215", "I"),
        None,
        0,
    );
    store.apply(0, NOON);
    let _ = store.take_update();

    admit(
        &mut store,
        "BAW0001",
        plan("BAW0001", "LFLL", "EGLL", "1220", "I"),
        None,
        0,
    );
    store.apply(0, NOON);

    let update = store.take_update().expect("a new strip is a change");
    assert_eq!(update.seq, 2);
    assert_eq!(update.upserted.len(), 1);
    assert_eq!(
        update.upserted.first().map(|view| &*view.callsign),
        Some("BAW0001")
    );
    assert!(update.removed.is_empty());
    assert_eq!(update.columns.map(|columns| columns.awake.len()), Some(2));
}

#[test]
fn leaving_the_board_shows_up_as_a_removal() {
    let mut store = store();
    admit(
        &mut store,
        "AFR1234",
        plan("AFR1234", "LFLL", "LFPG", "1215", "I"),
        None,
        0,
    );
    store.apply(0, NOON);
    let _ = store.take_update();

    store.apply(GRACE + 1, NOON);
    let update = store.take_update().expect("archival is a change");
    assert_eq!(
        update.removed.iter().map(|c| &**c).collect::<Vec<_>>(),
        ["AFR1234"]
    );
    assert!(update.upserted.is_empty());
}

#[test]
fn the_strip_view_carries_exactly_the_four_rendered_cells() {
    let mut store = store();
    admit(
        &mut store,
        "RYR33EK",
        plan("RYR33EK", "LFLL", "LFKJ", "1215", "I"),
        None,
        0,
    );
    store.apply(0, NOON);

    let view = store
        .board()
        .views
        .get("RYR33EK")
        .expect("the strip is on the board");
    assert_eq!(&*view.callsign, "RYR33EK");
    assert_eq!(&*view.adep, "LFLL");
    assert_eq!(&*view.ades, "LFKJ");
    assert_eq!(&*view.rules, "I");
}

#[test]
fn an_activation_survives_a_restart() {
    let mut store = store();
    admit(
        &mut store,
        "AFR1234",
        plan("AFR1234", "LFLL", "LFPG", "1215", "I"),
        None,
        0,
    );
    store.apply(0, NOON);
    store.activate("AFR1234", 0).expect("activation");
    store.apply(0, NOON);

    let saved = store.activation_records(UNIX_NOW, 0);
    assert_eq!(saved.len(), 1);

    let mut restarted = common::store();
    assert_eq!(restarted.restore_from(saved, UNIX_NOW, 0), 1);
    admit(
        &mut restarted,
        "AFR1234",
        plan("AFR1234", "LFLL", "LFPG", "1215", "I"),
        None,
        0,
    );
    restarted.apply(0, NOON);

    assert_eq!(
        column_of(&restarted, "AFR1234"),
        Some(Column::ActivatedDeparture)
    );
}

#[test]
fn a_recycled_callsign_does_not_inherit_yesterdays_activation() {
    let saved = vec![ActivationRecord {
        callsign: "AFR1234".to_owned(),
        dep: "LFLL".to_owned(),
        eobt: "1215".to_owned(),
        activated_at_unix_ms: UNIX_NOW,
    }];

    let mut store = store();
    assert_eq!(store.restore_from(saved, UNIX_NOW, 0), 1);
    admit(
        &mut store,
        "AFR1234",
        plan("AFR1234", "LFLL", "LFPG", "0730", "I"),
        None,
        0,
    );
    store.apply(0, NOON);

    assert_eq!(
        column_of(&store, "AFR1234"),
        Some(Column::Awake),
        "a different EOBT means a different flight"
    );
}

#[test]
fn an_activation_older_than_the_cutoff_is_not_restored() {
    let saved = vec![ActivationRecord {
        callsign: "AFR1234".to_owned(),
        dep: "LFLL".to_owned(),
        eobt: "1215".to_owned(),
        activated_at_unix_ms: UNIX_NOW - 7 * 60 * 60 * 1_000,
    }];

    let mut store = store();
    assert_eq!(store.restore_from(saved, UNIX_NOW, 0), 0);
}
