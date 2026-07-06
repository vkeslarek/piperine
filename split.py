import os

path = "crates/piperine-codegen/src/device/mod.rs"
with open(path, "r") as f:
    lines = f.readlines()

new_impl = """impl AnalogDevice for PiperineDevice {
    fn device_name(&self) -> &str {
        &self.label
    }

    fn limiting_active(&self) -> bool {
        self.analog
            .as_ref()
            .is_some_and(AnalogInstance::limiting_active)
    }

    fn bound_step_hint(&self) -> f64 {
        self.analog
            .as_ref()
            .map_or(f64::INFINITY, AnalogInstance::bound_step_hint)
    }

    fn load_dc(
        &mut self,
        state: &DcAnalysisState,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        match &mut self.analog {
            Some(analog) => analog.load_dc(state, context),
            None => Vec::new(),
        }
    }

    fn load_ac(
        &mut self,
        dc_op: &DcAnalysisResult,
        ac_ctx: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex64>> {
        match &mut self.analog {
            Some(analog) => analog.load_ac(dc_op, ac_ctx, context),
            None => Vec::new(),
        }
    }

    fn load_transient(
        &mut self,
        states: &TransientAnalysisState,
        tran_ctx: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        match &mut self.analog {
            Some(analog) => analog.load_transient(states, tran_ctx, context),
            None => Vec::new(),
        }
    }

    fn accept_timestep(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        ctx: &Context,
        nets: &[piperine_solver::digital::LogicValue],
        event_queue: &mut std::collections::BinaryHeap<std::cmp::Reverse<piperine_solver::digital::DigitalEvent>>,
    ) {
        if let Some(analog) = &mut self.analog {
            analog.accept_timestep(state, ctx);
        }
        
        if self.analog.is_none() && !self.analog_terminal_refs.is_empty() {
            let latest = state.latest();
            for (i, opt_ref) in self.analog_terminal_refs.iter().enumerate() {
                self.last_analog_voltages[i] = opt_ref
                    .as_ref()
                    .and_then(|r| r.idx())
                    .and_then(|idx| latest.map(|s| s[idx]))
                    .unwrap_or(0.0);
            }
        }
        
        if self.digital.as_ref().map_or(false, |d| d.kernel().layout().num_analog() > 0) {
            self.eval_discrete(ctx.time, nets, ndarray::ArrayView1::from(&[]), event_queue);
        }
    }

    fn noise_current_psd(
        &mut self,
        dc_point: &DcAnalysisResult,
        ac_context: &AcAnalysisContext,
        noise_point: &Noise,
        context: &Context,
    ) -> f64 {
        match &mut self.analog {
            Some(analog) => analog.noise_current_psd(dc_point, ac_context, noise_point, context),
            None => 0.0,
        }
    }
}

impl DigitalDevice for PiperineDevice {
    fn digital_input_nets(&self) -> &[DigitalNet] {
        self.digital
            .as_ref()
            .map_or(&[], DigitalInstance::input_nets)
    }

    fn digital_output_nets(&self) -> &[DigitalNet] {
        self.digital
            .as_ref()
            .map_or(&[], DigitalInstance::output_nets)
    }

    fn digital_init(&mut self, event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>) {
        if let Some(digital) = &mut self.digital {
            digital.init(event_queue);
        }
    }

    fn eval_discrete(
        &mut self,
        t: f64,
        nets: &[LogicValue],
        analog_voltages: ndarray::ArrayView1<f64>,
        event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        let Some(digital) = &mut self.digital else { return };
        let av = Self::analog_voltages_for(
            digital.kernel().layout(),
            self.analog.as_ref(),
            &self.analog_terminal_node_ids,
            &self.last_analog_voltages,
            analog_voltages.as_slice().unwrap(),
        );
        digital.eval(t, nets, &av, event_queue);

        if let Some(analog) = &mut self.analog {
            let vars = digital.export_vars();
            analog.sync_vars(&vars);
        }
    }

    fn digital_seq_phase(&mut self, t: f64, nets: &[LogicValue], analog_voltages: ndarray::ArrayView1<f64>) -> bool {
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
}
"""

with open(path, "w") as f:
    f.writelines(lines[:139]) # lines 0-138 (1-139 in 1-based)
    f.write(new_impl)
    f.writelines(lines[311:]) # 312 onwards

