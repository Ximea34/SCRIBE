use scribe_lib::aurora::scheduler::{Priority, Scheduler, SchedulerConfig, Task};

const NEVER: u64 = 1_000_000;

fn config(budget_per_second: u32) -> SchedulerConfig {
    SchedulerConfig {
        budget_per_second,
        traffic_list_interval: NEVER,
        station_interval: NEVER,
        board_refresh: 1_000,
        near_refresh: 2_000,
        far_refresh: 4_000,
        in_flight_grace: 4_000,
    }
}

/// Consumes the one-off traffic list and station requests both queued at construction.
fn warmed(budget_per_second: u32) -> Scheduler {
    let mut scheduler = Scheduler::new(config(budget_per_second), 0);
    let opening = scheduler.take_due(1_000, &[]);
    assert!(opening.contains(&Task::TrafficList));
    assert!(opening.contains(&Task::Station));
    scheduler
}

fn positions(tasks: &[Task]) -> Vec<String> {
    tasks
        .iter()
        .filter_map(|task| match task {
            Task::Position(callsign) => Some(callsign.to_string()),
            _ => None,
        })
        .collect()
}

fn plans(tasks: &[Task]) -> Vec<String> {
    tasks
        .iter()
        .filter_map(|task| match task {
            Task::FlightPlan(callsign) => Some(callsign.to_string()),
            _ => None,
        })
        .collect()
}

fn boxed(callsigns: &[&str]) -> Vec<Box<str>> {
    callsigns.iter().map(|c| Box::<str>::from(*c)).collect()
}

#[test]
fn nothing_is_dispatched_before_the_budget_has_accrued() {
    let mut scheduler = Scheduler::new(config(10), 0);
    assert!(
        scheduler.take_due(0, &[]).is_empty(),
        "an empty bucket buys nothing, not even the traffic list"
    );
    assert_eq!(scheduler.dispatched(), 0);
}

#[test]
fn the_traffic_list_and_station_are_asked_for_first() {
    let mut scheduler = Scheduler::new(config(10), 0);
    scheduler.observe("AFR1234", Priority::Board);

    let tasks = scheduler.take_due(1_000, &[]);
    assert_eq!(tasks.first(), Some(&Task::TrafficList));
    assert_eq!(tasks.get(1), Some(&Task::Station));
}

#[test]
fn the_budget_caps_what_one_tick_can_ask_for() {
    let mut scheduler = Scheduler::new(config(10), 0);
    for index in 0..50 {
        scheduler.observe(&format!("TFC{index:04}"), Priority::Board);
    }

    let tasks = scheduler.take_due(1_000, &[]);
    assert_eq!(tasks.len(), 10, "one second of budget is ten requests");
    assert_eq!(positions(&tasks).len(), 8, "after the list and the station");
    assert_eq!(scheduler.dispatched(), 10);
}

#[test]
fn the_bucket_never_accrues_more_than_one_second_of_budget() {
    let mut scheduler = Scheduler::new(config(10), 0);
    for index in 0..50 {
        scheduler.observe(&format!("TFC{index:04}"), Priority::Board);
    }

    let tasks = scheduler.take_due(60_000, &[]);
    assert_eq!(
        tasks.len(),
        10,
        "a long idle gap must not licence a burst of sixty seconds' worth"
    );
}

#[test]
fn flight_plans_are_bought_before_positions() {
    let mut scheduler = warmed(10);
    for index in 0..20 {
        scheduler.observe(&format!("TFC{index:04}"), Priority::Board);
    }

    let wanted = boxed(&["NEW00001", "NEW00002", "NEW00003"]);
    let tasks = scheduler.take_due(2_000, &wanted);

    assert_eq!(plans(&tasks).len(), 3);
    assert_eq!(
        positions(&tasks).len(),
        7,
        "positions get whatever the plans leave"
    );
}

#[test]
fn board_traffic_is_polled_faster_than_distant_traffic() {
    let mut scheduler = warmed(50);
    scheduler.observe("ONBOARD1", Priority::Board);
    scheduler.observe("NEARBY01", Priority::Near);
    scheduler.observe("FARAWAY1", Priority::Far);

    let first = scheduler.take_due(1_000, &[]);
    assert_eq!(positions(&first).len(), 3, "everything is due when unseen");
    for callsign in ["ONBOARD1", "NEARBY01", "FARAWAY1"] {
        scheduler.completed_position(callsign);
    }

    assert_eq!(positions(&scheduler.take_due(2_000, &[])), ["ONBOARD1"]);
    scheduler.completed_position("ONBOARD1");

    let mut at_three = positions(&scheduler.take_due(3_000, &[]));
    at_three.sort_unstable();
    assert_eq!(at_three, ["NEARBY01", "ONBOARD1"]);
    scheduler.completed_position("ONBOARD1");
    scheduler.completed_position("NEARBY01");

    // By now the board strip has been asked for three times, the distant one once.
    let mut at_five = positions(&scheduler.take_due(5_000, &[]));
    at_five.sort_unstable();
    assert_eq!(at_five, ["FARAWAY1", "NEARBY01", "ONBOARD1"]);
}

