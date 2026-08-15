use thiserror::Error;

use super::board::Column;
use super::classifier::AutoColumn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveReason {
    /// Airborne and outside the ring.
    Departed,
    /// `#TRPOS` field 17 reported a stand.
    Parked,
    /// Stopped on the ground long enough, used when field 17 stays empty (5.4).
    ParkedInferred,
    /// Gone from `#TR` for longer than the dropout grace period.
    RadarDropout,
}

/// Stored strip lifecycle. `Offboard` covers traffic that is tracked but not displayed —
/// the majority at a busy event — which the four board columns have no state for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StripState {
    Offboard,
    Awake,
    ActivatedDeparture,
    Arrival,
    Transit,
    Archived(ArchiveReason),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransitionError {
    #[error("only an awake departure can be activated, this strip is {0:?}")]
    NotAwake(StripState),
    #[error("an archived strip cannot change state")]
    AlreadyArchived,
}

impl StripState {
    /// The only way into `ActivatedDeparture`; a transit or arrival has no arm here by construction.
    pub fn activate(self) -> Result<Self, TransitionError> {
        match self {
            Self::Awake => Ok(Self::ActivatedDeparture),
            other => Err(TransitionError::NotAwake(other)),
        }
    }

    pub fn archive(self, reason: ArchiveReason) -> Result<Self, TransitionError> {
        match self {
            Self::Archived(_) => Err(TransitionError::AlreadyArchived),
            _ => Ok(Self::Archived(reason)),
        }
    }

    /// Follows the classifier, except that activation and archival both survive reclassification.
    pub fn observe(self, column: Option<AutoColumn>) -> Self {
        match self {
            Self::Archived(_) | Self::ActivatedDeparture => self,
            _ => match column {
                None => Self::Offboard,
                Some(AutoColumn::Awake) => Self::Awake,
                Some(AutoColumn::Arrival) => Self::Arrival,
                Some(AutoColumn::Transit) => Self::Transit,
            },
        }
    }

    pub fn column(self) -> Option<Column> {
        match self {
            Self::Awake => Some(Column::Awake),
            Self::ActivatedDeparture => Some(Column::ActivatedDeparture),
            Self::Arrival => Some(Column::Arrival),
            Self::Transit => Some(Column::Transit),
            Self::Offboard | Self::Archived(_) => None,
        }
    }

    pub fn is_archived(self) -> bool {
        matches!(self, Self::Archived(_))
    }

    pub fn is_departure(self) -> bool {
        matches!(self, Self::Awake | Self::ActivatedDeparture)
    }
}
