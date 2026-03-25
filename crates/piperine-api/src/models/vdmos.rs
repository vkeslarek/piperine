use crate::devices::Vdmos;
use crate::models::Model;
use crate::spice::SpiceModel;
use crate::units::{Ampere, Celsius, Dimensionless, ElectronVolt, Farad, Ohm, Second, Volt};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

pub trait VdmosModel: Model<ComponentType = Vdmos> + SpiceModel + Debug {}

pub static DEFAULT_NCHAN: LazyLock<Arc<dyn VdmosModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default", VdmosType::Nchan)));

pub static DEFAULT_PCHAN: LazyLock<Arc<dyn VdmosModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default", VdmosType::Pchan)));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VdmosType {
    Nchan,
    Pchan,
}

#[derive(Debug)]
pub struct DefaultModel {
    pub name: String,
    pub vdmos_type: VdmosType,
    pub vto: Option<Volt>,
    pub kp: Option<Dimensionless>,
    pub phi: Option<Volt>,
    pub lambda: Option<Dimensionless>,
    pub theta: Option<Dimensionless>,
    pub rd: Option<Ohm>,
    pub rs: Option<Ohm>,
    pub rg: Option<Ohm>,
    pub tnom: Option<Celsius>,
    pub kf: Option<Dimensionless>,
    pub af: Option<Dimensionless>,
    pub rq: Option<Ohm>,
    pub vq: Option<Volt>,
    pub mtriode: Option<Dimensionless>,
    pub subshift: Option<Dimensionless>,
    pub ksubthres: Option<Dimensionless>,
    pub bv: Option<Volt>,
    pub ibv: Option<Ampere>,
    pub nbv: Option<Dimensionless>,
    pub rds: Option<Ohm>,
    pub rb: Option<Ohm>,
    pub n: Option<Dimensionless>,
    pub tt: Option<Second>,
    pub eg: Option<ElectronVolt>,
    pub xti: Option<Dimensionless>,
    pub is: Option<Ampere>,
    pub vj: Option<Volt>,
    pub cjo: Option<Farad>,
    pub m: Option<Dimensionless>,
    pub fc: Option<Dimensionless>,
    pub cgdmin: Option<Farad>,
    pub cgdmax: Option<Farad>,
    pub a: Option<Dimensionless>,
    pub cgs: Option<Farad>,
    pub rthjc: Option<Dimensionless>,
    pub rthca: Option<Dimensionless>,
    pub cthj: Option<Dimensionless>,
}

impl DefaultModel {
    pub fn new(name: impl Into<String>, vdmos_type: VdmosType) -> Self {
        Self {
            name: name.into(),
            vdmos_type,
            vto: None,
            kp: None,
            phi: None,
            lambda: None,
            theta: None,
            rd: None,
            rs: None,
            rg: None,
            tnom: None,
            kf: None,
            af: None,
            rq: None,
            vq: None,
            mtriode: None,
            subshift: None,
            ksubthres: None,
            bv: None,
            ibv: None,
            nbv: None,
            rds: None,
            rb: None,
            n: None,
            tt: None,
            eg: None,
            xti: None,
            is: None,
            vj: None,
            cjo: None,
            m: None,
            fc: None,
            cgdmin: None,
            cgdmax: None,
            a: None,
            cgs: None,
            rthjc: None,
            rthca: None,
            cthj: None,
        }
    }
}

impl Model for DefaultModel {
    type ComponentType = Vdmos;
}

impl SpiceModel for DefaultModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn to_spice_model_line(&self) -> String {
        let mut params = vec![match self.vdmos_type {
            VdmosType::Nchan => "NCHAN".to_string(),
            VdmosType::Pchan => "PCHAN".to_string(),
        }];

