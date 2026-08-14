use std::net::SocketAddr;
use std::time::Duration;

use mock_aurora::{MockAurora, Quirk, Script, Traffic};
use scribe_lib::aurora::{AuroraClient, AuroraError, ClientConfig, ConnectionState};

fn fleet() -> Vec<Traffic> {
    vec![
        Traffic::new("AFR1234").route("LFLL", "LFPG").eobt("1215"),
        Traffic::new("RYR33EK").route("LFPG", "LFLL").rules("I"),
        Traffic::new("FGEKO")
            .route("LFNE", "LFMU")
            .rules("V")
            .on_ground(true)
            .gate("A12"),
    ]
}

fn config(addr: SocketAddr) -> ClientConfig {
    ClientConfig {
        addr,
        request_timeout: Duration::from_millis(400),
        backoff_initial: Duration::from_millis(10),
        backoff_max: Duration::from_millis(50),
    }
}

async fn connect(script: Script) -> (MockAurora, AuroraClient) {
    let server = MockAurora::start(script).await.expect("mock should bind");
    let (client, _join) = AuroraClient::spawn(config(server.addr()));
    (server, client)
}

async fn wait_connected(client: &AuroraClient) {
    let mut state = client.watch_state();
    while *state.borrow_and_update() != ConnectionState::Connected {
        state.changed().await.expect("state channel stays open");
    }
}

#[tokio::test]
async fn round_trips_every_command_used_by_the_board() {
    let (_server, client) = connect(Script::new("LFLL_TWR").with_traffics(fleet())).await;

    assert_eq!(&*client.conn().await.expect("conn"), "LFLL_TWR");
    assert_eq!(client.selected().await.expect("seltfc"), None);

    let plan = client.flight_plan("AFR1234").await.expect("fp");
    assert_eq!(&*plan.callsign, "AFR1234");
    assert_eq!(&*plan.dep, "LFLL");
    assert_eq!(&*plan.arr, "LFPG");
    assert_eq!(&*plan.eobt, "1215");
    assert_eq!(&*plan.rules, "I");

    let position = client.traffic_position("FGEKO").await.expect("trpos");
    assert_eq!(&*position.callsign, "FGEKO");
    assert!(position.on_ground);
    assert_eq!(&*position.gate, "A12");
    assert!(position.lat.is_some());

    let list = client.traffic_list().await.expect("tr");
    assert_eq!(
        list.iter().map(|c| &**c).collect::<Vec<_>>(),
        ["AFR1234", "RYR33EK", "FGEKO"]
    );

    let atc = client.atc().await.expect("atc");
    assert_eq!(&*atc[0].station, "LFLL_APP");
    assert_eq!(&*atc[0].frequency, "120.500");
}

#[tokio::test]
async fn seltfc_reports_the_current_selection() {
    let mut script = Script::new("LFLL_TWR").with_traffics(fleet());
    script.selected = Some("RYR33EK".to_owned());
    let (_server, client) = connect(script).await;

    assert_eq!(
        client.selected().await.expect("seltfc").as_deref(),
        Some("RYR33EK")
    );
}

#[tokio::test]
async fn a_traffic_without_a_flight_plan_answers_with_empty_fields() {
    let script =
        Script::new("LFLL_TWR").with_traffics(vec![Traffic::new("FGXYZ").no_flight_plan()]);
    let (_server, client) = connect(script).await;

    let plan = client.flight_plan("FGXYZ").await.expect("fp");
    assert_eq!(&*plan.callsign, "FGXYZ");
    assert!(plan.dep.is_empty());
    assert!(plan.arr.is_empty());
}

#[tokio::test]
async fn per_command_fifo_matches_each_reply_to_its_own_request() {
    let script = Script::new("LFLL_TWR")
        .with_traffics(fleet())
        .with_quirk(Quirk::DelayReplies(Duration::from_millis(60)));
    let (_server, client) = connect(script).await;
    wait_connected(&client).await;

    let first = tokio::spawn({
        let client = client.clone();
        async move { client.flight_plan("AFR1234").await }
    });
    tokio::time::sleep(Duration::from_millis(10)).await;
    let second = tokio::spawn({
        let client = client.clone();
        async move { client.flight_plan("RYR33EK").await }
    });

    let first = first.await.expect("task").expect("fp");
    let second = second.await.expect("task").expect("fp");
    assert_eq!(&*first.callsign, "AFR1234");
    assert_eq!(&*second.callsign, "RYR33EK");
}