#[test]
fn the_same_callsign_is_never_in_flight_twice() {
    let mut scheduler = warmed(50);
    scheduler.observe("AFR1234", Priority::Board);

    assert_eq!(positions(&scheduler.take_due(1_000, &[])), ["AFR1234"]);
    assert!(
        positions(&scheduler.take_due(4_999, &[])).is_empty(),
        "long overdue, but the first request has not come back"
    );
}

#[test]
fn an_abandoned_request_stops_blocking_after_the_grace_period() {
    let mut scheduler = warmed(50);
    scheduler.observe("AFR1234", Priority::Board);
    let _ = scheduler.take_due(1_000, &[]);

    assert!(positions(&scheduler.take_due(5_000, &[])).is_empty());
    assert_eq!(positions(&scheduler.take_due(5_001, &[])), ["AFR1234"]);
}

#[test]
fn a_pending_flight_plan_is_not_asked_for_again() {
    let mut scheduler = warmed(50);
    let wanted = boxed(&["FGXYZ"]);

    assert_eq!(plans(&scheduler.take_due(1_000, &wanted)), ["FGXYZ"]);
    assert!(plans(&scheduler.take_due(1_100, &wanted)).is_empty());

    scheduler.completed_flight_plan("FGXYZ");
    assert_eq!(plans(&scheduler.take_due(1_200, &wanted)), ["FGXYZ"]);
}

#[test]
fn repeated_failures_back_a_callsign_off() {
    let mut scheduler = warmed(50);
    scheduler.observe("BROKEN01", Priority::Board);
    scheduler.observe("HEALTHY1", Priority::Board);

    let _ = scheduler.take_due(1_000, &[]);
    scheduler.penalise("BROKEN01");
    scheduler.completed_position("HEALTHY1");

    // The healthy one is due one interval later, the failing one an interval after that.
    assert_eq!(positions(&scheduler.take_due(2_000, &[])), ["HEALTHY1"]);
    scheduler.completed_position("HEALTHY1");

    let mut at_three = positions(&scheduler.take_due(3_000, &[]));
    at_three.sort_unstable();
    assert_eq!(at_three, ["BROKEN01", "HEALTHY1"]);
}

#[test]
fn a_success_clears_the_penalty() {
    let mut scheduler = warmed(50);
    scheduler.observe("AFR1234", Priority::Board);

    let _ = scheduler.take_due(1_000, &[]);
    scheduler.penalise("AFR1234");
    let _ = scheduler.take_due(3_000, &[]);
    scheduler.completed_position("AFR1234");

    assert_eq!(
        positions(&scheduler.take_due(4_000, &[])),
        ["AFR1234"],
        "back to the normal interval once it answers again"
    );
}

#[test]
fn a_reconnect_makes_everything_due_again() {
    let mut scheduler = warmed(50);
    scheduler.observe("AFR1234", Priority::Board);
    let _ = scheduler.take_due(1_000, &[]);
    scheduler.completed_position("AFR1234");

    assert!(positions(&scheduler.take_due(1_500, &[])).is_empty());

    scheduler.reset(1_500);
    let tasks = scheduler.take_due(2_500, &[]);
    assert!(tasks.contains(&Task::TrafficList));
    assert!(tasks.contains(&Task::Station));
    assert_eq!(positions(&tasks), ["AFR1234"]);
}

#[test]
fn callsigns_that_leave_the_registry_are_forgotten() {
    let mut scheduler = warmed(50);
    scheduler.observe("STAYS001", Priority::Board);
    scheduler.observe("GOES0001", Priority::Board);
    assert_eq!(scheduler.tracked(), 2);

    scheduler.retain(|callsign| callsign == "STAYS001");
    assert_eq!(scheduler.tracked(), 1);
    assert_eq!(positions(&scheduler.take_due(1_000, &[])), ["STAYS001"]);
}

#[test]
fn the_most_overdue_traffic_is_served_first() {
    let mut scheduler = warmed(50);
    scheduler.observe("EARLY001", Priority::Board);
    let _ = scheduler.take_due(1_000, &[]);
    scheduler.completed_position("EARLY001");

    scheduler.observe("LATE0001", Priority::Board);
    let _ = scheduler.take_due(1_100, &[]);
    scheduler.completed_position("LATE0001");

    let tasks = scheduler.take_due(2_500, &[]);
    assert_eq!(
        positions(&tasks),
        ["EARLY001", "LATE0001"],
        "ordered by how long each has waited"
    );
}
