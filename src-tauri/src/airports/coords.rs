use std::fmt;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Latitude,
    Longitude,
}

impl Axis {
    fn limit(self) -> f64 {
        match self {
            Self::Latitude => 90.0,
            Self::Longitude => 180.0,
        }
    }

    fn accepts(self, hemisphere: char) -> bool {
        match self {
            Self::Latitude => matches!(hemisphere, 'N' | 'S'),
            Self::Longitude => matches!(hemisphere, 'E' | 'W'),
        }
    }
}

impl fmt::Display for Axis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Latitude => "latitude",
            Self::Longitude => "longitude",
        })
    }
}

#[derive(Debug, Clone, Error, PartialEq)]
pub enum CoordError {
    #[error("coordinate is empty")]
    Empty,
    #[error("{0:?} is not a recognisable coordinate")]
    Malformed(String),
    #[error("{0:?} has more than three degree/minute/second parts")]
    TooManyParts(String),
    #[error("minutes must be below 60, got {0}")]
    Minutes(f64),
    #[error("seconds must be below 60, got {0}")]
    Seconds(f64),
    #[error("{hemisphere:?} is not a hemisphere for a {axis}")]
    WrongHemisphere { axis: Axis, hemisphere: char },
    #[error("a sign and a hemisphere letter cannot both be given")]
    SignAndHemisphere,
    #[error("{value} is outside the range of a {axis}")]
    OutOfRange { axis: Axis, value: f64 },
}

/// Accepts DMS with or without symbols, hemisphere as prefix or suffix, dot- or space-separated
/// DMS, and signed or hemisphere-suffixed decimal degrees. North and east are positive.
pub fn parse(input: &str, axis: Axis) -> Result<f64, CoordError> {
    let original = input.trim();
    if original.is_empty() {
        return Err(CoordError::Empty);
    }

    let normalised: String = original
        .to_ascii_uppercase()
        .chars()
        .map(|c| if is_separator(c) { ' ' } else { c })
        .collect();

    let (body, hemisphere) = split_hemisphere(normalised.trim());
    if let Some(hemisphere) = hemisphere {
        if !axis.accepts(hemisphere) {
            return Err(CoordError::WrongHemisphere { axis, hemisphere });
        }
    }

    let negative_sign = body.starts_with('-');
    if negative_sign && hemisphere.is_some() {
        return Err(CoordError::SignAndHemisphere);
    }
    let unsigned = body.strip_prefix(['-', '+']).unwrap_or(body).trim();
    if unsigned.is_empty() {
        return Err(CoordError::Malformed(original.to_owned()));
    }

    let mut value = degrees(unsigned, original)?;
    if negative_sign || matches!(hemisphere, Some('S' | 'W')) {
        value = -value;
    }
    if !(-axis.limit()..=axis.limit()).contains(&value) {
        return Err(CoordError::OutOfRange { axis, value });
    }
    Ok(value)
}

fn degrees(unsigned: &str, original: &str) -> Result<f64, CoordError> {
    let parts: Vec<&str> = if unsigned.split_whitespace().nth(1).is_some() {
        unsigned.split_whitespace().collect()
    } else if unsigned.matches('.').count() >= 2 {
        unsigned.split('.').collect()
    } else {
        vec![unsigned]
    };

    if parts.len() > 3 {
        return Err(CoordError::TooManyParts(original.to_owned()));
    }

    let mut value = 0.0;
    for (index, part) in parts.iter().enumerate() {
        let number: f64 = part
            .parse()
            .ok()
            .filter(|n: &f64| n.is_finite() && *n >= 0.0)
            .ok_or_else(|| CoordError::Malformed(original.to_owned()))?;
        match index {
            0 => value += number,
            1 if number >= 60.0 => return Err(CoordError::Minutes(number)),
            1 => value += number / 60.0,
            _ if number >= 60.0 => return Err(CoordError::Seconds(number)),
            _ => value += number / 3600.0,
        }
    }
    Ok(value)
}

fn split_hemisphere(body: &str) -> (&str, Option<char>) {
    if let Some(first) = body.chars().next() {
        if is_hemisphere(first) {
            return (body[first.len_utf8()..].trim(), Some(first));
        }
    }
    if let Some(last) = body.chars().last() {
        if is_hemisphere(last) {
            return (body[..body.len() - last.len_utf8()].trim(), Some(last));
        }
    }
    (body, None)
}

fn is_hemisphere(c: char) -> bool {
    matches!(c, 'N' | 'S' | 'E' | 'W')
}

fn is_separator(c: char) -> bool {
    matches!(
        c,
        '°' | 'º' | '\'' | '’' | '′' | '"' | '“' | '”' | '″' | ':' | '\t'
    )
}
