use crate::model::{
    Circuit, Component, Exp, ModelCard, ModelType, Node, SourceFunction, Unit, Value,
};
use crate::ngspice::NgSpiceEngine;
use std::{thread, time};

mod model;
mod ngspice;

#[macro_export]
macro_rules! map {
    // Match: key => value, (repeating)
    // The optional comma at the end ensures [A => B, ] also works.
    ( $( $key:expr => $val:expr ),* $(,)? ) => {{
        let mut _map = std::collections::HashMap::new();
        $(
            _map.insert($key, $val);
        )*
        _map
    }};
    () => {{
        std::collections::HashMap::new()
    }};
}

fn main() {
    let engine = NgSpiceEngine::instance();
    let rx = engine.receiver();

    thread::spawn(move || {
        while let Ok(message) = rx.recv() {
            println!("LOG: {:?}", message);
        }
    });

    // engine.add_sync_data(Arc::new(LogSyncData));
    engine.send_command("source dcop.cir").unwrap();
    engine.send_command("tran 10m 2s").unwrap();

    thread::sleep(time::Duration::from_secs(5));

    engine.sync_quit().unwrap();
}

fn create_model() -> Circuit {
    Circuit {
        title: "bipolar amplifier".into(),
        models: vec![ModelCard {
            name: "BC546B".into(),
            model_type: ModelType::NPN,
            parameters: map![
            "IS".into() => model::ParameterValue::Numeric(7.59E-15),
            "VAF".into() => model::ParameterValue::Numeric(73.4),
            "BF".into() => model::ParameterValue::Numeric(480.0),
            "IKF".into() => model::ParameterValue::Numeric(0.0962),
            "NE".into() => model::ParameterValue::Numeric(1.2665),
            "ISE".into() => model::ParameterValue::Numeric(3.278E-15),
            "IKR".into() => model::ParameterValue::Numeric(0.03),
            "ISC".into() => model::ParameterValue::Numeric(2.00E-13),
            "NC".into() => model::ParameterValue::Numeric(1.2),
            "NR".into() => model::ParameterValue::Numeric(1.0),
            "BR".into() => model::ParameterValue::Numeric(5.0),
            "RC".into() => model::ParameterValue::Numeric(0.25),
            "CJC".into() => model::ParameterValue::Numeric(6.33E-12),
            "FC".into() => model::ParameterValue::Numeric(0.5),
            "MJC".into() => model::ParameterValue::Numeric(0.33),
            "VJC".into() => model::ParameterValue::Numeric(0.65),
            "CJE".into() => model::ParameterValue::Numeric(1.25E-11),
            "MJE".into() => model::ParameterValue::Numeric(0.55),
            "VJE".into() => model::ParameterValue::Numeric(0.65),
            "TF".into() => model::ParameterValue::Numeric(4.26E-10),
            "ITF".into() => model::ParameterValue::Numeric(0.6),
            "VTF".into() => model::ParameterValue::Numeric(3.0),
            "XTF".into() => model::ParameterValue::Numeric(20.0),
            "RB".into() => model::ParameterValue::Numeric(100.0),
            "IRB".into() => model::ParameterValue::Numeric(0.0001),
            "RBM".into() => model::ParameterValue::Numeric(10.0),
            "RE".into() => model::ParameterValue::Numeric(0.5),
            "TR".into() => model::ParameterValue::Numeric(1.50E-07)
            ],
        }],
        components: map![
            "R3".into() => Component::Resistor {n1: Node::Named("vcc".into()), n2: Node::Named("intc".into()), value: Value(10.0, Some(Exp::Kilo), Some(Unit::Ohm)), model: None},
            "R1".into() => Component::Resistor {n1: Node::Named("vcc".into()), n2: Node::Named("intb".into()), value: Value(68.0, Some(Exp::Kilo), Some(Unit::Ohm)), model: None},
            "R2".into() => Component::Resistor {n1: Node::Named("intb".into()), n2: Node::Ground, value: Value(10.0, Some(Exp::Kilo), Some(Unit::Ohm)), model: None},
            "Cout".into() => Component::Capacitor {n1: Node::Named("out".into()), n2: Node::Named("intc".into()), value: Value(10.0, Some(Exp::Micro), Some(Unit::Farad)), ic: None},
            "Cin".into() => Component::Capacitor {n1: Node::Named("intb".into()), n2: Node::Named("in".into()), value: Value(10.0, Some(Exp::Micro), Some(Unit::Farad)), ic: None},
            "VCC".into() => Component::VoltageSource {n1: Node::Named("vcc".into()), n2: Node::Ground, functions: vec![SourceFunction::External {buffer_index: 0}]},
            "Vin".into() => Component::VoltageSource {n1: Node::Named("in".into()), n2: Node::Ground, functions: vec![SourceFunction::DC(0.0), SourceFunction::AC {offset: 0.0, amplitude: Value(1.0, Some(Exp::Mili), None), frequency: 500.0}]},
            "RLoad".into() => Component::Resistor {n1: Node::Named("out".into()), n2: Node::Ground, value: Value(100.0, Some(Exp::Kilo), Some(Unit::Ohm)), model: None},
            "Q1".into() => Component::BJT {collector: Node::Named("intc".into()), base: Node::Named("intb".into()), emitter: Node::Ground, substrate: None, model: "BC546B".into(), area: None},
        ],
        ..Default::default()
    }
}
