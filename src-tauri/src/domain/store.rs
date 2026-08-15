use std::collections::{BTreeMap, HashMap};

use thiserror::Error;
use tracing::{debug, info, warn};

use crate::airports::Airport;
use crate::aurora::types::{FlightPlan, TrafficPosition};

use super::activations::ActivationRecord;
use super::board::{Board, BoardUpdate, Column, Columns};
use super::classifier::{classify, Context};
use super::flight::Flight;
use super::ordering;
use super::strip::{ArchiveReason, StripState, TransitionError};
use super::Millis;

#[derive(Debug, Clone, PartialEq)]
pub struct DomainConfig {
    pub ring_radius_nm: f64,
    pub max_position_age: Millis,
    pub radar_dropout_grace: Millis,
    pub archive_ttl: Millis,
    pub parked_ground_speed_kt: u16,
    pub parked_hold: Millis,
    pub flight_plan_max_attempts: u32,
    pub flight_plan_retry_backoff: Millis,
    pub activation_max_age: Millis,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActivationError {
    #[error("no flight {0} is being tracked")]
    UnknownCallsign(String),
    #[error(transparent)]
    Transition(#[from] TransitionError),
}

#[derive(Debug, Clone)]
struct PendingPlan {
    last_seen: Millis,
    attempts: u32,
    next_attempt: Millis,
}

#[derive(Debug, Clone)]
struct Restorable {
    record: ActivationRecord,
    expires_at: Millis,
}

/// Sole owner of the board state. Every mutation goes through here, so classification,
/// ordering and the emitted diff can never drift apart.
pub struct Store {
    airport: Airport,
    config: DomainConfig,
    flights: HashMap<Box<str>, Flight>,
    pending: HashMap<Box<str>, PendingPlan>,
    restorable: HashMap<Box<str>, Restorable>,
    board: Board,
    emitted: Board,
    seq: u64,
}

impl Store {
    pub fn new(airport: Airport, config: DomainConfig) -> Self {
        Self {
            airport,
            config,
            flights: HashMap::new(),
            pending: HashMap::new(),
            restorable: HashMap::new(),
            board: Board::default(),
            emitted: Board::default(),
            seq: 0,
        }
    }

    pub fn airport(&self) -> &Airport {
        &self.airport
    }

    pub fn board(&self) -> &Board {
        &self.board
    }

    pub fn flight(&self, callsign: &str) -> Option<&Flight> {
        self.flights.get(callsign)
    }

    pub fn tracked(&self) -> usize {
        self.flights.len()
    }

    /// `#TR` is authoritative for what exists; a callsign with no plan yet waits here and is
    /// never shown, and dropping out of radar long enough clears it so a return re-arms it.
    pub fn observe_radar<'a>(&mut self, callsigns: impl IntoIterator<Item = &'a str>, now: Millis) {
        for callsign in callsigns {
            if let Some(flight) = self.flights.get_mut(callsign) {
                flight.last_seen = now;
            } else if let Some(pending) = self.pending.get_mut(callsign) {
                pending.last_seen = now;
            } else {
                self.pending.insert(
                    callsign.into(),
                    PendingPlan {
                        last_seen: now,
                        attempts: 0,
                        next_attempt: now,
                    },
                );
            }
        }
    }

    pub fn observe_flight_plan(&mut self, callsign: &str, plan: FlightPlan, now: Millis) {
        if plan.dep.is_empty() && plan.arr.is_empty() {
            self.observe_missing_flight_plan(callsign, now);
            return;
        }
        self.pending.remove(callsign);
        match self.flights.get_mut(callsign) {
            Some(flight) => flight.set_plan(plan, now),
            None => {
                self.flights
                    .insert(callsign.into(), Flight::new(callsign, plan, now));
            }
        }
    }

    pub fn observe_missing_flight_plan(&mut self, callsign: &str, now: Millis) {
        let backoff = self.config.flight_plan_retry_backoff;
        let pending = self
            .pending
            .entry(callsign.into())
            .or_insert_with(|| PendingPlan {
                last_seen: now,
                attempts: 0,
                next_attempt: now,
            });
        pending.attempts = pending.attempts.saturating_add(1);
        pending.last_seen = now;
        pending.next_attempt = now.saturating_add(backoff * u64::from(pending.attempts));
        if pending.attempts >= self.config.flight_plan_max_attempts {
            debug!(callsign, "giving up on a flight plan until reconnect");
        }
    }

    pub fn observe_position(&mut self, callsign: &str, position: TrafficPosition, now: Millis) {
        let centre = self.airport.centre();
        if let Some(flight) = self.flights.get_mut(callsign) {
            flight.set_position(position, centre, now);
        }
    }

    pub fn callsigns_awaiting_flight_plan(&self, now: Millis) -> Vec<Box<str>> {
        self.pending
            .iter()
            .filter(|(_, plan)| {
                plan.attempts < self.config.flight_plan_max_attempts && now >= plan.next_attempt
            })
            .map(|(callsign, _)| callsign.clone())
            .collect()
    }

    /// A reconnect is the one event that makes a previously refused flight plan worth retrying.
    pub fn rearm_flight_plans(&mut self, now: Millis) {
        for pending in self.pending.values_mut() {
            pending.attempts = 0;
            pending.next_attempt = now;
        }
    }

    pub fn activate(&mut self, callsign: &str, now: Millis) -> Result<(), ActivationError> {
        let flight = self
            .flights
            .get_mut(callsign)
            .ok_or_else(|| ActivationError::UnknownCallsign(callsign.to_owned()))?;
        flight.state = flight.state.activate()?;
        flight.activated_at = Some(now);
        self.restorable.remove(callsign);
        Ok(())
    }

    /// Seeds activations saved by a previous run; they apply as the matching strips reappear.
    pub fn restore_from(
        &mut self,
        records: Vec<ActivationRecord>,
        unix_now_ms: u64,
        now: Millis,
    ) -> usize {
        for record in records {
            let age = unix_now_ms.saturating_sub(record.activated_at_unix_ms);
            let Some(remaining) = self.config.activation_max_age.checked_sub(age) else {
                continue;
            };
            self.restorable.insert(
                record.callsign.as_str().into(),
                Restorable {
                    record,
                    expires_at: now.saturating_add(remaining),
                },
            );
        }
        self.restorable.len()
    }

    pub fn activation_records(&self, unix_now_ms: u64, now: Millis) -> Vec<ActivationRecord> {
        self.flights
            .values()
            .filter(|flight| flight.state == StripState::ActivatedDeparture)
            .filter_map(|flight| {
                let elapsed = now.saturating_sub(flight.activated_at?);
                Some(ActivationRecord {
                    callsign: flight.callsign.to_string(),
                    dep: flight.plan.dep.to_string(),
                    eobt: flight.plan.eobt.to_string(),
                    activated_at_unix_ms: unix_now_ms.saturating_sub(elapsed),
                })
            })
            .collect()
    }

    /// One pass of the domain: expire, reclassify, restore, retire, reorder.
    pub fn apply(&mut self, now: Millis, utc_minutes: u16) {
        self.prune(now);
        self.reclassify(now);
        self.apply_restorable(now);
        self.enforce_removal(now);
        self.rebuild(utc_minutes);
    }

    pub fn take_update(&mut self) -> Option<BoardUpdate> {
        let update = self.board.diff_from(&self.emitted, self.seq + 1)?;
        self.seq += 1;
        self.emitted = self.board.clone();
        Some(update)
    }

    fn prune(&mut self, now: Millis) {
        let grace = self.config.radar_dropout_grace;
        let ttl = self.config.archive_ttl;
        self.pending
            .retain(|_, plan| now.saturating_sub(plan.last_seen) <= grace);
        self.flights.retain(|_, flight| {
            flight
                .archived_at
                .is_none_or(|at| now.saturating_sub(at) <= ttl)
        });
    }

    fn reclassify(&mut self, now: Millis) {
        let Self {
            airport,
            config,
            flights,
            ..
        } = self;
        let context = Context {
            airport,
            ring_radius_nm: config.ring_radius_nm,
            max_position_age: config.max_position_age,
        };
        for flight in flights.values_mut() {
            let column = classify(flight, now, context);
            flight.state = flight.state.observe(column);
        }
    }

    fn apply_restorable(&mut self, now: Millis) {
        let Self {
            restorable,
            flights,
            ..
        } = self;
        restorable.retain(|callsign, entry| {
            if now >= entry.expires_at {
                return false;
            }
            let Some(flight) = flights.get_mut(callsign) else {
                return true;
            };
            if flight.state != StripState::Awake {
                return true;
            }
            if &*flight.plan.dep != entry.record.dep.as_str()
                || &*flight.plan.eobt != entry.record.eobt.as_str()
            {
                return false;
            }
            match flight.state.activate() {
                Ok(state) => {
                    flight.state = state;
                    flight.activated_at = Some(now);
                    info!(callsign, "restored activation from the previous session");
                    false
                }
                Err(_) => true,
            }
        });
    }

    fn enforce_removal(&mut self, now: Millis) {
        let Self {
            config, flights, ..
        } = self;
        for flight in flights.values_mut() {
            if flight.state.is_archived() {
                continue;
            }
            if now.saturating_sub(flight.last_seen) > config.radar_dropout_grace {
                archive(flight, ArchiveReason::RadarDropout, now);
                continue;
            }
            if flight.state.is_departure() {
                let outside = flight
                    .fresh_distance_nm(now, config.max_position_age)
                    .is_some_and(|nautical_miles| nautical_miles > config.ring_radius_nm);
                if !flight.on_ground() && outside {
                    archive(flight, ArchiveReason::Departed, now);
                }
                continue;
            }
            if flight.state == StripState::Arrival {
                if !flight.gate().is_empty() {
                    archive(flight, ArchiveReason::Parked, now);
                    continue;
                }
                let stopped = flight.on_ground()
                    && flight
                        .ground_speed()
                        .is_some_and(|knots| knots <= config.parked_ground_speed_kt);
                if stopped {
                    let since = *flight.slow_since.get_or_insert(now);
                    if now.saturating_sub(since) >= config.parked_hold {
                        archive(flight, ArchiveReason::ParkedInferred, now);
                    }
                } else {
                    flight.slow_since = None;
                }
            }
        }
    }

    fn rebuild(&mut self, utc_minutes: u16) {
        let mut columns = Columns::default();
        let mut views = BTreeMap::new();
        for column in Column::ALL {
            let mut members: Vec<&Flight> = self
                .flights
                .values()
                .filter(|flight| flight.state.column() == Some(column))
                .collect();
            ordering::order(column, &mut members, utc_minutes);
            columns.set(
                column,
                members
                    .iter()
                    .map(|flight| flight.callsign.clone())
                    .collect(),
            );
            for flight in members {
                views.insert(flight.callsign.clone(), flight.strip_view());
            }
        }
        self.board = Board { columns, views };
    }
}

fn archive(flight: &mut Flight, reason: ArchiveReason, now: Millis) {
    match flight.state.archive(reason) {
        Ok(state) => {
            flight.state = state;
            flight.archived_at = Some(now);
            info!(callsign = %flight.callsign, ?reason, "strip left the board");
        }
        Err(error) => warn!(callsign = %flight.callsign, %error, "archive refused"),
    }
}
