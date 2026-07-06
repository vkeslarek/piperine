import os

path = "crates/piperine-codegen/src/device/mod.rs"
with open(path, "r") as f:
    text = f.read()

target = """impl AnalogDevice for PiperineDevice {
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
}"""

replacement = """impl Device for PiperineDevice {
    fn device_name(&self) -> &str {
        &self.label
    }
}

impl AnalogDevice for PiperineDevice {
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
    ) -> Vec<Noise> {
        match &mut self.analog {
            Some(analog) => analog.noise_current_psd(dc_point, ac_context),
            None => Vec::new(),
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

