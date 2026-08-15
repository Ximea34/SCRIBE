use crate::aurora::types::{FlightPlan, TrafficPosition};

use super::board::StripView;
use super::geo::{distance_nm, LatLon};
use super::ordering::parse_eobt;
use super::strip::StripState;
use super::Millis;

/// A flight plan plus its latest position and strip lifecycle. Derived keys are cached on
/// ingest so the per-tick sort reads fields instead of re-parsing strings.
#[derive(Debug, Clone, PartialEq)]
pub struct Flight {
    pub callsign: Box<str>,
    pub plan: FlightPlan,
    pub position: Option<TrafficPosition>,
    pub state: StripState,
    pub eobt_minutes: Option<u16>,
    pub distance_nm: Option<f64>,
    pub first_seen: Millis,
    pub last_seen: Millis,
    pub fp_fetched_at: Millis,
    pub position_at: Option<Millis>,
    pub activated_at: Option<Millis>,
    pub archived_at: Option<Millis>,
    pub slow_since: Option<Millis>,
}

impl Flight {
    pub fn new(callsign: &str, plan: FlightPlan, now: Millis) -> Self {
        Self {
            callsign: callsign.into(),
            eobt_minutes: parse_eobt(&plan.eobt),
            plan,
            position: None,
            state: StripState::Offboard,
            distance_nm: None,
            first_seen: now,
            last_seen: now,
            fp_fetched_at: now,
            position_at: None,
            activated_at: None,
            archived_at: None,
            slow_since: None,
        }
    }

    pub fn set_plan(&mut self, plan: FlightPlan, now: Millis) {
        self.eobt_minutes = parse_eobt(&plan.eobt);
        self.plan = plan;
        self.fp_fetched_at = now;
    }

    pub fn set_position(&mut self, position: TrafficPosition, centre: LatLon, now: Millis) {
        self.distance_nm = position_latlon(&position).map(|at| distance_nm(at, centre));
        self.position = Some(position);
        self.position_at = Some(now);
        self.last_seen = now;
    }

    pub fn latlon(&self) -> Option<LatLon> {
        self.position.as_ref().and_then(position_latlon)
    }

    pub fn altitude(&self) -> Option<i32> {
        self.position.as_ref()?.altitude
    }

    pub fn ground_speed(&self) -> Option<u16> {
        self.position.as_ref()?.ground_speed
    }

    pub fn on_ground(&self) -> bool {
        self.position.as_ref().is_some_and(|p| p.on_ground)
    }

    pub fn gate(&self) -> &str {
        self.position.as_ref().map_or("", |p| &p.gate)
    }

    /// Displayed height above the field; a constant offset, so it never affects ordering.
    pub fn height_above_field(&self, elevation_ft: i32) -> Option<i32> {
        Some(self.altitude()? - elevation_ft)
    }

    pub fn has_fresh_position(&self, now: Millis, max_age: Millis) -> bool {
        self.position_at
            .is_some_and(|at| now.saturating_sub(at) <= max_age)
    }

    /// Distance to the field, or `None` when there is no position or it has gone stale.
    pub fn fresh_distance_nm(&self, now: Millis, max_age: Millis) -> Option<f64> {
        self.has_fresh_position(now, max_age)
            .then_some(self.distance_nm)
            .flatten()
    }

    pub fn strip_view(&self) -> StripView {
        StripView {
            callsign: self.callsign.clone(),
            adep: self.plan.dep.clone(),
            ades: self.plan.arr.clone(),
            rules: self.plan.rules.clone(),
        }
    }
}

fn position_latlon(position: &TrafficPosition) -> Option<LatLon> {
    Some(LatLon::new(position.lat?, position.lon?))
}
