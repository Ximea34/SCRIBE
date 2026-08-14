use scribe_lib::aurora::codec::{CodecError, LineFramer};

fn feed(framer: &mut LineFramer, chunk: &[u8]) -> Vec<Result<String, CodecError>> {
    framer.push(chunk);
    let mut out = Vec::new();
    framer.drain(|line| out.push(line.map(str::to_owned)));
    out
}

fn lines(framer: &mut LineFramer, chunk: &[u8]) -> Vec<String> {
    feed(framer, chunk)
        .into_iter()
        .filter_map(Result::ok)
        .collect()
}

#[test]
fn frames_a_crlf_line() {
    let mut framer = LineFramer::default();
    assert_eq!(
        lines(&mut framer, b"#CONN;LFLL_TWR\r\n"),
        ["#CONN;LFLL_TWR"]
    );
}

#[test]
fn accepts_a_bare_newline() {
    let mut framer = LineFramer::default();
    assert_eq!(lines(&mut framer, b"#CONN;LFLL_TWR\n"), ["#CONN;LFLL_TWR"]);
}

#[test]
fn splits_coalesced_reads() {
    let mut framer = LineFramer::default();
    assert_eq!(
        lines(&mut framer, b"#A\r\n#B\r\n#C\r\n"),
        ["#A", "#B", "#C"]
    );
}

#[test]
fn reassembles_a_line_arriving_byte_by_byte() {
    let mut framer = LineFramer::default();
    let mut seen = Vec::new();
    for byte in b"#FP;AFR1234\r\n" {
        seen.extend(lines(&mut framer, &[*byte]));
    }
    assert_eq!(seen, ["#FP;AFR1234"]);
}

#[test]
fn tolerates_a_split_between_cr_and_lf() {
    let mut framer = LineFramer::default();
    assert!(lines(&mut framer, b"#TR;AFR1234\r").is_empty());
    assert_eq!(
        lines(&mut framer, b"\n#TR;RYR33EK\r\n"),
        ["#TR;AFR1234", "#TR;RYR33EK"]
    );
}

#[test]
fn holds_an_unterminated_line_until_it_completes() {
    let mut framer = LineFramer::default();
    assert!(lines(&mut framer, b"#CONN;LF").is_empty());
    assert_eq!(lines(&mut framer, b"LL_TWR\r\n"), ["#CONN;LFLL_TWR"]);
}

#[test]
fn skips_empty_lines() {
    let mut framer = LineFramer::default();
    assert_eq!(lines(&mut framer, b"\r\n\n#A\r\n\r\n"), ["#A"]);
}

#[test]
fn rejects_an_oversized_terminated_line_and_keeps_going() {
    let mut framer = LineFramer::with_max_line(16);
    let mut chunk = vec![b'#'; 64];
    chunk.extend_from_slice(b"\r\n#A\r\n");

    let out = feed(&mut framer, &chunk);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], Err(CodecError::LineTooLong(16)));
    assert_eq!(out[1].as_deref(), Ok("#A"));
}

#[test]
fn discards_an_oversized_unterminated_line_without_growing() {
    let mut framer = LineFramer::with_max_line(16);

    let out = feed(&mut framer, &[b'X'; 64]);
    assert_eq!(out, [Err(CodecError::LineTooLong(16))]);

    let out = feed(&mut framer, &[b'X'; 64]);
    assert!(out.is_empty());

    assert_eq!(lines(&mut framer, b"tail\r\n#A\r\n"), ["#A"]);
}

#[test]
fn reports_invalid_utf8_and_recovers() {
    let mut framer = LineFramer::default();
    let out = feed(&mut framer, b"\xff\xfe\r\n#A\r\n");
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], Err(CodecError::NotUtf8));
    assert_eq!(out[1].as_deref(), Ok("#A"));
}
