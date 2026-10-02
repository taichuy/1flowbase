use super::ClientTrajectoryFrameKind;
use serde_json::Value;
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
        buffer.extend_from_slice(bytes);
        let mut values = Vec::new();
        let mut stream = serde_json::Deserializer::from_slice(buffer).into_iter::<Value>();
        let mut consumed = 0;
        while let Some(value) = stream.next() {
            match value {
                Ok(value) => {
                    consumed = stream.byte_offset();
                    values.push(value);
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
                        if self.data == b"[DONE]" {
                            values.push(serde_json::json!({"__client_sse_done":true}));
                        } else {
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
                    self.data.extend_from_slice(value);
                    self.data.push(b'\n');
                }
                self.line.clear();
            } else {
                self.line.push(byte);
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
