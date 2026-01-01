use crate::solver::NodeRef;
use std::sync::Arc;

pub enum SolverError {
    ComponentValidationError {
        name: String,
        component: String,
        details: String,
    },
    ModelValidationError {
        name: String,
        model: String,
        details: String,
    },
}

pub enum SolverWarningLevel {
    LOW,
    MEDIUM,
    HIGH,
}

pub struct SolverWarning {
    level: SolverWarningLevel,
    name: String,
    component: String,
    details: String,
}

pub struct SolverRuntime {}

pub type SolverResult<T> = Result<T, SolverError>;

pub enum Stamp<T> {
    /// Contribution to the A matrix: (row, col, value)
    Matrix { r: usize, c: usize, value: T },
    /// Contribution to the RHS vector b: (row, value)
    Rhs { r: usize, value: T },
}

pub struct Variables<T> {
    pub values: Vec<T>,
}

pub enum AnalysisInfo {
    OP,
}

pub struct Options {
    pub temp: f64,
    pub gmin: f64,
}

pub struct ResistorComponentParams {
    pub name: String,
    pub n_plus: NodeRef,
    pub n_minus: NodeRef,
    pub model: Option<String>,
    pub resistance: Option<f64>,
    pub ac_resistance: Option<f64>,
    pub multiplier: Option<usize>,
    pub scale: Option<f64>,
    pub length: Option<f64>,
    pub width: Option<f64>,
    pub temp: Option<f64>,
    pub dtemp: Option<f64>,
    pub tc1: Option<f64>,
    pub tc2: Option<f64>,
    pub noisy: Option<bool>,
}

pub struct ResistorComponent {
    pub model: Arc<ResistorModel>,
    pub n_plus: NodeRef,
    pub n_minus: NodeRef,
    pub resistance: f64,
    pub ac_resistance: f64,
    pub multiplier: usize,
    pub scale: f64,
    pub temp: f64,
    pub dtemp: f64,
    pub tc1: f64,
    pub tc2: f64,
    pub noisy: bool,
}

pub trait SolverContext {
    fn get_model_instance<M: Model>(&mut self, name: Option<String>) -> SolverResult<Arc<M>>;
    fn raise_warning(&mut self, waning: SolverWarning);
}

pub trait Component: Sized {
    type ComponentParamsType;
    fn instantiate(
        params: Self::ComponentParamsType,
        options: &Options,
        solver_context: &mut impl SolverContext,
    ) -> SolverResult<Self>;
}

impl Component for ResistorComponent {
    type ComponentParamsType = ResistorComponentParams;

    fn instantiate(
        params: Self::ComponentParamsType,
        options: &Options,
        solver_context: &mut impl SolverContext,
    ) -> SolverResult<Self> {
        if (params.n_plus == params.n_minus) {
            return Err(SolverError::ComponentValidationError {
                name: params.name.clone(),
                component: "Resistor".to_string(),
                details: "n_plus and n_minus cannot be the same node".to_string(),
            });
        }

        if params.resistance.is_none() && params.model.is_none() {
            return Err(SolverError::ComponentValidationError {
                name: params.name.clone(),
                component: "Resistor".to_string(),
                details: "Either resistance is set or model must be set".to_string(),
            });
        }

        let model: Arc<ResistorModel> = solver_context.get_model_instance(params.model)?;

        if params.resistance.is_none() && model.resistance.is_none() {
            return Err(SolverError::ComponentValidationError {
                name: params.name.clone(),
                component: "Resistor".to_string(),
                details: "Resistance must be specified either in the component or the model"
                    .to_string(),
            });
        }

        let resistance = params
            .resistance
            .unwrap_or(model.resistance.clone().unwrap());

        if resistance.abs() < options.gmin {
            solver_context.raise_warning(SolverWarning {
                level: SolverWarningLevel::LOW,
                name: params.name.clone(),
                component: "Resistor".to_string(),
                details: "Resistance is too low, setting value to GMIN".to_string(),
            })
        }

        let ac_resistance = params.ac_resistance.unwrap_or(resistance.clone());
        let multiplier = params.multiplier.unwrap_or(1);
        let scale = params.scale.unwrap_or(1.0);
        let temp = params.temp.unwrap_or(options.temp);
        let dtemp = params.dtemp.unwrap_or(0.0);
        let tc1 = params.tc1.unwrap_or(1.0);
        let tc2 = params.tc2.unwrap_or(1.0);
        let noisy = params.noisy.unwrap_or(false);

        Ok(ResistorComponent {
            model,
            n_plus: params.n_plus,
            n_minus: params.n_minus,
            resistance,
            ac_resistance,
            multiplier,
            scale,
            temp,
            dtemp,
            tc1,
            tc2,
            noisy,
        })
    }
}

pub trait Model: Sized {
    type ComponentType: Component;
    type ModelParamsType;

    fn stamp(
        &mut self,
        component: &Self::ComponentType,
        guess: &Variables<f64>,
        an: &AnalysisInfo,
        opts: &Options,
    ) -> Vec<crate::solver::Stamp<f64>>;

    fn setup(params: Self::ModelParamsType) -> SolverResult<Self>;
}

pub struct ResistorModelParams {
    pub name: String,
    pub tc1: Option<f64>,
    pub tc2: Option<f64>,
    pub rsh: Option<f64>,
    pub defw: Option<f64>,
    pub narrow: Option<f64>,
    pub short: Option<f64>,
    pub tnom: Option<f64>,
    pub kf: Option<f64>,
    pub af: Option<f64>,
    pub wf: Option<f64>,
    pub lf: Option<f64>,
    pub ef: Option<f64>,
    pub resistance: Option<f64>,
}

pub struct ResistorModel {
    pub name: String,
    pub resistance: Option<f64>,
    pub temp: f64,
    pub tc1: f64,
    pub tc2: f64,
    pub rsh: f64,
    pub defw: f64,
    pub narrow: f64,
    pub short: f64,
    pub tnom: Option<f64>,
    pub kf: f64,
    pub af: f64,
    pub wf: f64,
    pub lf: f64,
    pub ef: f64,
}

impl Model for ResistorModel {
    type ComponentType = ResistorComponent;
    type ModelParamsType = ResistorModelParams;

    fn stamp(
        &mut self,
        component: &Self::ComponentType,
        guess: &Variables<f64>,
        an: &AnalysisInfo,
        opts: &Options,
    ) -> Vec<crate::solver::Stamp<f64>> {
        vec![]
    }

    fn setup(params: Self::ModelParamsType) -> SolverResult<Self> {
        Ok(ResistorModel {
            name: params.name,
            resistance: params.resistance,
            temp: params.tnom.unwrap_or(27.0),
            tc1: params.tc1.unwrap_or(0.0),
            tc2: params.tc2.unwrap_or(0.0),
            rsh: params.rsh.unwrap_or(50.0),
            defw: params.defw.unwrap_or(2e-6),
            narrow: params.narrow.unwrap_or(1e-7),
            short: params.short.unwrap_or(1e-7),
            tnom: params.tnom,
            kf: params.kf.unwrap_or(1e-25),
            af: params.af.unwrap_or(1.0),
            wf: params.wf.unwrap_or(1.0),
            lf: params.lf.unwrap_or(1.0),
            ef: params.ef.unwrap_or(1.0),
        })
    }
}
