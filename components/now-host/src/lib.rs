//! roc:temporal-now — the clock and the machine's configured time zone.
//!
//! Deliberately vendors NO temporal_rs (H0c): the instant is
//! `std::time::SystemTime` and the zone is `iana-time-zone`, which is exactly
//! what temporal_rs's own `sys-local` feature reaches for. Keeping those two
//! crates here rather than in `temporal-host` is what lets a world grant a
//! clock separately from granting calendar arithmetic.
use core::mem::ManuallyDrop;
use trantor_abi as abi;
use abi::*;
use std::time::{SystemTime, UNIX_EPOCH};

const NANOS_PER_SECOND: i128 = 1_000_000_000;

/// A clock that reads before 1970 is a broken machine, not a date, so it is
/// reported rather than wrapped into a plausible negative instant.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__now_host__epoch_ns() -> NowHostEpochNsResult {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => NowHostEpochNsResult {
            payload: NowHostEpochNsResultPayload {
                ok: ManuallyDrop::new(d.as_secs() as i128 * NANOS_PER_SECOND + d.subsec_nanos() as i128),
            },
            tag: NowHostEpochNsResultTag::Ok,
        },
        Err(_) => NowHostEpochNsResult {
            payload: NowHostEpochNsResultPayload { err: [] },
            tag: NowHostEpochNsResultTag::Err,
        },
    }
}

/// The IANA identifier, not a guess. A host that cannot say says so — silently
/// answering UTC would make every later calculation quietly wrong.
#[unsafe(no_mangle)]
pub extern "C-unwind" fn trantor__now_host__system_time_zone_id() -> NowHostSystemTimeZoneIdResult {
    match iana_time_zone::get_timezone() {
        Ok(id) => NowHostSystemTimeZoneIdResult {
            payload: NowHostSystemTimeZoneIdResultPayload {
                ok: ManuallyDrop::new(RocStr::from_str(&id, abi::host())),
            },
            tag: NowHostSystemTimeZoneIdResultTag::Ok,
        },
        Err(_) => NowHostSystemTimeZoneIdResult {
            payload: NowHostSystemTimeZoneIdResultPayload { err: [] },
            tag: NowHostSystemTimeZoneIdResultTag::Err,
        },
    }
}
