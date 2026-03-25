use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{ElementRef, SpiceComponent, SpiceElement};
use crate::units::{Celsius, Dimensionless, Meter, MeterSquared, Volt};
use std::sync::Arc;

/// MOSFET transistor (`M`).
///
/// `MXXXX nd ng ns nb mname <L=val> <W=val> <AD=val> <AS=val> <PD=val> <PS=val>
///  <NRD=val> <NRS=val> <OFF> <IC=VDS,VGS,VBS> <TEMP=val> <M=val>`
/// See ngspice manual §9.1.
#[derive(Debug)]
pub struct Mosfet {
    name: String,
    drain: Node,
    gate: Node,
    source: Node,
    bulk: Node,
    /// Model (required).
    model: Arc<dyn crate::models::mosfet::MosfetModel + Send + Sync>,
    /// L: Channel length.
    length: Option<Dynamic<Meter>>,
    /// W: Channel width.
    width: Option<Dynamic<Meter>>,
    /// AD: Drain diffusion area.
    ad: Option<Dynamic<MeterSquared>>,
    /// AS: Source diffusion area.
    as_area: Option<Dynamic<MeterSquared>>,
    /// PD: Drain perimeter.
    pd: Option<Dynamic<Meter>>,
    /// PS: Source perimeter.
    ps: Option<Dynamic<Meter>>,
    /// NRD: Number of drain squares for resistance.
    nrd: Option<Dynamic<Dimensionless>>,
    /// NRS: Number of source squares for resistance.
    nrs: Option<Dynamic<Dimensionless>>,
    /// M: Multiplier.
    multiplier: Option<Dynamic<Dimensionless>>,
    /// OFF: Initial condition hint.
    off: bool,
    /// IC: Initial VDS.
    ic_vds: Option<Dynamic<Volt>>,
    /// IC: Initial VGS.
    ic_vgs: Option<Dynamic<Volt>>,
    /// IC: Initial VBS.
    ic_vbs: Option<Dynamic<Volt>>,
    /// TEMP: Instance temperature.
    temp: Option<Dynamic<Celsius>>,
}

impl Clone for Mosfet {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            drain: self.drain.clone(),
            gate: self.gate.clone(),
            source: self.source.clone(),
            bulk: self.bulk.clone(),
            model: Arc::clone(&self.model),
            length: self.length.clone(),
            width: self.width.clone(),
            ad: self.ad.clone(),
            as_area: self.as_area.clone(),
            pd: self.pd.clone(),
            ps: self.ps.clone(),
            nrd: self.nrd.clone(),
            nrs: self.nrs.clone(),
            multiplier: self.multiplier.clone(),
            off: self.off,
            ic_vds: self.ic_vds.clone(),
            ic_vgs: self.ic_vgs.clone(),
            ic_vbs: self.ic_vbs.clone(),
            temp: self.temp.clone(),
        }
    }
}

impl Mosfet {
    pub const SYMBOL: &str = "M";

    pub fn new(
        name: impl Into<String>,
        drain: impl Into<Node>,
        gate: impl Into<Node>,
        source: impl Into<Node>,
        bulk: impl Into<Node>,
        model: Arc<dyn crate::models::mosfet::MosfetModel + Send + Sync>,
    ) -> Self {
        Self {
            name: name.into(),
            drain: drain.into(),
            gate: gate.into(),
            source: source.into(),
            bulk: bulk.into(),
            model,
            length: None,
            width: None,
            ad: None,
            as_area: None,
            pd: None,
            ps: None,
            nrd: None,
            nrs: None,
            multiplier: None,
            off: false,
            ic_vds: None,
            ic_vgs: None,
            ic_vbs: None,
            temp: None,
        }
    }

