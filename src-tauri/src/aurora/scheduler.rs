use std::collections::HashMap;

use crate::domain::Millis;

/// How much slower a callsign is polled after each consecutive failure, capped.
const MAX_PENALTY_STEPS: u32 = 4;

/// Ranked by what the position is actually used for, not merely by whether a strip is visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// The position drives the column's order: arrivals and transits.
    Board,
    /// The position only decides when a strip leaves or joins: departures, and traffic just
    /// outside the ring.
    Near,
    /// Everything else, polled often enough to notice it approaching and no more.
    Far,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchedulerConfig {
    pub budget_per_second: u32,
    pub traffic_list_interval: Millis,
    pub station_interval: Millis,
    pub board_refresh: Millis,
    pub near_refresh: Millis,
    pub far_refresh: Millis,
    pub in_flight_grace: Millis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Task {
    TrafficList,
    Station,
    FlightPlan(Box<str>),
    Position(Box<str>),
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    priority: Priority,
    last_dispatched: Option<Millis>,
    failures: u32,
}

/// Decides what to ask Aurora next. Pure policy: no I/O, no clock of its own.
pub struct Scheduler {
    config: SchedulerConfig,
    entries: HashMap<Box<str>, Entry>,
    plans_in_flight: HashMap<Box<str>, Millis>,
    positions_in_flight: HashMap<Box<str>, Millis>,
    next_traffic_list: Millis,
    next_station: Millis,
    allowance: f64,
    last_refill: Millis,
    dispatched: u64,
}

impl Scheduler {
    pub fn new(config: SchedulerConfig, now: Millis) -> Self {
        Self {
            config,
            entries: HashMap::new(),
            plans_in_flight: HashMap::new(),
            positions_in_flight: HashMap::new(),
            next_traffic_list: now,
            next_station: now,
            allowance: 0.0,
            last_refill: now,
            dispatched: 0,
        }
    }

    pub fn observe(&mut self, callsign: &str, priority: Priority) {
        match self.entries.get_mut(callsign) {
            Some(entry) => entry.priority = priority,
            None => {
                self.entries.insert(
                    callsign.into(),
                    Entry {
                        priority,
                        last_dispatched: None,
                        failures: 0,
                    },
                );
            }
        }
    }

    pub fn retain(&mut self, keep: impl Fn(&str) -> bool) {
        self.entries.retain(|callsign, _| keep(callsign));
    }

    pub fn completed_flight_plan(&mut self, callsign: &str) {
        self.plans_in_flight.remove(callsign);
    }

    pub fn completed_position(&mut self, callsign: &str) {
        self.positions_in_flight.remove(callsign);
        if let Some(entry) = self.entries.get_mut(callsign) {
            entry.failures = 0;
        }
    }

    /// A timeout or `@ERR` for one callsign must not keep costing the whole budget.
    pub fn penalise(&mut self, callsign: &str) {
        self.positions_in_flight.remove(callsign);
        if let Some(entry) = self.entries.get_mut(callsign) {
            entry.failures = entry.failures.saturating_add(1);
        }
    }

    /// A reconnect invalidates everything in flight and makes a full refresh worthwhile.
    pub fn reset(&mut self, now: Millis) {
        self.plans_in_flight.clear();
        self.positions_in_flight.clear();
        for entry in self.entries.values_mut() {
            entry.last_dispatched = None;
            entry.failures = 0;
        }
        self.next_traffic_list = now;
        self.next_station = now;
        self.allowance = 0.0;
        self.last_refill = now;
    }

    pub fn dispatched(&self) -> u64 {
        self.dispatched
    }

    pub fn tracked(&self) -> usize {
        self.entries.len()
    }

    /// Everything due now, in priority order, capped by the request budget.
    pub fn take_due(&mut self, now: Millis, flight_plans: &[Box<str>]) -> Vec<Task> {
        self.expire_in_flight(now);
        self.refill(now);

        let mut tasks = Vec::new();
        if now >= self.next_traffic_list && self.spend() {
            self.next_traffic_list = now.saturating_add(self.config.traffic_list_interval);
            tasks.push(Task::TrafficList);
        }
        if now >= self.next_station && self.spend() {
            self.next_station = now.saturating_add(self.config.station_interval);
            tasks.push(Task::Station);
        }

        // Flight plans come first: without one a strip cannot exist at all.
        for callsign in flight_plans {
            if self.plans_in_flight.contains_key(&**callsign) {
                continue;
            }
            if !self.spend() {
                return tasks;
            }
            self.plans_in_flight.insert(callsign.clone(), now);
            tasks.push(Task::FlightPlan(callsign.clone()));
        }

        for callsign in self.due_positions(now) {
            if !self.spend() {
                break;
            }
            self.positions_in_flight.insert(callsign.clone(), now);
            if let Some(entry) = self.entries.get_mut(&*callsign) {
                entry.last_dispatched = Some(now);
            }
            tasks.push(Task::Position(callsign));
        }
        tasks
    }

    /// Only the most overdue callsigns the budget can pay for are cloned out.
    fn due_positions(&self, now: Millis) -> Vec<Box<str>> {
        let affordable = self.allowance.max(0.0) as usize;
        if affordable == 0 {
            return Vec::new();
        }
        let mut due: Vec<(Millis, &str)> = self
            .entries
            .iter()
            .filter(|(callsign, _)| !self.positions_in_flight.contains_key(&***callsign))
            .filter_map(|(callsign, entry)| {
                let due_at = self.due_at(entry);
                (now >= due_at).then_some((due_at, &**callsign))
            })
            .collect();

        if due.len() > affordable {
            due.select_nth_unstable(affordable);
            due.truncate(affordable);
        }
        due.sort_unstable();
        due.into_iter()
            .map(|(_, callsign)| Box::<str>::from(callsign))
            .collect()
    }

    fn due_at(&self, entry: &Entry) -> Millis {
        let Some(last) = entry.last_dispatched else {
            return 0;
        };
        let interval = self.interval(entry.priority);
        let penalty = interval.saturating_mul(u64::from(entry.failures.min(MAX_PENALTY_STEPS)));
        last.saturating_add(interval).saturating_add(penalty)
    }

    fn interval(&self, priority: Priority) -> Millis {
        match priority {
            Priority::Board => self.config.board_refresh,
            Priority::Near => self.config.near_refresh,
            Priority::Far => self.config.far_refresh,
        }
    }

    fn expire_in_flight(&mut self, now: Millis) {
        let grace = self.config.in_flight_grace;
        self.plans_in_flight
            .retain(|_, at| now.saturating_sub(*at) <= grace);
        self.positions_in_flight
            .retain(|_, at| now.saturating_sub(*at) <= grace);
    }

    fn refill(&mut self, now: Millis) {
        let elapsed = now.saturating_sub(self.last_refill);
        if elapsed == 0 {
            return;
        }
        let ceiling = f64::from(self.config.budget_per_second);
        let earned = elapsed as f64 * ceiling / 1000.0;
        self.allowance = (self.allowance + earned).min(ceiling);
        self.last_refill = now;
    }

    fn spend(&mut self) -> bool {
        if self.allowance < 1.0 {
            return false;
        }
        self.allowance -= 1.0;
        self.dispatched = self.dispatched.saturating_add(1);
        true
    }
}
