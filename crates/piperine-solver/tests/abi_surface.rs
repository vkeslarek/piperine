use piperine_solver::abi::*;

struct Resistor {
    r: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}

impl Element for Resistor {
    fn name(&self) -> &str {
        "r1"
    }
    
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }

    fn list_params(&self) -> Vec<ParamDescriptor> {
        vec![ParamDescriptor {
            name: "r".into(),
            kind: ValueKind::Real,
            default: Value::Real(1000.0),
            unit: Some("ohm".into()),
            bounds: Bounds { min: Some(0.0), max: None },
            scope: ParamScope::Instance,
            invalidation: Invalidation::Restamp,
        }]
    }

    fn get_param(&self, name: &str) -> Option<Value> {
        (name == "r").then(|| Value::Real(self.r))
    }

    fn set_param(&mut self, name: &str, value: Value) -> std::result::Result<Invalidation, ParamError> {
        if name != "r" {
            return Err(ParamError::Unknown(name.into()));
        }
        let Some(v) = value.as_real() else {
            return Err(ParamError::TypeMismatch { name: name.into(), expected: ValueKind::Real });
        };
        if v <= 0.0 {
            return Err(ParamError::OutOfRange { name: name.into(), value });
        }
        self.r = v;
        Ok(Invalidation::Restamp)
    }

    fn load_dc(&mut self, _state: &DcAnalysisState, _ctx: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        let g = 1.0 / self.r;
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }
}

#[test]
fn abi_compiles_an_element() {
    let mut netlist = Netlist::new();
    let n1 = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let n2 = netlist.connect_node(GND);

    let r = Resistor { r: 1000.0, n1, n2 };
    assert!(r.capabilities().contains(ElementCapabilities::ANALOG));
    assert_eq!(r.get_param("r"), Some(Value::Real(1000.0)));

    let elements: Vec<Box<dyn Element>> = vec![Box::new(r)];
    
    let mut circuit = CircuitInstance::from_devices_and_netlist("test", elements, netlist);
    let ctx = Context::default();
    
    let mut dc = circuit.dc(ctx).unwrap();
    let res = dc.solve().unwrap();
    assert_eq!(res.get_node(&NodeIdentifier::Anonymous(1)), Some(0.0));
}
