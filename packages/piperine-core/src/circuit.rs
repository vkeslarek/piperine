use crate::analysis::transient::TransientAnalysisContext;
use crate::component::{Component, ComponentSpec};
use crate::math::linear::Stamp;
use crate::model::{AnyModel, ModelResolver};
use crate::netlist::{CircuitReference, Netlist};
use crate::solver::Context;
use crate::state::CircuitState;
use std::collections::HashMap;
use std::sync::Arc;

pub struct CircuitSpec {
    title: String,
    models: HashMap<String, Arc<dyn AnyModel>>,
    components: HashMap<String, Box<dyn ComponentSpec>>,
}

impl CircuitSpec {
    pub fn new(title: String) -> Self {
        Self {
            title,
            models: HashMap::new(),
            components: HashMap::new(),
        }
    }

    pub fn insert_get<B: ComponentSpec>(
        &mut self,
        name: &str,
        component: impl ComponentSpec,
    ) -> Option<&mut B> {
        let name_str = name.to_string();

        self.components
            .insert(name_str.clone(), Box::new(component));

        self.components
            .get_mut(&name_str)
            .and_then(|b| b.as_any_mut().downcast_mut::<B>())
    }

    pub fn model(&mut self, name: &str, model: impl AnyModel) {
        self.models.insert(name.to_string(), Arc::new(model));
    }

    pub fn instantiate(&self, model_resolver: &mut ModelResolver) -> crate::error::Result<Circuit> {
        for (name, model) in self.models.iter() {
            model_resolver.insert(name.clone(), model.clone())?;
        }

        let mut netlist = Netlist::new();
        let mut components = HashMap::new();
        for (name, spec) in &self.components {
            components.insert(
                name.clone(),
                spec.instantiate(&mut netlist, model_resolver)?,
            );
        }

        Ok(Circuit::new(self.title.clone(), netlist, components))
    }
}

pub struct Circuit {
    title: String,
    netlist: Netlist,
    components: HashMap<String, Box<dyn Component>>,
}

impl Circuit {
    pub fn new(
        title: String,
        netlist: Netlist,
        components: HashMap<String, Box<dyn Component>>,
    ) -> Self {
        Self {
            title,
            netlist,
            components,
        }
    }

    pub fn netlist(&self) -> &Netlist {
        &self.netlist
    }

    pub fn build(title: &str, build_fn: fn(&mut CircuitSpec)) -> CircuitSpec {
        let mut circuit_spec = CircuitSpec::new(title.to_string());
        (build_fn)(&mut circuit_spec);
        circuit_spec
    }

    pub fn update(&mut self) -> crate::error::Result<()> {
        for (_, component) in &mut self.components {
            component.update()?;
        }

        Ok(())
    }

    pub fn update_dc(&mut self, context: &Context) -> crate::error::Result<()> {
        for (_, component) in &mut self.components {
            let dc_comp = component.as_dc_mut().unwrap();
            dc_comp.update_dc(context)?;
        }

        Ok(())
    }

    pub fn load_dc(&mut self, context: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        let mut stamps = Vec::new();

        for (_, component) in &mut self.components {
            let dc_comp = component.as_dc_mut().unwrap();
            stamps.extend(
                dc_comp
                    .load_dc(context)
                    .into_iter()
                    .filter_map(|stamp| match stamp {
                        Stamp::Matrix(r, c, val) => {
                            // Only keep the stamp if NEITHER the row nor the column is ground
                            if !r.is_ground() && !c.is_ground() {
                                Some(Stamp::Matrix(r, c, val))
                            } else {
                                None
                            }
                        }
                        Stamp::Rhs(r, val) => {
                            if !r.is_ground() {
                                Some(Stamp::Rhs(r, val))
                            } else {
                                None
                            }
                        }
                    }),
            );
        }

        stamps
    }

    pub fn update_transient(
        &mut self,
        circuit_state: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> crate::error::Result<()> {
        for (_, component) in &mut self.components {
            let tran_comp = component.as_transient_mut().unwrap();
            tran_comp.update_transient(circuit_state, transient_analysis_context, context)?;
        }

        Ok(())
    }

    pub fn load_transient(
        &mut self,
        circuit_state: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let mut stamps = Vec::new();

        for (_, component) in &mut self.components {
            let tran_comp = component.as_transient_mut().unwrap();
            stamps.extend(
                tran_comp
                    .load_transient(circuit_state, transient_analysis_context, context)
                    .into_iter()
                    .filter_map(|stamp| match stamp {
                        Stamp::Matrix(r, c, val) => {
                            // Only keep the stamp if NEITHER the row nor the column is ground
                            if !r.is_ground() && !c.is_ground() {
                                Some(Stamp::Matrix(r, c, val))
                            } else {
                                None
                            }
                        }
                        Stamp::Rhs(r, val) => {
                            if !r.is_ground() {
                                Some(Stamp::Rhs(r, val))
                            } else {
                                None
                            }
                        }
                    }),
            );
        }

        stamps
    }

    pub fn check_convergence(
        &mut self,
        circuit_state: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> crate::error::Result<bool> {
        for (_, component) in &mut self.components {
            let tran_comp = component.as_transient_mut().unwrap();
            if !tran_comp.check_convergence(circuit_state, transient_analysis_context, context) {
                return Ok(false);
            }
        }

        Ok(true)
    }
}
