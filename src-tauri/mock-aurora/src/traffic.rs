/// One synthetic aircraft. Field order below is the second, independent transcription of the
/// tables in section 4.3 / 4.4 — a mismatch with the parser shows up as a failing test.
#[derive(Debug, Clone)]
pub struct Traffic {
    pub callsign: String,
    pub has_fp: bool,
    pub dep: String,
    pub arr: String,
    pub alternate: String,
    pub eobt: String,
    pub aircraft: String,
    pub wake: String,
    pub rules: String,
    pub flight_type: String,
    pub equipment: String,
    pub cruise_level: String,
    pub cruise_speed: String,
    pub endurance: String,
    pub eet: String,
    pub route: String,
    pub remarks: String,
    pub heading: u16,
    pub track: u16,
    pub altitude: i32,
    pub ground_speed: u16,
    pub lat: f64,
    pub lon: f64,
    pub squawk_set: String,
    pub squawk_label: String,
    pub wp_label: String,
    pub alt_label: String,
    pub spd_label: String,
    pub assumed_by: String,
    pub next_station: String,
    pub on_ground: bool,
    pub is_selected: bool,
    pub was_selected: bool,
    pub gate: String,
    pub voice: String,
    pub vertical_speed: i32,
    pub assigned_gate: String,
}

impl Traffic {
    pub fn new(callsign: &str) -> Self {
        Self {
            callsign: callsign.to_owned(),
            has_fp: true,
            dep: "LFLL".into(),
            arr: "LFPG".into(),
            alternate: "LFPO".into(),
            eobt: "1200".into(),
            aircraft: "A320".into(),
            wake: "M".into(),
            rules: "I".into(),
            flight_type: "S".into(),
            equipment: "SDE3FGHIRWY/LB1".into(),
            cruise_level: "F330".into(),
            cruise_speed: "N0450".into(),
            endurance: "0230".into(),
            eet: "0055".into(),
            route: "BEBIX UM976 MOROK".into(),
            remarks: "PBN/A1B1".into(),
            heading: 90,
            track: 90,
            altitude: 5000,
            ground_speed: 250,
            lat: 45.725556,
            lon: 5.081111,
            squawk_set: "7000".into(),
            squawk_label: "7000".into(),
            wp_label: "BODRU8A 04R".into(),
            alt_label: "F330".into(),
            spd_label: String::new(),
            assumed_by: String::new(),
            next_station: String::new(),
            on_ground: false,
            is_selected: false,
            was_selected: false,
            gate: String::new(),
            voice: "V".into(),
            vertical_speed: 0,
            assigned_gate: String::new(),
        }
    }

    pub fn no_flight_plan(mut self) -> Self {
        self.has_fp = false;
        self
    }

    pub fn route(mut self, dep: &str, arr: &str) -> Self {
        self.dep = dep.to_owned();
        self.arr = arr.to_owned();
        self
    }

    pub fn eobt(mut self, eobt: &str) -> Self {
        self.eobt = eobt.to_owned();
        self
    }

    pub fn rules(mut self, rules: &str) -> Self {
        self.rules = rules.to_owned();
        self
    }

    pub fn at(mut self, lat: f64, lon: f64, altitude: i32) -> Self {
        self.lat = lat;
        self.lon = lon;
        self.altitude = altitude;
        self
    }

    pub fn on_ground(mut self, on_ground: bool) -> Self {
        self.on_ground = on_ground;
        self
    }

    pub fn gate(mut self, gate: &str) -> Self {
        self.gate = gate.to_owned();
        self
    }

    pub fn flight_plan_line(&self) -> String {
        if !self.has_fp {
            return format!("#FP;{};;;;;;;;;;;;;;;", self.callsign);
        }
        let fields: [&str; 16] = [
            &self.callsign,
            &self.dep,
            &self.arr,
            &self.alternate,
            &self.eobt,
            &self.aircraft,
            &self.wake,
            &self.rules,
            &self.flight_type,
            &self.equipment,
            &self.cruise_level,
            &self.cruise_speed,
            &self.endurance,
            &self.eet,
            &self.route,
            &self.remarks,
        ];
        format!("#FP;{}", fields.join(";"))
    }

    pub fn position_line(&self) -> String {
        let fields: [String; 22] = [
            self.callsign.clone(),
            self.heading.to_string(),
            self.track.to_string(),
            self.altitude.to_string(),
            self.ground_speed.to_string(),
            format!("{:.6}", self.lat),
            format!("{:.6}", self.lon),
            self.squawk_set.clone(),
            self.squawk_label.clone(),
            self.wp_label.clone(),
            self.alt_label.clone(),
            self.spd_label.clone(),
            self.assumed_by.clone(),
            self.next_station.clone(),
            flag(self.on_ground),
            flag(self.is_selected),
            flag(self.was_selected),
            self.gate.clone(),
            self.voice.clone(),
            String::new(),
            self.vertical_speed.to_string(),
            self.assigned_gate.clone(),
        ];
        format!("#TRPOS;{}", fields.join(";"))
    }
}

fn flag(value: bool) -> String {
    if value { "1" } else { "0" }.to_owned()
}
