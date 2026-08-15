use scribe_lib::domain::classifier::AutoColumn;
use scribe_lib::domain::strip::{ArchiveReason, StripState, TransitionError};
use scribe_lib::domain::Column;

const EVERY_STATE: [StripState; 6] = [
    StripState::Offboard,
    StripState::Awake,
    StripState::ActivatedDeparture,
    StripState::Arrival,
    StripState::Transit,
    StripState::Archived(ArchiveReason::Departed),
];

#[test]
fn only_an_awake_strip_can_be_activated() {
    assert_eq!(
        StripState::Awake.activate(),
        Ok(StripState::ActivatedDeparture)
    );
    for state in EVERY_STATE {
        if state == StripState::Awake {
            continue;
        }
        assert_eq!(
            state.activate(),
            Err(TransitionError::NotAwake(state)),
            "{state:?} must not be activatable"
        );
    }
}

#[test]
fn a_transit_can_never_become_an_activated_departure() {
    assert!(StripState::Transit.activate().is_err());
    assert_eq!(
        StripState::Transit.observe(Some(AutoColumn::Transit)),
        StripState::Transit
    );
}

#[test]
fn activation_survives_every_reclassification() {
    let activated = StripState::ActivatedDeparture;
    assert_eq!(activated.observe(None), activated);
    assert_eq!(activated.observe(Some(AutoColumn::Awake)), activated);
    assert_eq!(activated.observe(Some(AutoColumn::Arrival)), activated);
    assert_eq!(activated.observe(Some(AutoColumn::Transit)), activated);
}

#[test]
fn archival_is_terminal() {
    let archived = StripState::Archived(ArchiveReason::Parked);
    assert_eq!(archived.observe(Some(AutoColumn::Arrival)), archived);
    assert_eq!(archived.observe(None), archived);
    assert_eq!(
        archived.archive(ArchiveReason::RadarDropout),
        Err(TransitionError::AlreadyArchived)
    );
    assert_eq!(
        archived.activate(),
        Err(TransitionError::NotAwake(archived))
    );
}

#[test]
fn any_live_strip_can_be_archived_once() {
    for state in EVERY_STATE {
        let outcome = state.archive(ArchiveReason::RadarDropout);
        if state.is_archived() {
            assert!(outcome.is_err());
        } else {
            assert_eq!(
                outcome,
                Ok(StripState::Archived(ArchiveReason::RadarDropout))
            );
        }
    }
}

#[test]
fn an_unactivated_strip_follows_the_classifier() {
    for state in [
        StripState::Offboard,
        StripState::Awake,
        StripState::Arrival,
        StripState::Transit,
    ] {
        assert_eq!(state.observe(None), StripState::Offboard);
        assert_eq!(state.observe(Some(AutoColumn::Awake)), StripState::Awake);
        assert_eq!(
            state.observe(Some(AutoColumn::Arrival)),
            StripState::Arrival
        );
        assert_eq!(
            state.observe(Some(AutoColumn::Transit)),
            StripState::Transit
        );
    }
}

#[test]
fn only_displayed_states_map_to_a_column() {
    assert_eq!(StripState::Awake.column(), Some(Column::Awake));
    assert_eq!(
        StripState::ActivatedDeparture.column(),
        Some(Column::ActivatedDeparture)
    );
    assert_eq!(StripState::Arrival.column(), Some(Column::Arrival));
    assert_eq!(StripState::Transit.column(), Some(Column::Transit));
    assert_eq!(StripState::Offboard.column(), None);
    assert_eq!(StripState::Archived(ArchiveReason::Parked).column(), None);
}

#[test]
fn departure_states_are_the_ones_the_departure_removal_rule_applies_to() {
    assert!(StripState::Awake.is_departure());
    assert!(StripState::ActivatedDeparture.is_departure());
    assert!(!StripState::Arrival.is_departure());
    assert!(!StripState::Transit.is_departure());
    assert!(!StripState::Offboard.is_departure());
}
