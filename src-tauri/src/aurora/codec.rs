use thiserror::Error;

/// Longest line accepted; anything longer is reported once and discarded up to the next terminator.
pub const MAX_LINE_LEN: usize = 8 * 1024;

const INITIAL_CAPACITY: usize = 4096;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CodecError {
    #[error("line exceeds the {0} byte limit and was discarded")]
    LineTooLong(usize),
    #[error("line is not valid UTF-8 and was discarded")]
    NotUtf8,
}

/// Reassembles `\r\n` (or bare `\n`) delimited lines from arbitrarily chunked reads.
#[derive(Debug)]
pub struct LineFramer {
    buf: Vec<u8>,
    max_line: usize,
    discarding: bool,
}

impl Default for LineFramer {
    fn default() -> Self {
        Self::with_max_line(MAX_LINE_LEN)
    }
}

impl LineFramer {
    pub fn with_max_line(max_line: usize) -> Self {
        Self {
            buf: Vec::with_capacity(INITIAL_CAPACITY),
            max_line,
            discarding: false,
        }
    }

    pub fn push(&mut self, chunk: &[u8]) {
        self.buf.extend_from_slice(chunk);
    }

    /// Yields every complete line currently buffered; empty lines are skipped silently.
    pub fn drain(&mut self, mut on_line: impl FnMut(Result<&str, CodecError>)) {
        let Self {
            buf,
            max_line,
            discarding,
        } = self;
        let mut consumed = 0usize;

        while let Some(offset) = buf[consumed..].iter().position(|&b| b == b'\n') {
            let end = consumed + offset;
            let raw = strip_trailing_cr(&buf[consumed..end]);
            consumed = end + 1;

            if *discarding {
                *discarding = false;
                continue;
            }
            if raw.is_empty() {
                continue;
            }
            if raw.len() > *max_line {
                on_line(Err(CodecError::LineTooLong(*max_line)));
                continue;
            }
            match std::str::from_utf8(raw) {
                Ok(line) => on_line(Ok(line)),
                Err(_) => on_line(Err(CodecError::NotUtf8)),
            }
        }

        if *discarding {
            buf.clear();
            return;
        }
        if buf.len() - consumed > *max_line {
            *discarding = true;
            buf.clear();
            on_line(Err(CodecError::LineTooLong(*max_line)));
            return;
        }
        if consumed == buf.len() {
            buf.clear();
        } else if consumed > 0 {
            buf.drain(..consumed);
        }
    }
}

fn strip_trailing_cr(raw: &[u8]) -> &[u8] {
    match raw {
        [head @ .., b'\r'] => head,
        _ => raw,
    }
}
