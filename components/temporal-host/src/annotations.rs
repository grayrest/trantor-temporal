//! RFC 9557 `[key=value]` annotations, read by TC39's grammar before
//! temporal_rs sees the string (D-T2-31). ixdtf 0.6.6 checks each character
//! against the one after it, so it refuses a one-character key or value
//! segment (`[f=ab]`, `[foo=ab-c]`) and accepts a value that starts, ends or
//! doubles a hyphen (`[foo=-ab]`, `[foo=a--b]`). Here rather than in lib.rs,
//! which is the ABI, so `tests/sweeps` compiles this same file.
use std::borrow::Cow;
use temporal_rs::TemporalError;

const CALENDAR_KEY: &str = "u-ca";

/// `AnnotationKey`: a lower-case letter or `_`, then those, digits or `-`.
fn is_key(key: &str) -> bool {
    let mut chars = key.bytes();
    let is_leading = |c: u8| c.is_ascii_lowercase() || c == b'_';
    chars.next().is_some_and(is_leading) && chars.all(|c| is_leading(c) || c.is_ascii_digit() || c == b'-')
}

/// `AnnotationValue`: alphanumeric components joined by single hyphens.
fn is_value(value: &str) -> bool {
    value.split('-').all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_alphanumeric()))
}

/// The string with every well-formed, non-critical annotation other than the
/// calendar removed — TC39 ignores those — and a malformed one refused. The
/// calendar and critical annotations stay for temporal_rs to judge. A string
/// whose brackets do not close in sequence, or whose zone annotation follows
/// a `key=value` one, is passed on untouched for ixdtf to refuse.
pub fn normalize(s: &str) -> Result<Cow<'_, str>, TemporalError> {
    let Some(start) = s.find('[') else { return Ok(Cow::Borrowed(s)) };
    let Some(groups) = s[start..].strip_prefix('[').and_then(|t| t.strip_suffix(']')) else {
        return Ok(Cow::Borrowed(s));
    };
    let groups: Vec<&str> = groups.split("][").collect();
    if groups.iter().any(|g| g.contains(['[', ']'])) {
        return Ok(Cow::Borrowed(s));
    }
    let first_keyed = groups.iter().position(|g| g.contains('=')).unwrap_or(groups.len());
    if groups[first_keyed..].iter().any(|g| !g.contains('=')) {
        return Ok(Cow::Borrowed(s));
    }
    let mut kept = s[..start].to_string();
    for (i, group) in groups.iter().enumerate() {
        if i >= first_keyed {
            let (is_critical, body) = group.strip_prefix('!').map_or((false, *group), |b| (true, b));
            let (key, value) = body.split_once('=').unwrap_or((body, ""));
            if !is_key(key) || !is_value(value) {
                return Err(TemporalError::range().with_message("an annotation is [key=value] in lower-case letters, digits and hyphens"));
            }
            if key != CALENDAR_KEY && !is_critical {
                continue;
            }
        }
        kept.push('[');
        kept.push_str(group);
        kept.push(']');
    }
    Ok(Cow::Owned(kept))
}
