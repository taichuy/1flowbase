use super::ClientTrajectoryFrameKind;
use serde_json::Value;
// This best-effort diagnostic budget is independent of the larger Responses ingress limit.
// SSE/WS values remain individually bounded; whole sessions are not accumulated.
pub(super) const AGGREGATE_BYTES: usize = 2 * 1024 * 1024;
/// Byte-based framing preserves partial UTF-8 until a complete JSON/SSE value exists.
#[derive(Default)]
pub(super) struct Decoder {
    request: Vec<u8>,
    response: Vec<u8>,
    line: Vec<u8>,
    data: Vec<u8>,
    last_cr: bool,
    first_line_seen: bool,
    discarding: bool,
    pub incomplete: bool,
}
impl Decoder {
    pub fn feed(&mut self, kind: ClientTrajectoryFrameKind, bytes: &[u8]) -> Vec<Value> {
        if kind == ClientTrajectoryFrameKind::ResponseSse {
            return self.sse(bytes);
        }
        let buffer = if kind == ClientTrajectoryFrameKind::Request {
            &mut self.request
        } else {
            &mut self.response
        };
        if buffer.len().saturating_add(bytes.len()) > AGGREGATE_BYTES {
            buffer.clear();
            self.incomplete = true;
            return vec![];
        }
        buffer.extend_from_slice(bytes);
        let mut values = Vec::new();
        let mut stream = serde_json::Deserializer::from_slice(buffer).into_iter::<Value>();
        let mut consumed = 0;
        while let Some(value) = stream.next() {
            match value {
                Ok(value) => {
                    consumed = stream.byte_offset();
                    values.push(value);
                    if values.len() >= 256 {
                        self.incomplete = true;
                        break;
                    }
                }
                Err(error) if error.is_eof() => break,
                Err(_) => {
                    self.incomplete = true;
                    consumed = buffer.len();
                    break;
                }
            }
        }
        buffer.drain(..consumed);
        values
    }
    fn sse(&mut self, bytes: &[u8]) -> Vec<Value> {
        let mut values = Vec::new();
        for &byte in bytes {
            if byte == b'\n' && self.last_cr {
                self.last_cr = false;
                continue;
            }
            self.last_cr = byte == b'\r';
            if byte == b'\r' || byte == b'\n' {
                if !self.first_line_seen {
                    self.first_line_seen = true;
                    if self.line.starts_with(b"\xef\xbb\xbf") {
                        self.line.drain(..3);
                    }
                }
                if self.line.is_empty() {
                    if !self.discarding && !self.data.is_empty() {
                        if self.data.last() == Some(&b'\n') {
                            self.data.pop();
                        }
                        if self.data != b"[DONE]" {
                            match serde_json::from_slice(&self.data) {
                                Ok(value) => values.push(value),
                                Err(_) => self.incomplete = true,
                            }
                        }
                    }
                    self.data.clear();
                    self.discarding = false;
                } else if !self.discarding && self.line.starts_with(b"data:") {
                    let mut value = &self.line[5..];
                    if value.first() == Some(&b' ') {
                        value = &value[1..];
                    }
                    if self.data.len() + value.len() + 1 > AGGREGATE_BYTES {
                        self.incomplete = true;
                        self.discarding = true;
                        self.data.clear();
                    } else {
                        self.data.extend_from_slice(value);
                        self.data.push(b'\n');
                    }
                }
                self.line.clear();
            } else if self.line.len() < AGGREGATE_BYTES {
                self.line.push(byte);
            } else {
                self.incomplete = true;
                self.discarding = true;
            }
            if values.len() >= 256 {
                self.incomplete = true;
                break;
            }
        }
        values
    }
    pub fn finish(&mut self) {
        if self
            .request
            .iter()
            .chain(&self.response)
            .any(|b| !b.is_ascii_whitespace())
            || !self.line.is_empty()
            || !self.data.is_empty()
            || self.discarding
        {
            self.incomplete = true;
        }
    }
}
