#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    Voltage(Option<(String, String)>),
    Current(Option<String>),
    Power,

    Conductance,
    Transconductance,
    Capacitance(Option<String>),

    Temperature,
    IsLinear,

    ModelParam(String),
}