#[tokio::test]
async fn a_refusal_fails_only_the_command_it_names() {
    let script = Script::new("LFLL_TWR")
        .with_traffics(fleet())
        .with_quirk(Quirk::SilentOn("TRPOS".to_owned()))
        .with_quirk(Quirk::ErrOn("FP".to_owned()));
    let (_server, client) = connect(script).await;
    wait_connected(&client).await;

    let mut stranded = tokio::spawn({
        let client = client.clone();
        async move { client.traffic_position("AFR1234").await }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;

    let refused = client.flight_plan("AFR1234").await;
    assert!(matches!(refused, Err(AuroraError::Refused { .. })));

    let still_waiting = tokio::time::timeout(Duration::from_millis(50), &mut stranded).await;
    assert!(
        still_waiting.is_err(),
        "the @ERR for #FP must not resolve the pending #TRPOS"
    );

    let timed_out = stranded.await.expect("task");
    assert!(matches!(timed_out, Err(AuroraError::Timeout(_))));
}

#[tokio::test]
async fn an_unanswered_command_times_out_and_leaves_the_client_usable() {
    let script = Script::new("LFLL_TWR")
        .with_traffics(fleet())
        .with_quirk(Quirk::SilentOn("TR".to_owned()));
    let (_server, client) = connect(script).await;

    assert!(matches!(
        client.traffic_list().await,
        Err(AuroraError::Timeout(_))
    ));
    assert_eq!(&*client.conn().await.expect("conn"), "LFLL_TWR");
}

#[tokio::test]
async fn an_unknown_callsign_comes_back_as_a_refusal() {
    let (_server, client) = connect(Script::new("LFLL_TWR").with_traffics(fleet())).await;

    assert!(matches!(
        client.flight_plan("ZZZZZ").await,
        Err(AuroraError::Refused { .. })
    ));
}

#[tokio::test]
async fn a_closed_socket_fails_pending_work_then_the_client_reconnects() {
    let script = Script::new("LFLL_TWR")
        .with_traffics(fleet())
        .with_quirk(Quirk::CloseOn("TR".to_owned()));
    let (_server, client) = connect(script).await;
    wait_connected(&client).await;

    assert!(matches!(
        client.traffic_list().await,
        Err(AuroraError::Disconnected)
    ));

    let mut station = None;
    for _ in 0..50 {
        if let Ok(value) = client.conn().await {
            station = Some(value);
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(station.as_deref(), Some("LFLL_TWR"));
}

#[tokio::test]
async fn replies_split_across_two_writes_are_reassembled() {
    let script = Script::new("LFLL_TWR")
        .with_traffics(fleet())
        .with_quirk(Quirk::SplitWrites);
    let (_server, client) = connect(script).await;

    let plan = client.flight_plan("AFR1234").await.expect("fp");
    assert_eq!(&*plan.remarks, "PBN/A1B1");
}

#[tokio::test]
async fn replies_terminated_with_a_bare_newline_still_parse() {
    let script = Script::new("LFLL_TWR")
        .with_traffics(fleet())
        .with_quirk(Quirk::BareNewlines);
    let (_server, client) = connect(script).await;

    let plan = client.flight_plan("AFR1234").await.expect("fp");
    assert_eq!(&*plan.callsign, "AFR1234");
}

#[tokio::test]
async fn an_oversized_line_is_discarded_without_losing_the_next_reply() {
    let script = Script::new("LFLL_TWR")
        .with_traffics(fleet())
        .with_quirk(Quirk::OversizedLine);
    let (_server, client) = connect(script).await;

    let plan = client.flight_plan("AFR1234").await.expect("fp");
    assert_eq!(&*plan.callsign, "AFR1234");
}

#[tokio::test]
async fn the_connection_state_channel_reports_connected() {
    let (_server, client) = connect(Script::new("LFLL_TWR")).await;
    wait_connected(&client).await;
    assert_eq!(client.state(), ConnectionState::Connected);
}

#[tokio::test]
async fn requests_are_rejected_while_aurora_is_unreachable() {
    let (client, _join) = AuroraClient::spawn(config(SocketAddr::from(([127, 0, 0, 1], 1))));

    for _ in 0..20 {
        if matches!(client.conn().await, Err(AuroraError::Disconnected)) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("expected the client to reject requests while offline");
}
