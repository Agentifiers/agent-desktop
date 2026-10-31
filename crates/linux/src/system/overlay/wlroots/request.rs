//! What a command sends a session's overlay helper: one JSON line, answered
//! with one acknowledgement byte once the compositor has applied it.

use std::io::{BufRead, BufReader, Read};

use agent_desktop_core::{AdapterError, CursorOverlayControl, ErrorCode};
use serde_json::{Value, json};

pub(crate) const ACKNOWLEDGED: u8 = 1;
const MAX_REQUEST_BYTES: u64 = 8 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Request {
    /// A control the session's markers follow.
    Control(Box<CursorOverlayControl>),
    /// Keep every marker off the screen until the requester closes its
    /// connection, so a capture never shows one.
    Conceal,
}

fn malformed(detail: impl std::fmt::Display) -> AdapterError {
    AdapterError::internal("The cursor overlay helper received a malformed request")
        .with_platform_detail(detail.to_string())
}

impl Request {
    pub(crate) fn encode(&self) -> Result<Vec<u8>, AdapterError> {
        let value = match self {
            Self::Control(control) => json!({ "request": "control", "control": control }),
            Self::Conceal => json!({ "request": "conceal" }),
        };
        let mut line = serde_json::to_vec(&value).map_err(malformed)?;
        if line.len() as u64 >= MAX_REQUEST_BYTES {
            return Err(AdapterError::new(
                ErrorCode::InvalidArgs,
                "Cursor overlay instruction exceeds the transport limit",
            ));
        }
        line.push(b'\n');
        Ok(line)
    }

    /// Reads one request line; stops at the transport limit.
    pub(crate) fn read(reader: impl Read) -> Result<Self, AdapterError> {
        let mut line = Vec::new();
        BufReader::new(reader.take(MAX_REQUEST_BYTES))
            .read_until(b'\n', &mut line)
            .map_err(malformed)?;
        if line.pop() != Some(b'\n') {
            return Err(malformed("the request line is unterminated"));
        }
        Self::decode(&line)
    }

    fn decode(line: &[u8]) -> Result<Self, AdapterError> {
        let mut value: Value = serde_json::from_slice(line).map_err(malformed)?;
        let Some(object) = value.as_object_mut() else {
            return Err(malformed("the request is not an object"));
        };
        let kind = object.remove("request");
        match (kind.as_ref().and_then(Value::as_str), object.len()) {
            (Some("conceal"), 0) => Ok(Self::Conceal),
            (Some("control"), 1) => {
                let control = object
                    .remove("control")
                    .ok_or_else(|| malformed("the control is missing"))?;
                let control: CursorOverlayControl =
                    serde_json::from_value(control).map_err(malformed)?;
                control.validate()?;
                Ok(Self::Control(Box::new(control)))
            }
            _ => Err(malformed("unknown request")),
        }
    }
}

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
