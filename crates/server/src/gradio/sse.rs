//! An incremental parser for `text/event-stream` bodies (the WHATWG event stream format), fed with chunks split at
//! arbitrary byte positions: inside a line, inside a CRLF pair or inside a multi-byte UTF-8 character.

/// One dispatched event. `event` defaults to `message` when the stream gives no `event:` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("event stream line longer than {MAX_LINE_BYTES} bytes")]
pub struct LineTooLong;

/// Longest line kept in memory; a render response with word timings for a full batch is well under 1 MB.
pub const MAX_LINE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct SseParser {
    buf: Vec<u8>,
    event: String,
    data: String,
    has_data: bool,
    started: bool,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a chunk; returns every event completed by it.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<SseEvent>, LineTooLong> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        let mut start = 0;
        let mut i = 0;
        while i < self.buf.len() {
            match self.buf[i] {
                b'\n' => {
                    self.line(start, i, &mut out);
                    i += 1;
                    start = i;
                }
                b'\r' => {
                    // A CR at the end of the buffer may be the first half of a CRLF split across chunks.
                    if i + 1 == self.buf.len() {
                        break;
                    }
                    self.line(start, i, &mut out);
                    i += if self.buf[i + 1] == b'\n' { 2 } else { 1 };
                    start = i;
                }
                _ => i += 1,
            }
        }
        self.buf.drain(..start);
        if self.buf.len() > MAX_LINE_BYTES {
            return Err(LineTooLong);
        }
        Ok(out)
    }

    /// End of stream: a final line without a newline still counts, and a pending event is dispatched even though
    /// the blank line that should end it never came (Gradio closes the stream right after `complete`).
    pub fn finish(&mut self) -> Option<SseEvent> {
        let mut out = Vec::new();
        if !self.buf.is_empty() {
            let end = if self.buf.last() == Some(&b'\r') { self.buf.len() - 1 } else { self.buf.len() };
            self.line(0, end, &mut out);
            self.buf.clear();
        }
        self.dispatch(&mut out);
        out.into_iter().next()
    }

    fn line(&mut self, start: usize, end: usize, out: &mut Vec<SseEvent>) {
        let mut bytes = &self.buf[start..end];
        if !self.started {
            self.started = true;
            bytes = bytes.strip_prefix("\u{feff}".as_bytes()).unwrap_or(bytes);
        }
        if bytes.is_empty() {
            self.dispatch(out);
            return;
        }
        let line = String::from_utf8_lossy(bytes);
        if line.starts_with(':') {
            return;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line.as_ref(), ""),
        };
        match field {
            "event" => self.event = value.to_owned(),
            "data" => {
                if self.has_data {
                    self.data.push('\n');
                }
                self.data.push_str(value);
                self.has_data = true;
            }
            _ => {}
        }
    }

    fn dispatch(&mut self, out: &mut Vec<SseEvent>) {
        let event = std::mem::take(&mut self.event);
        if self.has_data {
            let event = if event.is_empty() { "message".to_owned() } else { event };
            out.push(SseEvent { event, data: std::mem::take(&mut self.data) });
        }
        self.has_data = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A recorded Gradio 6 stream: heartbeat, progress, then the result, with CRLF and a split UTF-8 char.
    const RECORDED: &str = "event: heartbeat\r\ndata: null\r\n\r\n\
        event: generating\ndata: [\"partial \u{2014} text\"]\n\n\
        : keep-alive comment\n\
        event: complete\ndata: [\"{\\\"v\\\":1,\\\"x\\\":\\\"caf\u{e9}\\\"}\"]\n\n";

    fn parse_in_chunks(bytes: &[u8], cuts: &[usize]) -> Vec<SseEvent> {
        let mut p = SseParser::new();
        let mut out = Vec::new();
        let mut last = 0;
        for &cut in cuts.iter().chain(std::iter::once(&bytes.len())) {
            out.extend(p.push(&bytes[last..cut]).unwrap());
            last = cut;
        }
        out.extend(p.finish());
        out
    }

    fn expected() -> Vec<SseEvent> {
        vec![
            SseEvent { event: "heartbeat".into(), data: "null".into() },
            SseEvent { event: "generating".into(), data: "[\"partial \u{2014} text\"]".into() },
            SseEvent { event: "complete".into(), data: "[\"{\\\"v\\\":1,\\\"x\\\":\\\"caf\u{e9}\\\"}\"]".into() },
        ]
    }

    #[test]
    fn whole_stream_at_once() {
        assert_eq!(parse_in_chunks(RECORDED.as_bytes(), &[]), expected());
    }

    #[test]
    fn every_single_split_point_gives_the_same_events() {
        let bytes = RECORDED.as_bytes();
        for cut in 0..=bytes.len() {
            assert_eq!(parse_in_chunks(bytes, &[cut]), expected(), "split at {cut}");
        }
    }

    #[test]
    fn byte_by_byte() {
        let bytes = RECORDED.as_bytes();
        let cuts: Vec<usize> = (1..bytes.len()).collect();
        assert_eq!(parse_in_chunks(bytes, &cuts), expected());
    }

    #[test]
    fn pairs_of_split_points() {
        let bytes = RECORDED.as_bytes();
        for a in (0..bytes.len()).step_by(3) {
            for b in (a..bytes.len()).step_by(7) {
                assert_eq!(parse_in_chunks(bytes, &[a, b]), expected(), "split at {a},{b}");
            }
        }
    }

    #[test]
    fn multi_line_data_bare_cr_and_default_event() {
        let events = parse_in_chunks(b"data: a\rdata:b\r\rid: 3\nretry: 10\nevent\ndata\n\n", &[]);
        assert_eq!(
            events,
            vec![
                SseEvent { event: "message".into(), data: "a\nb".into() },
                SseEvent { event: "message".into(), data: String::new() },
            ]
        );
    }

    #[test]
    fn stream_closed_without_trailing_blank_line_still_dispatches() {
        assert_eq!(
            parse_in_chunks(b"event: error\ndata: \"boom\"", &[5]),
            vec![SseEvent { event: "error".into(), data: "\"boom\"".into() }]
        );
        assert_eq!(parse_in_chunks(b"event: lonely\n\n", &[]), vec![]);
    }

    #[test]
    fn leading_bom_is_ignored() {
        let events = parse_in_chunks("\u{feff}event: x\ndata: 1\n\n".as_bytes(), &[1]);
        assert_eq!(events, vec![SseEvent { event: "x".into(), data: "1".into() }]);
    }

    #[test]
    fn overlong_lines_are_refused() {
        let mut p = SseParser::new();
        assert!(p.push(&vec![b'a'; MAX_LINE_BYTES + 1]).is_err());
    }
}
