use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Column {
    Awake,
    ActivatedDeparture,
    Arrival,
    Transit,
}

impl Column {
    pub const ALL: [Self; 4] = [
        Self::Awake,
        Self::ActivatedDeparture,
        Self::Arrival,
        Self::Transit,
    ];
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Columns {
    pub awake: Vec<Box<str>>,
    pub activated_departures: Vec<Box<str>>,
    pub arrivals: Vec<Box<str>>,
    pub transits: Vec<Box<str>>,
}

impl Columns {
    pub fn get(&self, column: Column) -> &[Box<str>] {
        match column {
            Column::Awake => &self.awake,
            Column::ActivatedDeparture => &self.activated_departures,
            Column::Arrival => &self.arrivals,
            Column::Transit => &self.transits,
        }
    }

    pub fn set(&mut self, column: Column, callsigns: Vec<Box<str>>) {
        match column {
            Column::Awake => self.awake = callsigns,
            Column::ActivatedDeparture => self.activated_departures = callsigns,
            Column::Arrival => self.arrivals = callsigns,
            Column::Transit => self.transits = callsigns,
        }
    }

    pub fn callsigns(&self) -> impl Iterator<Item = &str> {
        Column::ALL
            .into_iter()
            .flat_map(|column| self.get(column).iter().map(|c| &**c))
    }
}

/// Exactly what one strip renders; everything else the modal fetches on demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripView {
    pub callsign: Box<str>,
    pub adep: Box<str>,
    pub ades: Box<str>,
    pub rules: Box<str>,
}

/// What the board looks like right now. `views` holds only callsigns present in a column.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Board {
    pub columns: Columns,
    pub views: BTreeMap<Box<str>, StripView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardUpdate {
    pub seq: u64,
    pub columns: Option<Columns>,
    pub upserted: Vec<StripView>,
    pub removed: Vec<Box<str>>,
}

impl Board {
    /// Minimal delta, or `None` when nothing the front end can see has changed.
    pub fn diff_from(&self, previous: &Self, seq: u64) -> Option<BoardUpdate> {
        let upserted: Vec<StripView> = self
            .views
            .iter()
            .filter(|(callsign, view)| previous.views.get(*callsign) != Some(view))
            .map(|(_, view)| view.clone())
            .collect();

        let removed: Vec<Box<str>> = previous
            .views
            .keys()
            .filter(|callsign| !self.views.contains_key(*callsign))
            .cloned()
            .collect();

        let columns = (self.columns != previous.columns).then(|| self.columns.clone());

        if columns.is_none() && upserted.is_empty() && removed.is_empty() {
            return None;
        }
        Some(BoardUpdate {
            seq,
            columns,
            upserted,
            removed,
        })
    }
}
