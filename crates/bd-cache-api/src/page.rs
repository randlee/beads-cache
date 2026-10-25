//! Orders and keyset paging (REQ-BDC-0808, contract §5.3).
//!
//! `issues list` serves two orders, both total because the id breaks every tie:
//!
//! - [`ListSort::Created`] (the default, as `/v0`): `(created_at desc, id asc)`;
//! - [`ListSort::Priority`]: `(priority asc, created_at desc, id asc)`, which
//!   is also `ready`'s order.
//!
//! `created_at` is compared as an instant (seconds, then the fraction), never
//! as text. A page after a [`Cursor`] holds the rows strictly after the
//! cursor's key, so a walk with `next_cursor` visits every row once.
//!
//! The cursor is opaque to clients (its encoding is not covered by semver):
//! currently `v1.` followed by base64url, without padding, of the JSON array
//! `[sort, key…, check]`, where `check` is a checksum of the rest.

use std::cmp::Ordering;

use bd_cache_model::RowView;
use sc_beads::{Bead, rfc3339_to_unix};
use serde_json::{Value, json};

use crate::error::ApiError;

/// The cursor's version prefix.
const CURSOR_PREFIX: &str = "v1.";

/// The order of `issues list` (contract §5.3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ListSort {
    /// `(created_at desc, id asc)`: `/v0`'s default.
    #[default]
    Created,
    /// `(priority asc, created_at desc, id asc)`.
    Priority,
}

impl ListSort {
    /// The wire name: `created` or `priority`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Priority => "priority",
        }
    }

    /// Parses a wire name.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "created" => Some(Self::Created),
            "priority" => Some(Self::Priority),
            _ => None,
        }
    }
}

/// A row's sort key: priority, `created_at` as `(seconds, nanoseconds)`, id.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Key {
    priority: i64,
    created: (i64, u32),
    id: String,
}

impl Key {
    fn of(b: &Bead) -> Self {
        Self {
            priority: b.priority,
            created: instant(&b.created_at),
            id: b.id.as_str().to_owned(),
        }
    }
}

/// An RFC 3339 instant as `(unix seconds, nanoseconds)`; an unparseable
/// text sorts as the oldest instant.
fn instant(text: &str) -> (i64, u32) {
    let Some(secs) = rfc3339_to_unix(text) else {
        return (i64::MIN, 0);
    };
    // The fraction follows the seconds of the time part: `HH:MM:SS.fff…`.
    let nanos = text
        .split_once('T')
        .and_then(|(_, time)| time.split_once('.'))
        .map_or(0, |(_, frac)| {
            let digits: String = frac
                .chars()
                .take_while(char::is_ascii_digit)
                .take(9)
                .collect();
            format!("{digits:0<9}").parse::<u32>().unwrap_or(0)
        });
    (secs, nanos)
}

/// The order of two keys under `sort`.
fn order(sort: ListSort, a: &Key, b: &Key) -> Ordering {
    let created_desc = b.created.cmp(&a.created);
    let head = match sort {
        ListSort::Created => created_desc,
        ListSort::Priority => a.priority.cmp(&b.priority).then(created_desc),
    };
    // The id makes the order total, so the keyset never skips or repeats a row.
    head.then_with(|| a.id.cmp(&b.id))
}

/// Sorts rows into `sort`'s order.
pub(crate) fn sort_rows(sort: ListSort, rows: &mut [RowView]) {
    rows.sort_by(|a, b| order(sort, &Key::of(&a.bead), &Key::of(&b.bead)));
}

/// A decoded `issues list` cursor: the sort it was minted for and the key
/// of the last row of its page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    sort: ListSort,
    key: Key,
}

impl Cursor {
    /// The sort the cursor was minted for.
    #[must_use]
    pub fn sort(&self) -> ListSort {
        self.sort
    }

    /// The opaque text (`v1.` + base64url of `[sort, key…]`).
    #[must_use]
    pub fn encode(&self) -> String {
        let k = &self.key;
        let body = match self.sort {
            ListSort::Created => json!(["created", k.created.0, k.created.1, k.id]),
            ListSort::Priority => {
                json!(["priority", k.priority, k.created.0, k.created.1, k.id])
            }
        };
        let mut parts = match body {
            Value::Array(parts) => parts,
            other => vec![other],
        };
        parts.push(Value::String(check(&parts)));
        let text = Value::Array(parts).to_string();
        format!("{CURSOR_PREFIX}{}", b64url_encode(text.as_bytes()))
    }

