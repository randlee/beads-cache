//! `cache tick`: bd-cache-sync's tick, with each refusal as an `OpError` (contract §6).
//!
//! The tick has no `bd-cache-api` model (its request and reply belong to
//! `bd-cache-sync`), so its two members are decoded here with the decoder's
//! rules: an unknown member is `BAD_ARGS` with `details.member`.

use std::time::Duration;

use bd_cache_api::ApiError;
use bd_cache_api::data::OpError;
use bd_cache_sync::{AtmSender, Composed, TICK_BUDGET, TickReply};
use sc_beads::{Clock, Deadline};
use serde_json::{Map, Value};

use crate::args::{ArgSpec, CommandSpec, Shape, Target, Ty};

/// `cache tick [--budget-ms N] [--atm PATH]`.
pub(crate) const COMMAND: CommandSpec = CommandSpec {
    path: "cache tick",
    about: "One sync tick: drain, sync the active phases, announce (bd_cache_cron.py's job).",
    target: Target::Tick,
    args: &[
        ArgSpec::flag(
            "budget-ms",
            "N",
            Shape::One,
            Ty::Int,
            false,
            "budget_ms",
            "The tick's budget in ms (default and maximum 25000).",
        ),
        ArgSpec::flag(
            "atm",
            "PATH",
            Shape::One,
            Ty::Str,
            false,
            "atm",
            "The atm binary that delivers announcements (default: atm on PATH).",
        ),
    ],
    one_of: None,
};

/// A decoded `cache tick` request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TickRequest {
    budget: Duration,
    atm: Option<String>,
}

/// Decodes the `cache tick` request.
///
/// # Errors
///
/// `BAD_ARGS` for a request that is not an object, an unknown member, a
/// budget that is not an integer in `1..=25000`, or an `atm` that is not a
/// nonempty string.
pub(crate) fn decode(request: &Value) -> Result<TickRequest, ApiError> {
    let empty = Map::new();
    let obj = match request {
        Value::Object(m) => m,
        Value::Null => &empty,
        _ => return Err(ApiError::bad_args("the request must be a JSON object")),
    };
    if let Some(k) = obj
        .keys()
        .find(|k| !matches!(k.as_str(), "budget_ms" | "atm"))
    {
        return Err(ApiError::unknown_member(k));
    }
    let max = u64::try_from(TICK_BUDGET.as_millis()).unwrap_or(u64::MAX);
    let budget = match obj.get("budget_ms").filter(|v| !v.is_null()) {
        None => TICK_BUDGET,
        Some(v) => match v.as_u64() {
            Some(ms) if (1..=max).contains(&ms) => Duration::from_millis(ms),
            _ => {
                return Err(ApiError::bad_member(
                    "budget_ms",
                    format!("budget_ms must be an integer from 1 to {max}"),
                ));
            }
        },
    };
    let atm = match obj.get("atm").filter(|v| !v.is_null()) {
        None => None,
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(_) => {
            return Err(ApiError::bad_member("atm", "atm must be a nonempty path"));
        }
    };
    Ok(TickRequest { budget, atm })
}

/// Runs one tick on the composed cache and returns its `data`: the
/// `TickReply`, with each `refused[]` item an `OpError` (`ApiError::tick_refusal`).
///
/// # Errors
///
/// The cache's errors, as [`ApiError`].
pub(crate) fn run(
    composed: &mut Composed,
    request: &TickRequest,
    clock: &dyn Clock,
) -> Result<Value, ApiError> {
    let sender = request
        .atm
        .as_ref()
        .map_or_else(AtmSender::default, AtmSender::with_binary);
    let end = Deadline::after(clock, request.budget);
    let reply = bd_cache_sync::tick(&mut composed.cache, &composed.tick, &sender, end)?;
    reply_json(&reply)
}

/// The reply as JSON, with `refused[]` mapped to `OpError`s.
fn reply_json(reply: &TickReply) -> Result<Value, ApiError> {
    let mut value = serde_json::to_value(reply)
        .map_err(|e| ApiError::internal(format!("the tick reply does not serialize: {e}")))?;
    let refused: Vec<OpError> = reply
        .refused
        .iter()
        .map(|r| {
            let e = ApiError::tick_refusal(r.code.as_str(), r.detail.clone());
            OpError {
                code: e.code().to_owned(),
                message: e.message().to_owned(),
            }
        })
        .collect();
    let refused = serde_json::to_value(refused)
        .map_err(|e| ApiError::internal(format!("a refusal does not serialize: {e}")))?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert("refused".to_owned(), refused);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, reason = "test code: JSON indexing")]

    use super::*;
    use bd_cache_sync::{Refusal, RefusalCode};
    use serde_json::json;

    #[test]
    fn the_request_is_checked_like_the_decoder_checks_one() {
        assert_eq!(
            decode(&json!({})).map(|r| r.budget),
            Ok(TICK_BUDGET),
            "the default budget"
        );
        let e = decode(&json!({"budget": 3})).err();
        assert_eq!(e.as_ref().map(ApiError::code), Some("BD_CACHE.BAD_ARGS"));
        assert_eq!(
            e.as_ref().and_then(|e| e.details().get("member").cloned()),
            Some(json!("budget"))
        );
        assert!(decode(&json!({"budget_ms": 0})).is_err());
        assert!(decode(&json!({"budget_ms": 25_001})).is_err());
        assert!(decode(&json!({"atm": ""})).is_err());
    }

    #[test]
    fn a_refusal_is_an_op_error_with_the_tick_code() {
        let reply = TickReply::refused_with(Refusal::new(RefusalCode::MissingTeam, "no team"));
        let v = reply_json(&reply).unwrap_or_default();
        assert_eq!(
            v["refused"],
            json!([{"code": "BD_CACHE.TICK_MISSING_TEAM", "message": "no team"}])
        );
        assert_eq!(v["outcome"], "refused");
    }
}
