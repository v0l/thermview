use crate::image::Params;

const ABS_ZERO: f64 = 273.15;
const ATA1: f64 = 0.006569;
const ATA2: f64 = 0.01262;
const ATB1: f64 = -0.002276;
const ATB2: f64 = -0.00667;
const ATX: f64 = 1.9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Planck {
    pub r1: f64,
    pub b: f64,
    pub f: f64,
    pub o: f64,
    pub r2: f64,
}

impl Default for Planck {
    fn default() -> Self {
        Planck { r1: 21106.77, b: 1501.0, f: 1.0, o: -7340.0, r2: 0.012545258 }
    }
}

impl Planck {
    pub fn or_defaults(self) -> Planck {
        let d = Planck::default();
        let pick = |v: f64, d: f64| if v.is_finite() && v != 0.0 { v } else { d };
        Planck {
            r1: pick(self.r1, d.r1),
            b: pick(self.b, d.b),
            f: pick(self.f, d.f),
            o: pick(self.o, d.o),
            r2: pick(self.r2, d.r2),
        }
    }

    fn radiance(&self, celsius: f64) -> f64 {
        let denom = self.r2 * ((self.b / (celsius + ABS_ZERO)).exp() - self.f);
        if denom <= 0.0 { 1e6 } else { self.r1 / denom - self.o }
    }
}

pub fn tau(distance: f64, air: f64, humidity: f64) -> f64 {
    let h2o = (humidity / 100.0)
        * (1.5587 + 0.06939 * air - 0.00027816 * air.powi(2) + 0.00000068455 * air.powi(3)).exp();
    let d = (distance / 2.0).sqrt();
    let w = h2o.sqrt();
    ATX * (-d * (ATA1 + ATB1 * w)).exp() + (1.0 - ATX) * (-d * (ATA2 + ATB2 * w)).exp()
}

pub struct Conversion {
    planck: Planck,
    divisor: f64,
    background: f64,
}

impl Conversion {
    pub fn new(p: &Params, planck: &Planck) -> Conversion {
        let window_temp = p.air;
        let window_trans = 1.0;
        let window_emission = 1.0 - window_trans;
        let window_reflection = 0.0;
        let e = p.emissivity;
        let tau1 = tau(p.distance, p.air, p.humidity);
        let tau2 = tau1;
        let refl1 = (1.0 - e) / e * planck.radiance(p.reflected);
        let atm1 = (1.0 - tau1) / e / tau1 * planck.radiance(p.air);
        let wind = window_emission / e / tau1 / window_trans * planck.radiance(window_temp);
        let refl2 = window_reflection / e / tau1 / window_trans * planck.radiance(p.reflected);
        let atm2 = (1.0 - tau2) / e / tau1 / window_trans / tau2 * planck.radiance(p.air);
        Conversion {
            planck: *planck,
            divisor: e * tau1 * window_trans * tau2,
            background: atm1 + atm2 + wind + refl1 + refl2,
        }
    }

    pub fn celsius(&self, raw: f64) -> f64 {
        let object = raw / self.divisor - self.background;
        let p = &self.planck;
        let v = p.r1 / (p.r2 * (object + p.o)) + p.f;
        if v <= 1.0 { -ABS_ZERO } else { p.b / v.ln() - ABS_ZERO }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Params {
        Params { emissivity: 0.95, distance: 1.0, reflected: 20.0, air: 20.0, humidity: 50.0 }
    }

    #[test]
    fn a_blackbody_with_no_atmosphere_inverts_its_own_radiance() {
        let planck = Planck { r1: 17837.531, b: 1450.4, f: 1.0, o: -1143.0, r2: 0.012332781 };
        let p = Params { emissivity: 1.0, distance: 0.0, ..params() };
        let raw = planck.radiance(37.0);
        assert!((Conversion::new(&p, &planck).celsius(raw) - 37.0).abs() < 1e-6);
    }

    #[test]
    fn a_metre_of_air_barely_attenuates() {
        let t = tau(1.0, 20.0, 50.0);
        assert!((t - 0.995_721_603_320_265).abs() < 1e-9, "{t}");
    }
}
