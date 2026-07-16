use crate::analysis::dc::DcAnalysis;
use crate::prelude::DcAnalysisResult;
use crate::analog::AnalogReference;
use crate::math::linear::Stamp;
use crate::math::unit::Hertz;
use crate::solver::Context;
use num_complex::Complex;

pub struct AcAnalysisContext {
    pub frequency: Hertz,
}

pub trait AcAnalysis: DcAnalysis {
    fn load_ac(
        &mut self,
        dc_analysis_result: &DcAnalysisResult,
        ac_analysis_context: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex<f64>>>;
}

#[derive(Clone, Debug)]
pub struct AcSweepAnalysisOptions {
    pub start_frequency: f64,
    pub stop_frequency: f64,
    pub steps: usize,
    pub logarithmic: bool,
}

impl AcSweepAnalysisOptions {
    /// Generates frequency points for the sweep.
    ///
    /// # Returns
    ///
    /// A vector of frequencies distributed between `start_frequency` and `stop_frequency`.
    /// If `logarithmic` is true, uses logarithmic spacing; otherwise uses linear spacing.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let options = AcSweepAnalysisOptions {
    ///     start_frequency: 1.0,
    ///     stop_frequency: 1000.0,
    ///     steps: 3,
    ///     logarithmic: true,
    /// };
    /// let freqs = options.generate_frequencies();
    /// // freqs ≈ [1.0, 31.62, 1000.0] (logarithmic spacing)
    /// ```
    pub fn generate_frequencies(&self) -> Vec<f64> {
        if self.steps <= 1 {
            return vec![self.start_frequency];
        }

        (0..self.steps)
            .map(|i| {
                let ratio = i as f64 / (self.steps - 1) as f64;
                if self.logarithmic {
                    // Logarithmic spacing: f = f_start * (f_stop / f_start)^ratio
                    self.start_frequency * (self.stop_frequency / self.start_frequency).powf(ratio)
                } else {
                    // Linear spacing: f = f_start + (f_stop - f_start) * ratio
                    self.start_frequency + (self.stop_frequency - self.start_frequency) * ratio
                }
            })
            .collect()
    }
}



/// Per-analysis config for AC. Thin wrapper over the sweep options.
#[derive(Debug, Clone)]
pub struct AcContext {
    pub sweep: AcSweepAnalysisOptions,
}