    /// Decodes a cursor for a request in `sort`.
    ///
    /// # Errors
    ///
    /// `BAD_ARGS` with `v0_code: invalid_cursor` for text this crate did not
    /// mint, or a cursor minted for the other sort.
    pub fn decode(text: &str, sort: ListSort) -> Result<Self, ApiError> {
        let bad = |why: &str| ApiError::invalid_cursor(format!("invalid cursor: {why}"));
        let body = text
            .strip_prefix(CURSOR_PREFIX)
            .ok_or_else(|| bad("unknown version"))?;
        let bytes = b64url_decode(body).ok_or_else(|| bad("not base64url"))?;
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|e| bad(&format!("not JSON ({e})")))?;
        let mut parts = value.as_array().ok_or_else(|| bad("not an array"))?.clone();
        let sum = parts.pop().ok_or_else(|| bad("empty"))?;
        if sum.as_str() != Some(check(&parts).as_str()) {
            return Err(bad("checksum mismatch"));
        }
        let int = |v: Option<&Value>| v.and_then(Value::as_i64).ok_or_else(|| bad("bad key"));
        let nanos = |v: Option<&Value>| {
            v.and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .filter(|n| *n < 1_000_000_000)
                .ok_or_else(|| bad("bad key"))
        };
        let id = |v: Option<&Value>| {
            v.and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| bad("bad key"))
        };
        let minted = parts
            .first()
            .and_then(Value::as_str)
            .and_then(ListSort::parse)
            .ok_or_else(|| bad("unknown sort"))?;
        if minted != sort {
            return Err(bad(&format!(
                "minted for sort={}, not sort={}",
                minted.as_str(),
                sort.as_str()
            )));
        }
        let key = match (sort, parts.len()) {
            (ListSort::Created, 4) => Key {
                priority: 0,
                created: (int(parts.get(1))?, nanos(parts.get(2))?),
                id: id(parts.get(3))?,
            },
            (ListSort::Priority, 5) => Key {
                priority: int(parts.get(1))?,
                created: (int(parts.get(2))?, nanos(parts.get(3))?),
                id: id(parts.get(4))?,
            },
            _ => return Err(bad("wrong length")),
        };
        Ok(Self { sort, key })
    }
}

/// One page of sorted rows.
#[derive(Debug)]
pub(crate) struct Page {
    pub(crate) items: Vec<RowView>,
    pub(crate) has_more: bool,
    pub(crate) next_cursor: Option<String>,
}

/// The page of `sorted` (already in `sort`'s order) after `cursor`, of at
/// most `limit` rows; `None` or 0 is no limit (contract §5.3).
pub(crate) fn page(
    sort: ListSort,
    sorted: Vec<RowView>,
    limit: Option<u64>,
    cursor: Option<&Cursor>,
) -> Page {
    let mut rest: Vec<RowView> = match cursor {
        Some(c) => sorted
            .into_iter()
            .filter(|r| order(sort, &Key::of(&r.bead), &c.key) == Ordering::Greater)
            .collect(),
        None => sorted,
    };
    let limit = limit
        .filter(|n| *n > 0)
        .map(|n| usize::try_from(n).unwrap_or(usize::MAX));
    let Some(limit) = limit.filter(|n| rest.len() > *n) else {
        return Page {
            items: rest,
            has_more: false,
            next_cursor: None,
        };
    };
    rest.truncate(limit);
    let next_cursor = rest.last().map(|r| {
        Cursor {
            sort,
            key: Key::of(&r.bead),
        }
        .encode()
    });
    Page {
        items: rest,
        has_more: true,
        next_cursor,
    }
}

/// The cursor's checksum: FNV-1a (64 bit) of the key's JSON, in hex, so a
/// cursor edited by hand is refused rather than read as another position.
fn check(parts: &[Value]) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    let text = Value::Array(parts.to_vec()).to_string();
    let hash = text
        .bytes()
        .fold(OFFSET, |h, b| (h ^ u64::from(b)).wrapping_mul(PRIME));
    format!("{hash:016x}")
}

const B64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// base64url without padding (RFC 4648 §5).
fn b64url_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk.first().copied().unwrap_or(0),
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let chars = chunk.len() + 1;
        for i in 0..chars {
            let idx = (n >> (18 - 6 * i)) & 0x3f;
            if let Some(c) = B64URL.get(idx as usize) {
                out.push(char::from(*c));
            }
        }
    }
    out
}

/// Decodes [`b64url_encode`]'s output; `None` for anything else.
fn b64url_decode(text: &str) -> Option<Vec<u8>> {
    let sextet = |c: u8| {
        B64URL
            .iter()
            .position(|x| *x == c)
            .and_then(|p| u32::try_from(p).ok())
    };
    let bytes = text.as_bytes();
    if bytes.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= sextet(*c)? << (18 - 6 * i);
        }
        let len = chunk.len() - 1;
        for i in 0..len {
            out.push(u8::try_from((n >> (16 - 8 * i)) & 0xff).ok()?);
        }
    }
    // Reject non-canonical text: it must re-encode to itself.
    (b64url_encode(&out) == text).then_some(out)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "test code: a failed unwrap is a test failure"
    )]

    use super::*;

    #[test]
    fn base64url_round_trips_every_length() {
        for len in 0..20u8 {
            let bytes: Vec<u8> = (0..len).map(|i| i.wrapping_mul(37)).collect();
            let text = b64url_encode(&bytes);
            assert!(!text.contains('='));
            assert_eq!(b64url_decode(&text), Some(bytes));
        }
        assert_eq!(b64url_decode("A"), None);
        assert_eq!(b64url_decode("A*=="), None);
    }

    #[test]
    fn instants_compare_by_value_with_fractions() {
        assert_eq!(
            instant("2026-10-23T10:00:00+02:00"),
            instant("2026-10-23T08:00:00Z")
        );
        assert!(instant("2026-10-23T08:00:00.5Z") > instant("2026-10-23T08:00:00.25Z"));
        assert_eq!(instant("yesterday"), (i64::MIN, 0));
    }

    #[test]
    fn a_cursor_round_trips_and_is_bound_to_its_sort() {
        let c = Cursor {
            sort: ListSort::Priority,
            key: Key {
                priority: 2,
                created: (1_761_213_600, 5),
                id: "b-1".into(),
            },
        };
        let text = c.encode();
        assert!(text.starts_with("v1."));
        assert_eq!(Cursor::decode(&text, ListSort::Priority), Ok(c));
        let wrong = Cursor::decode(&text, ListSort::Created).unwrap_err();
        assert_eq!(wrong.details()["v0_code"], "invalid_cursor");
    }
}
