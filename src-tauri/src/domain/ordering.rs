use super::board::Column;
use super::flight::Flight;

pub const MINUTES_PER_DAY: i32 = 1440;
/// How far into the past an EOBT still counts as "already due" rather than "tomorrow".
pub const EOBT_LOOKBACK_MINUTES: i32 = 360;

/// `HHMM` UTC, optionally with a separator. Anything else is unusable and sorts last.
pub fn parse_eobt(raw: &str) -> Option<u16> {
    let digits: String = raw
        .chars()
        .filter(|c| !matches!(c, ':' | ' ' | '.'))
        .collect();
    if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hours: u16 = digits.get(..2)?.parse().ok()?;
    let minutes: u16 = digits.get(2..)?.parse().ok()?;
    if hours > 23 || minutes > 59 {
        return None;
    }
    Some(hours * 60 + minutes)
}

/// Signed minutes from now on a circular clock, so 2350 sorts before 0010 just after midnight.
pub fn eobt_sort_key(eobt: Option<u16>, now_minutes: u16) -> i32 {
    let Some(eobt) = eobt else {
        return i32::MAX;
    };
    let delta = (i32::from(eobt) - i32::from(now_minutes)).rem_euclid(MINUTES_PER_DAY);
    if delta > MINUTES_PER_DAY - EOBT_LOOKBACK_MINUTES {
        delta - MINUTES_PER_DAY
    } else {
        delta
    }
}

/// Integer key in thousandths of a nautical mile; unknown distances sort last and never panic.
pub fn distance_key(nautical_miles: Option<f64>) -> i32 {
    match nautical_miles {
        Some(nm) if nm.is_finite() => {
            (nm * 1000.0).round().clamp(0.0, f64::from(i32::MAX - 1)) as i32
        }
        _ => i32::MAX,
    }
}

pub fn altitude_key(altitude: Option<i32>) -> i32 {
    altitude.unwrap_or(i32::MAX)
}

/// Callsign is always the final tie-break, so every ordering is total and stable.
pub fn order(column: Column, flights: &mut [&Flight], now_minutes: u16) {
    match column {
        Column::Awake | Column::ActivatedDeparture => flights.sort_by(|a, b| {
            eobt_sort_key(a.eobt_minutes, now_minutes)
                .cmp(&eobt_sort_key(b.eobt_minutes, now_minutes))
                .then_with(|| a.callsign.cmp(&b.callsign))
        }),
        Column::Arrival => flights.sort_by(|a, b| {
            distance_key(a.distance_nm)
                .cmp(&distance_key(b.distance_nm))
                .then_with(|| altitude_key(a.altitude()).cmp(&altitude_key(b.altitude())))
                .then_with(|| a.callsign.cmp(&b.callsign))
        }),
        Column::Transit => flights.sort_by(|a, b| {
            distance_key(a.distance_nm)
                .cmp(&distance_key(b.distance_nm))
                .then_with(|| a.callsign.cmp(&b.callsign))
        }),
    }
}
