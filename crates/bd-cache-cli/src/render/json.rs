//! The sc-command envelope on stdout (contract §1 rule 1, §4).
//!
//! One envelope, one line, a trailing newline, for every reply: success,
//! typed error and `USAGE` alike. An [`ApiError`] becomes the envelope's
//! `error` member field by field: kind, code, message, `details` (always an
//! object) and `suggested_action`.

use bd_cache_api::{ApiError, ErrorKind as ApiKind};
use sc_command::{Envelope, ErrorKind, OpError};
use sc_observability_types::ErrorCode;
use serde_json::Value;

/// The envelope's `error` for an [`ApiError`].
#[must_use]
pub(crate) fn op_error(e: &ApiError) -> OpError {
    let kind = match e.kind() {
        ApiKind::InvalidInput => ErrorKind::InvalidInput,
        ApiKind::NotFound => ErrorKind::NotFound,
        ApiKind::Unavailable => ErrorKind::Unavailable,
        ApiKind::Conflict => ErrorKind::Conflict,
        ApiKind::Internal => ErrorKind::Internal,
    };
    let error = OpError::new(kind, ErrorCode::new_owned(e.code()), e.message())
        .with_details(Value::Object(e.details().clone()));
    match e.suggested_action() {
        Some(action) => error.with_suggestion(action),
        None => error,
    }
}

/// The envelope of a result, as one line of JSON and a newline.
#[must_use]
pub(crate) fn envelope(result: &Result<Value, ApiError>) -> String {
    let envelope = match result {
        Ok(data) => Envelope::success(data.clone()),
        Err(e) => Envelope::failure(op_error(e)),
    };
    line(&envelope)
}

/// The envelope as one line of JSON and a newline.
fn line(envelope: &Envelope<Value>) -> String {
    // An envelope of plain JSON values always serializes; the fallback is a valid
    // failure envelope should that ever stop being true.
    let mut line = serde_json::to_string(envelope).unwrap_or_else(|_| {
        r#"{"version":"1","ok":false,"data":null,"error":{"kind":"internal","code":"BD_CACHE.INTERNAL","message":"the reply could not be serialized","details":{},"suggested_action":null}}"#.to_owned()
    });
    line.push('\n');
    line
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, reason = "test code: JSON indexing")]

    use super::*;

    #[test]
    fn an_api_error_keeps_every_field() {
        let e = ApiError::not_cached("b1");
        let text = envelope(&Err(e));
        let v: Value = serde_json::from_str(&text).unwrap_or_default();
        assert_eq!(v["ok"], false);
        assert_eq!(v["data"], Value::Null);
        assert_eq!(v["error"]["kind"], "unavailable");
        assert_eq!(v["error"]["code"], "BD_CACHE.NOT_CACHED");
        assert_eq!(v["error"]["details"]["bead"], "b1");
        assert_eq!(
            v["error"]["suggested_action"],
            "retry with --miss-timeout-ms above 0"
        );
        assert!(text.ends_with('\n') && !text.trim_end().contains('\n'));
    }
}