        if let Some(v) = self.vto {
            params.push(format!("VTO={v}"));
        }
        if let Some(v) = self.kp {
            params.push(format!("KP={v}"));
        }
        if let Some(v) = self.phi {
            params.push(format!("PHI={v}"));
        }
        if let Some(v) = self.lambda {
            params.push(format!("LAMBDA={v}"));
        }
        if let Some(v) = self.theta {
            params.push(format!("THETA={v}"));
        }
        if let Some(v) = self.rd {
            params.push(format!("RD={v}"));
        }
        if let Some(v) = self.rs {
            params.push(format!("RS={v}"));
        }
        if let Some(v) = self.rg {
            params.push(format!("RG={v}"));
        }
        if let Some(v) = self.tnom {
            params.push(format!("TNOM={v}"));
        }
        if let Some(v) = self.kf {
            params.push(format!("KF={v}"));
        }
        if let Some(v) = self.af {
            params.push(format!("AF={v}"));
        }
        if let Some(v) = self.rq {
            params.push(format!("RQ={v}"));
        }
        if let Some(v) = self.vq {
            params.push(format!("VQ={v}"));
        }
        if let Some(v) = self.mtriode {
            params.push(format!("MTRIODE={v}"));
        }
        if let Some(v) = self.subshift {
            params.push(format!("SUBSHIFT={v}"));
        }
        if let Some(v) = self.ksubthres {
            params.push(format!("KSUBTHRES={v}"));
        }
        if let Some(v) = self.bv {
            params.push(format!("BV={v}"));
        }
        if let Some(v) = self.ibv {
            params.push(format!("IBV={v}"));
        }
        if let Some(v) = self.nbv {
            params.push(format!("NBV={v}"));
        }
        if let Some(v) = self.rds {
            params.push(format!("RDS={v}"));
        }
        if let Some(v) = self.rb {
            params.push(format!("RB={v}"));
        }
        if let Some(v) = self.n {
            params.push(format!("N={v}"));
        }
        if let Some(v) = self.tt {
            params.push(format!("TT={v}"));
        }
        if let Some(v) = self.eg {
            params.push(format!("EG={v}"));
        }
        if let Some(v) = self.xti {
            params.push(format!("XTI={v}"));
        }
        if let Some(v) = self.is {
            params.push(format!("IS={v}"));
        }
        if let Some(v) = self.vj {
            params.push(format!("VJ={v}"));
        }
        if let Some(v) = self.cjo {
            params.push(format!("CJO={v}"));
        }
        if let Some(v) = self.m {
            params.push(format!("M={v}"));
        }
        if let Some(v) = self.fc {
            params.push(format!("FC={v}"));
        }
        if let Some(v) = self.cgdmin {
            params.push(format!("CGDMIN={v}"));
        }
        if let Some(v) = self.cgdmax {
            params.push(format!("CGDMAX={v}"));
        }
        if let Some(v) = self.a {
            params.push(format!("A={v}"));
        }
        if let Some(v) = self.cgs {
            params.push(format!("CGS={v}"));
        }
        if let Some(v) = self.rthjc {
            params.push(format!("RTHJC={v}"));
        }
        if let Some(v) = self.rthca {
            params.push(format!("RTHCA={v}"));
        }
        if let Some(v) = self.cthj {
            params.push(format!("CTHJ={v}"));
        }

        format!(".MODEL {} VDMOS ({})", self.name, params.join(" "))
    }
}

impl VdmosModel for DefaultModel {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_vdmos_model_with_polarity() {
        let m_n = DefaultModel::new("IRFP240", VdmosType::Nchan);
        assert_eq!(m_n.to_spice_model_line(), ".MODEL IRFP240 VDMOS (NCHAN)");

        let m_p = DefaultModel::new("IRFP9240", VdmosType::Pchan);
        assert_eq!(m_p.to_spice_model_line(), ".MODEL IRFP9240 VDMOS (PCHAN)");
    }

    #[test]
    fn serializes_vdmos_model_with_common_parameters() {
        let mut m = DefaultModel::new("IRFZ48Z", VdmosType::Nchan);
        m.vto = Some(4.0);
        m.rd = Some(0.00185);
        m.rs = Some(0.0);
        m.rg = Some(1.77);
        m.kp = Some(25.0);
        m.cgdmax = Some(2.1e-9);
        m.cgdmin = Some(0.05e-9);
        m.cgs = Some(1.8e-9);

        assert_eq!(
            m.to_spice_model_line(),
            ".MODEL IRFZ48Z VDMOS (NCHAN VTO=4 KP=25 RD=0.00185 RS=0 RG=1.77 CGDMIN=0.00000000005 CGDMAX=0.0000000021 CGS=0.0000000018)"
        );
    }
}
