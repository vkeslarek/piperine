import os

path = "crates/piperine-codegen/src/device/mod.rs"
with open(path, "r") as f:
    text = f.read()

target = """    fn digital_seq_phase(&mut self, t: f64, nets: &[LogicValue], analog_voltages: ndarray::ArrayView1<f64>) -> bool {
        let Some(digital) = &mut self.digital else { return false };
        let av = Self::analog_voltages_for(
            digital.kernel().layout(),
            self.analog.as_ref(),
            &self.analog_terminal_node_ids,
            &self.last_analog_voltages,
            analog_voltages.as_slice().unwrap(),
        );
        digital.eval_seq_phase(t, nets, &av)
    }
}"""

replacement = """    fn digital_seq_phase(&mut self, t: f64, nets: &[LogicValue], analog_voltages: ndarray::ArrayView1<f64>) -> bool {
        let Some(digital) = &mut self.digital else { return false };
        let av = Self::analog_voltages_for(
            digital.kernel().layout(),
            self.analog.as_ref(),
            &self.analog_terminal_node_ids,
            &self.last_analog_voltages,
            analog_voltages.as_slice().unwrap(),
        );
        digital.eval_seq_phase(t, nets, &av)
    }

    fn digital_comb_phase(
        &mut self,
        t: f64,
        nets: &[LogicValue],
        analog_voltages: ndarray::ArrayView1<f64>,
        event_queue: &mut std::collections::BinaryHeap<std::cmp::Reverse<piperine_solver::digital::DigitalEvent>>,
    ) {
        let Some(digital) = &mut self.digital else { return };
        let av = Self::analog_voltages_for(
            digital.kernel().layout(),
            self.analog.as_ref(),
            &self.analog_terminal_node_ids,
            &self.last_analog_voltages,
            analog_voltages.as_slice().unwrap(),
        );
        digital.eval_comb_phase(t, nets, &av, event_queue);

        if let Some(analog) = &mut self.analog {
            let vars = digital.export_vars();
            analog.sync_vars(&vars);
        }
    }
}"""

if target in text:
    text = text.replace(target, replacement)
    with open(path, "w") as f:
        f.write(text)
    print("SUCCESS")
else:
    print("TARGET NOT FOUND")