    pub fn with_length(&mut self, v: impl Into<Dynamic<Meter>>) -> &mut Self {
        self.length = Some(v.into());
        self
    }
    pub fn with_width(&mut self, v: impl Into<Dynamic<Meter>>) -> &mut Self {
        self.width = Some(v.into());
        self
    }
    pub fn with_ad(&mut self, v: impl Into<Dynamic<MeterSquared>>) -> &mut Self {
        self.ad = Some(v.into());
        self
    }
    pub fn with_as(&mut self, v: impl Into<Dynamic<MeterSquared>>) -> &mut Self {
        self.as_area = Some(v.into());
        self
    }
    pub fn with_pd(&mut self, v: impl Into<Dynamic<Meter>>) -> &mut Self {
        self.pd = Some(v.into());
        self
    }
    pub fn with_ps(&mut self, v: impl Into<Dynamic<Meter>>) -> &mut Self {
        self.ps = Some(v.into());
        self
    }
    pub fn with_nrd(&mut self, v: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.nrd = Some(v.into());
        self
    }
    pub fn with_nrs(&mut self, v: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.nrs = Some(v.into());
        self
    }
    pub fn with_multiplier(&mut self, v: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.multiplier = Some(v.into());
        self
    }
    pub fn with_off(&mut self) -> &mut Self {
        self.off = true;
        self
    }
    pub fn with_ic(
        &mut self,
        vds: impl Into<Dynamic<Volt>>,
        vgs: impl Into<Dynamic<Volt>>,
        vbs: impl Into<Dynamic<Volt>>,
    ) -> &mut Self {
        self.ic_vds = Some(vds.into());
        self.ic_vgs = Some(vgs.into());
        self.ic_vbs = Some(vbs.into());
        self
    }
    pub fn with_temp(&mut self, v: impl Into<Dynamic<Celsius>>) -> &mut Self {
        self.temp = Some(v.into());
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn drain(&self) -> &Node {
        &self.drain
    }
    pub fn gate(&self) -> &Node {
        &self.gate
    }
    pub fn source(&self) -> &Node {
        &self.source
    }
    pub fn bulk(&self) -> &Node {
        &self.bulk
    }
    pub fn model_name(&self) -> &str {
        self.model.model_name()
    }
}

impl Component for Mosfet {}

impl SpiceElement for Mosfet {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }

    fn spice_model(&self) -> Option<Arc<dyn crate::spice::SpiceModel>> {
        Some(Arc::clone(&self.model) as Arc<dyn crate::spice::SpiceModel>)
    }
}

impl SpiceComponent for Mosfet {
    fn into_spice(&self) -> String {
        let model_name = self.model.model_name();
        let mut s = format!(
            "{}{} {} {} {} {} {}",
            Self::SYMBOL,
            self.name(),
            self.drain(),
            self.gate(),
            self.source(),
            self.bulk(),
            model_name
        );
        if let Some(v) = &self.length {
            s.push_str(&format!(" L={}", v));
        }
        if let Some(v) = &self.width {
            s.push_str(&format!(" W={}", v));
        }
        if let Some(v) = &self.ad {
            s.push_str(&format!(" AD={}", v));
        }
        if let Some(v) = &self.as_area {
            s.push_str(&format!(" AS={}", v));
        }
        if let Some(v) = &self.pd {
            s.push_str(&format!(" PD={}", v));
        }
        if let Some(v) = &self.ps {
            s.push_str(&format!(" PS={}", v));
        }
        if let Some(v) = &self.nrd {
            s.push_str(&format!(" NRD={}", v));
        }
        if let Some(v) = &self.nrs {
            s.push_str(&format!(" NRS={}", v));
        }
        if let Some(v) = &self.multiplier {
            s.push_str(&format!(" M={}", v));
        }
        if self.off {
            s.push_str(" OFF");
        }
        if let (Some(vds), Some(vgs), Some(vbs)) = (&self.ic_vds, &self.ic_vgs, &self.ic_vbs) {
            s.push_str(&format!(" IC={},{},{}", vds, vgs, vbs));
        }
        if let Some(v) = &self.temp {
            s.push_str(&format!(" TEMP={}", v));
        }
        s
    }
}
