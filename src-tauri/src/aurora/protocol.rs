use thiserror::Error;

pub const FP_FIELDS: usize = 16;
pub const TRPOS_FIELDS: usize = 22;
pub const MAX_CALLSIGN_LEN: usize = 16;

/// Leading character of a line; it marks the response kind, never the command's meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prefix {
    Hash,
    At,
    Dollar,
}

impl Prefix {
    fn from_char(c: char) -> Option<Self> {
        match c {
            '#' => Some(Self::Hash),
            '@' => Some(Self::At),
            '$' => Some(Self::Dollar),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandName {
    Conn,
    SelTfc,
    FlightPlan,
    TrafficPosition,
    TrafficList,
    Atc,
}

impl CommandName {
    pub const ALL: [Self; 6] = [
        Self::Conn,
        Self::SelTfc,
        Self::FlightPlan,
        Self::TrafficPosition,
        Self::TrafficList,
        Self::Atc,
    ];
    pub const COUNT: usize = Self::ALL.len();

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Conn => "CONN",
            Self::SelTfc => "SELTFC",
            Self::FlightPlan => "FP",
            Self::TrafficPosition => "TRPOS",
            Self::TrafficList => "TR",
            Self::Atc => "ATC",
        }
    }

    /// Resolves a wire token, tolerating the `#`/`@`/`$` prefix that `@ERR` echoes back.
    pub fn from_wire(token: &str) -> Option<Self> {
        let name = token.strip_prefix(['#', '@', '$']).unwrap_or(token);
        Self::ALL.into_iter().find(|c| c.as_str() == name)
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InvalidCallsign {
    #[error("callsign is empty")]
    Empty,
    #[error("callsign is longer than {MAX_CALLSIGN_LEN} characters")]
    TooLong,
    #[error("callsign contains the reserved character {0:?}")]
    ReservedChar(char),
}

/// Rejects `%` so `%SELTFC%` can never reach Aurora — sending it closes the socket (4.6.1).
pub fn validate_callsign(callsign: &str) -> Result<(), InvalidCallsign> {
    if callsign.is_empty() {
        return Err(InvalidCallsign::Empty);
    }
    if callsign.chars().count() > MAX_CALLSIGN_LEN {
        return Err(InvalidCallsign::TooLong);
    }
    match callsign
        .chars()
        .find(|c| matches!(c, ';' | '%') || c.is_control())
    {
        Some(c) => Err(InvalidCallsign::ReservedChar(c)),
        None => Ok(()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Conn,
    SelTfc,
    FlightPlan(Box<str>),
    TrafficPosition(Box<str>),
    TrafficList,
    Atc,
}

impl Command {
    pub fn flight_plan(callsign: &str) -> Result<Self, InvalidCallsign> {
        validate_callsign(callsign)?;
        Ok(Self::FlightPlan(callsign.into()))
    }

    pub fn traffic_position(callsign: &str) -> Result<Self, InvalidCallsign> {
        validate_callsign(callsign)?;
        Ok(Self::TrafficPosition(callsign.into()))
    }

    pub fn name(&self) -> CommandName {
        match self {
            Self::Conn => CommandName::Conn,
            Self::SelTfc => CommandName::SelTfc,
            Self::FlightPlan(_) => CommandName::FlightPlan,
            Self::TrafficPosition(_) => CommandName::TrafficPosition,
            Self::TrafficList => CommandName::TrafficList,
            Self::Atc => CommandName::Atc,
        }
    }

    pub fn argument(&self) -> Option<&str> {
        match self {
            Self::FlightPlan(cs) | Self::TrafficPosition(cs) => Some(cs),
            _ => None,
        }
    }

    /// Appends the wire form to `out`, which the client reuses to avoid a per-request allocation.
    pub fn write_into(&self, out: &mut String) {
        out.push('#');
        out.push_str(self.name().as_str());
        if let Some(argument) = self.argument() {
            out.push(';');
            out.push_str(argument);
        }
        out.push_str("\r\n");
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("empty line")]
    Empty,
    #[error("line does not start with '#', '@' or '$'")]
    MissingPrefix,
    #[error("line carries no command name")]
    MissingCommand,
    #[error("{command}: response carries no callsign")]
    MissingCallsign { command: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refusal<'a> {
    pub command: &'a str,
    pub argument: &'a str,
    pub reason: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlightPlanRef<'a>([&'a str; FP_FIELDS]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrafficPositionRef<'a>([&'a str; TRPOS_FIELDS]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrafficListRef<'a>(&'a str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtcListRef<'a>(&'a str);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response<'a> {
    Conn {
        station: &'a str,
    },
    SelTfc {
        callsign: &'a str,
    },
    FlightPlan(FlightPlanRef<'a>),
    TrafficPosition(TrafficPositionRef<'a>),
    TrafficList(TrafficListRef<'a>),
    Atc(AtcListRef<'a>),
    Refusal(Refusal<'a>),
    Unknown {
        prefix: Prefix,
        command: &'a str,
        body: &'a str,
    },
}

macro_rules! text_fields {
    ($ty:ident, $cmd:literal, $($idx:literal => $name:ident),+ $(,)?) => {
        impl<'a> $ty<'a> {
            $(
                #[doc = concat!("`", $cmd, "` field ", stringify!($idx), ".")]
                #[inline]
                pub fn $name(&self) -> &'a str { self.0[$idx] }
            )+
        }
    };
}

text_fields!(FlightPlanRef, "#FP",
    0 => callsign,
    1 => dep,
    2 => arr,
    3 => alternate,
    4 => eobt,
    5 => aircraft,
    6 => wake,
    7 => rules,
    8 => flight_type,
    9 => equipment,
    10 => cruise_level,
    11 => cruise_speed,
    12 => endurance,
    13 => eet,
    14 => route,
    15 => remarks,
);

text_fields!(TrafficPositionRef, "#TRPOS",
    0 => callsign,
    7 => squawk_set,
    8 => squawk_label,
    9 => wp_label,
    10 => alt_label,
    11 => spd_label,
    12 => assumed_by,
    13 => next_station,
    17 => gate,
    18 => voice,
    21 => assigned_gate,
);

impl<'a> TrafficPositionRef<'a> {
    /// `#TRPOS` field 1.
    pub fn heading(&self) -> Option<u16> {
        num_u16(self.0[1])
    }
    /// `#TRPOS` field 2.
    pub fn track(&self) -> Option<u16> {
        num_u16(self.0[2])
    }
    /// `#TRPOS` field 3, feet.
    pub fn altitude(&self) -> Option<i32> {
        num_i32(self.0[3])
    }
    /// `#TRPOS` field 4, knots over the ground.
    pub fn ground_speed(&self) -> Option<u16> {
        num_u16(self.0[4])
    }
    /// `#TRPOS` field 5.
    pub fn lat(&self) -> Option<f64> {
        num_f64(self.0[5])
    }
    /// `#TRPOS` field 6.
    pub fn lon(&self) -> Option<f64> {
        num_f64(self.0[6])
    }
    /// `#TRPOS` field 14.
    pub fn on_ground(&self) -> bool {
        flag(self.0[14])
    }
    /// `#TRPOS` field 15.
    pub fn is_selected(&self) -> bool {
        flag(self.0[15])
    }
    /// `#TRPOS` field 16.
    pub fn was_selected(&self) -> bool {
        flag(self.0[16])
    }
    /// `#TRPOS` field 20, feet per minute; undocumented but confirmed on real data.
    pub fn vertical_speed(&self) -> Option<i32> {
        num_i32(self.0[20])
    }
}

impl<'a> TrafficListRef<'a> {
    pub fn iter(&self) -> impl Iterator<Item = &'a str> {
        self.0.split(';').map(str::trim).filter(|s| !s.is_empty())
    }
}

impl<'a> AtcListRef<'a> {
    /// Yields `(station, frequency)`; entries without a `:` are skipped.
    pub fn iter(&self) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.0
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter_map(|entry| entry.split_once(':'))
    }
}

/// Maps one raw line to a typed response. Pure: no I/O, no allocation outside error paths.
pub fn parse(line: &str) -> Result<Response<'_>, ParseError> {
    let line = line.trim_end_matches(['\r', '\n']);
    let prefix = match line.chars().next() {
        Some(c) => Prefix::from_char(c).ok_or(ParseError::MissingPrefix)?,
        None => return Err(ParseError::Empty),
    };
    let body = &line[1..];
    let (command, rest) = body.split_once(';').unwrap_or((body, ""));
    if command.is_empty() {
        return Err(ParseError::MissingCommand);
    }

    let response = match CommandName::from_wire(command) {
        Some(CommandName::Conn) => Response::Conn {
            station: first_field(rest),
        },
        Some(CommandName::SelTfc) => Response::SelTfc {
            callsign: first_field(rest),
        },
        Some(CommandName::FlightPlan) => {
            let f = fields::<FP_FIELDS>(rest);
            require_callsign(f[0], command)?;
            Response::FlightPlan(FlightPlanRef(f))
        }
        Some(CommandName::TrafficPosition) => {
            let f = fields::<TRPOS_FIELDS>(rest);
            require_callsign(f[0], command)?;
            Response::TrafficPosition(TrafficPositionRef(f))
        }
        Some(CommandName::TrafficList) => Response::TrafficList(TrafficListRef(rest)),
        Some(CommandName::Atc) => Response::Atc(AtcListRef(rest)),
        None if command == "ERR" => {
            let mut parts = rest.splitn(3, ';');
            Response::Refusal(Refusal {
                command: parts.next().unwrap_or_default(),
                argument: parts.next().unwrap_or_default(),
                reason: parts.next().unwrap_or_default(),
            })
        }
        None => Response::Unknown {
            prefix,
            command,
            body: rest,
        },
    };
    Ok(response)
}

fn require_callsign(callsign: &str, command: &str) -> Result<(), ParseError> {
    if callsign.is_empty() {
        return Err(ParseError::MissingCallsign {
            command: command.to_owned(),
        });
    }
    Ok(())
}

fn first_field(rest: &str) -> &str {
    rest.split(';').next().unwrap_or_default()
}

/// Extra fields beyond `N` are ignored; missing trailing fields read as empty.
fn fields<const N: usize>(rest: &str) -> [&str; N] {
    let mut out = [""; N];
    for (slot, value) in out.iter_mut().zip(rest.split(';')) {
        *slot = value;
    }
    out
}

fn num_f64(raw: &str) -> Option<f64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    raw.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn num_i32(raw: &str) -> Option<i32> {
    num_f64(raw).map(|v| v.round() as i32)
}

fn num_u16(raw: &str) -> Option<u16> {
    num_f64(raw).map(|v| v.round().clamp(0.0, u16::MAX as f64) as u16)
}

fn flag(raw: &str) -> bool {
    raw.trim() == "1"
}
