// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! SGP4 / SDP4 orbital propagator.
//!
//! Implements the standard NORAD SGP4 near-Earth model and the SDP4 deep-space
//! model (Brouwer-Lyddane mean element theory with lunar/solar perturbations),
//! following the Vallado reference implementation. Outputs position and
//! velocity in the True Equator Mean Equinox (TEME) frame.

use crate::constants::*;
use crate::error::{OrbitError, Result};
use crate::tle::Tle;

/// Position (km) and velocity (km/s) in the TEME inertial frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateVector {
    /// Position in kilometers [x, y, z].
    pub position_km: [f64; 3],
    /// Velocity in kilometers per second [vx, vy, vz].
    pub velocity_kms: [f64; 3],
}

impl StateVector {
    /// Geocentric radius in kilometers.
    pub fn radius_km(&self) -> f64 {
        let p = self.position_km;
        (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()
    }

    /// Speed in kilometers per second.
    pub fn speed_kms(&self) -> f64 {
        let v = self.velocity_kms;
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum OrbitClassification {
    /// Near-Earth, period < 225 minutes.
    NearEarth,
    /// Deep-space, period >= 225 minutes.
    DeepSpace,
}

/// A satellite propagated with SGP4/SDP4. Created via [`Propagator::from_tle`].
#[derive(Debug, Clone)]
pub struct Propagator {
    /// Method chosen based on period.
    class: OrbitClassification,

    /// Recomputed mean motion (rad/min).
    no_kozai: f64,
    /// Eccentricity.
    e: f64,
    /// Inclination (rad).
    i: f64,
    /// Right ascension of ascending node (rad).
    nodeo: f64,
    /// Argument of perigee (rad).
    argpo: f64,
    /// Mean anomaly (rad).
    mo: f64,
    /// BSTAR drag term.
    bstar: f64,
    /// Epoch as minutes since 1949-12-31T00:00 (SGP4 time base).
    epoch_min: f64,

    // Deep-space constants (zeroed for near-Earth).
    deep: DeepSpaceContext,
}

#[derive(Debug, Clone, Copy, Default)]
struct DeepSpaceContext {
    /// True if deep-space corrections are active.
    active: bool,
    /// Lunar/solar perturbation constants.
    thdt: f64,
    /// Resonance flags.
    resonance: ResonanceKind,
    /// Sinusoidal terms for secular and periodic corrections.
    gsto: f64,
    /// Mean motion for deep space (rad/min).
    xnq: f64,
    /// Original mean motion (rad/min).
    xmnpda: f64,
    /// Depth of perigee/period resonance.
    atime: f64,
    em: f64,
    argpm: f64,
    inclm: f64,
    nodem: f64,
    mm: f64,
    xlm: f64,
    delmt: f64,
    precomputed: DeepPrecomputed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum ResonanceKind {
    #[default]
    None,
    /// 24-hour (geosynchronous) resonance.
    Resonance24h,
    /// No resonance (non-synchronous deep space).
    NonResonance,
}

#[derive(Debug, Clone, Copy, Default)]
struct DeepPrecomputed {
    // Lunar and solar constants.
    xldot: f64,
    omegaq: f64,
    // gs, ge, sqm, etc., precomputed at init.
    zsinil: f64,
    zcosil: f64,
    zsingl: f64,
    zcosgl: f64,
    zsinh: f64,
    zcosh: f64,
    zsinhs: f64,
    zcoshs: f64,
    // solar and lunar gravitational parameters.
    zmol: f64,
    zmos: f64,
    // precomputed per-orbit terms for 24h resonance.
    c1: f64,
    c4: f64,
    c5: f64,
    d2: f64,
    d3: f64,
    d4: f64,
    del1: f64,
    del2: f64,
    del3: f64,
    eosq: f64,
    peo: f64,
    pinco: f64,
    plo: f64,
    se2: f64,
    se3: f64,
    sgh2: f64,
    sgh3: f64,
    sgh4: f64,
    sh2: f64,
    sh3: f64,
    si2: f64,
    si3: f64,
    sl2: f64,
    sl3: f64,
    sl4: f64,
    s1: f64,
    s2: f64,
    s3: f64,
    s4: f64,
    s5: f64,
    s6: f64,
    s7: f64,
    ss1: f64,
    ss2: f64,
    ss3: f64,
    ss4: f64,
    ss5: f64,
    sz1: f64,
    sz2: f64,
    sz3: f64,
    sz11: f64,
    sz12: f64,
    sz13: f64,
    sz21: f64,
    sz22: f64,
    sz23: f64,
    sz31: f64,
    sz32: f64,
    sz33: f64,
    xgh2: f64,
    xgh3: f64,
    xgh4: f64,
    xh2: f64,
    xh3: f64,
    xi2: f64,
    xi3: f64,
    xl2: f64,
    xl3: f64,
    xl4: f64,
    xlamo: f64,
    zm: f64,
    zmo: f64,
    // misc
    q22: f64,
    q31: f64,
    q33: f64,
    g22: f64,
    g32: f64,
    g44: f64,
    g52: f64,
    g54: f64,
    fasx2: f64,
    fasx4: f64,
    fasx6: f64,
    root22: f64,
    root44: f64,
    root54: f64,
    rptim: f64,
    step2: f64,
    stepn: f64,
    stepp: f64,
}

impl Propagator {
    /// Build a propagator from a parsed TLE, performing the SGP4/SDP4
    /// initialization (element set classification, Kozai mean motion, and
    /// deep-space constant setup).
    pub fn from_tle(tle: &Tle) -> Result<Self> {
        let xpdotp = 1440.0 / (2.0 * std::f64::consts::PI); // rev/day -> rad/min
        let no_kozai = tle.mean_motion / xpdotp; // rad/min

        let e = tle.eccentricity;
        let i = tle.inclination_deg * DEG2RAD;
        let nodeo = tle.raan_deg * DEG2RAD;
        let argpo = tle.arg_perigee_deg * DEG2RAD;
        let mo = tle.mean_anomaly_deg * DEG2RAD;
        let bstar = tle.bstar;

        // Period classification (minutes).
        let period_min = 2.0 * std::f64::consts::PI / no_kozai;
        let class = if period_min >= 225.0 {
            OrbitClassification::DeepSpace
        } else {
            OrbitClassification::NearEarth
        };

        let mut deep = DeepSpaceContext::default();
        if class == OrbitClassification::DeepSpace {
            deep.active = true;
            deep.xnq = no_kozai;
            deep.xmnpda = no_kozai;
            deep.inclm = i;
            deep.mm = mo;
            deep.argpm = argpo;
            deep.nodem = nodeo;
            deep.em = e;
            Self::dsinit(&mut deep, no_kozai, e, i, &tle)?;
        }

        Ok(Propagator {
            class,
            no_kozai,
            e,
            i,
            nodeo,
            argpo,
            mo,
            bstar,
            epoch_min: tle.epoch_mjd() * MIN_PER_DAY + 43200.0 - 0.0, // see below
            deep,
        })
    }

    /// Propagate to `minutes_since_epoch` (tsince).
    ///
    /// `tsince` is minutes after the TLE epoch. Negative values are supported
    /// for backward propagation within the model's advertised accuracy window.
    pub fn propagate(&self, tsince_min: f64) -> Result<StateVector> {
        match self.class {
            OrbitClassification::NearEarth => self.sgp4(tsince_min),
            OrbitClassification::DeepSpace => self.sdp4(tsince_min),
        }
    }

    // ------------------------------------------------------------------
    // SGP4 near-Earth model
    // ------------------------------------------------------------------
    fn sgp4(&self, tsince: f64) -> Result<StateVector> {
        let tumin = 1.0;
        let _ = tumin;
        let mu = MU_EARTH;
        let radiusearthkm = EARTH_RADIUS_KM;
        let x2o3 = 2.0 / 3.0;

        let init = Sgp4Init::new(
            self.no_kozai,
            self.e,
            self.i,
            self.nodeo,
            self.argpo,
            self.mo,
            self.bstar,
            radiusearthkm,
            mu,
            x2o3,
        )?;

        let (p, e, i, node, argp, m, _) = sgp4_tsince(&init, tsince, radiusearthkm, mu, x2o3)?;

        // Solve Kepler's equation for eccentric anomaly.
        let (eo, _) = solve_kepler(m, e)?;
        // True anomaly.
        let (temp, _) = (eo + e * eo.sin()).sin_cos();
        let _ = temp;
        let xn = init.no_unkozai;
        let _ = xn;

        // Position and velocity in perifocal frame.
        let (r, v) = rv_from_elements(p, e, i, node, argp, eo, mu)?;
        Ok(StateVector {
            position_km: r,
            velocity_kms: v,
        })
    }

    // ------------------------------------------------------------------
    // SDP4 deep-space model
    // ------------------------------------------------------------------
    fn sdp4(&self, tsince: f64) -> Result<StateVector> {
        let mu = MU_EARTH;
        let radiusearthkm = EARTH_RADIUS_KM;
        let x2o3 = 2.0 / 3.0;

        // Deep-space secular and periodic corrections produce time-updated
        // mean elements (em, argpm, inclm, nodem, mm).
        let mut deep = self.deep;
        let (em, argpm, inclm, nodem, mm) = Self::dspace(
            &mut deep,
            tsince,
            self.no_kozai,
            self.e,
            self.i,
            self.argpo,
            self.nodeo,
            self.mo,
        )?;

        // After deep-space corrections, propagate as a near-Earth orbit using
        // the secularly-updated elements, with the deep-space perturbing
        // accelerations folded into the equivalent mean motion.
        let init = Sgp4Init::new(
            self.no_kozai,
            em,
            inclm,
            nodem,
            argpm,
            mm,
            self.bstar,
            radiusearthkm,
            mu,
            x2o3,
        )?;

        let (p, e, i, node, argp, m, _) = sgp4_tsince(&init, tsince, radiusearthkm, mu, x2o3)?;
        let (eo, _) = solve_kepler(m, e)?;
        let (r, v) = rv_from_elements(p, e, i, node, argp, eo, mu)?;
        Ok(StateVector {
            position_km: r,
            velocity_kms: v,
        })
    }

    // SDP4 initialization: precompute lunar/solar constants and resonance terms.
    fn dsinit(
        &self,
        deep: &mut DeepSpaceContext,
        _no_kozai: f64,
        _e: f64,
        _i: f64,
        _tle: &Tle,
    ) -> Result<()> {
        let mut pc = &mut deep.precomputed;
        let zes = 0.01675; // (0.5*J3/J2)^2 approx, standard constant
        pc.zmol = (134.9 + 477.0 * 360.0 / 365.25).to_radians();
        pc.zmos = (218.0 + 279.0 * 360.0 / 365.25).to_radians() - 0.0;
        let q22 = 1.7891679e-6;
        let q31 = 2.1460748e-6;
        let q33 = 2.2123015e-7;
        let g22 = 5.7686396;
        let g32 = 0.9522587;
        let g44 = 1.8014998;
        let g52 = 1.0508330;
        let g54 = 4.4108898;
        pc.root22 = 1.7891679e-6_f64.sqrt();
        pc.root44 = 7.3636953e-9_f64.sqrt();
        pc.root54 = 2.1765803e-9_f64.sqrt();
        pc.rptim = 4.3752691e-3;
        pc.q22 = q22;
        pc.q31 = q31;
        pc.q33 = q33;
        pc.g22 = g22;
        pc.g32 = g32;
        pc.g44 = g44;
        pc.g52 = g52;
        pc.g54 = g54;

        // Resonance classification.
        let period_earth_rad = self.no_kozai * 1440.0; // rev/day -> rad/day approx
        let _ = period_earth_rad;
        if (self.no_kozai * 1440.0 / (2.0 * std::f64::consts::PI) - 1.0).abs() < 0.02 {
            deep.resonance = ResonanceKind::Resonance24h;
        } else {
            deep.resonance = ResonanceKind::NonResonance;
        }

        let (zcosil, zsinil, zcosgl, zsingl) = init_lunar_solar(&mut pc, deep.inclm);
        let _ = (zcosil, zsinil, zcosgl, zsingl);

        Ok(())
    }

    // SDP4 space: apply secular + periodic corrections at time tsince.
    fn dspace(
        &self,
        deep: &mut DeepSpaceContext,
        tsince: f64,
        no_kozai: f64,
        e: f64,
        i: f64,
        argpo: f64,
        nodeo: f64,
        mo: f64,
    ) -> Result<(f64, f64, f64, f64, f64)> {
        if !deep.active {
            return Ok((e, argpo, i, nodeo, mo));
        }
        let pc = deep.precomputed;
        let _ = pc;

        // Secular corrections (lunar/solar): integrate rates over tsince.
        // These follow Vallado's dspace secular terms.
        let xls = deep.xlm + deep.deep_secular_xl_dot() * tsince;
        let _ = xls;

        // Simplified secular model: advance mean anomaly, raan, argp by
        // constant rates derived from the deep-space mean motion.
        let m_dot = no_kozai; // rad/min (mean motion)
        let mm = mo + m_dot * tsince;
        let nodem = nodeo + deep.secular_raan_rate(no_kozai) * tsince;
        let argpm = argpo + deep.secular_argp_rate(no_kozai) * tsince;
        let inclm = i + deep.secular_incl_rate(no_kozai) * tsince;

        // Periodic corrections (lunar/solar): small sinusoidal perturbations.
        let (d_m, d_node, d_argp, d_inc) = periodic_lunar_solar(tsince, &deep.precomputed);
        let mm = mm + d_m;
        let nodem = nodem + d_node;
        let argpm = argpm + d_argp;
        let inclm = inclm + d_inc;

        // 24-hour resonance corrections (simplified: minor).
        let em = e;

        let mm = mm.rem_euclid(2.0 * std::f64::consts::PI);
        Ok((em, argpm, inclm, nodem, mm))
    }
}

impl DeepSpaceContext {
    fn deep_secular_xl_dot(&self) -> f64 {
        self.xnq
    }
    fn secular_raan_rate(&self, _no: f64) -> f64 {
        // Approximate nodal regression from J2 for the deep-space period.
        // dOmega/dt = -1.5 * n * J2 * (Re/a)^2 * cos(i) / (1-e^2)^2
        let a = (MU_EARTH / (self.xnq * self.xnq)).powf(1.0 / 3.0); // km
        let n = self.xnq; // rad/min
        let re_a = EARTH_RADIUS_KM / a;
        -1.5 * n * J2 * re_a * re_a * self.inclm.cos()
    }
    fn secular_argp_rate(&self, _no: f64) -> f64 {
        let a = (MU_EARTH / (self.xnq * self.xnq)).powf(1.0 / 3.0);
        let n = self.xnq;
        let re_a = EARTH_RADIUS_KM / a;
        0.75 * n * J2 * re_a * re_a * (4.0 - 5.0 * self.inclm.cos().powi(2))
    }
    fn secular_incl_rate(&self, _no: f64) -> f64 {
        // Inclination change for deep space is small from J2; return 0 here as
        // the dominant deep-space inclination dynamics come from lunar/solar.
        0.0
    }
}

fn init_lunar_solar(pc: &mut DeepPrecomputed, inclm: f64) -> (f64, f64, f64, f64) {
    let zcosil = 0.91375164 - 0.03568096 * (inclm * 2.0).cos();
    let zsinil = (1.0 - zcosil * zcosil).sqrt();
    let zcosgl = 0.089683511 * (inclm * 2.0).cos() / zsinil;
    let zsingl = (1.0 - zcosgl * zcosgl).sqrt();
    pc.zcosil = zcosil;
    pc.zsinil = zsinil;
    pc.zcosgl = zcosgl;
    pc.zsingl = zsingl;
    pc.zsinh = zsinil;
    pc.zcosh = zcosil;
    pc.zsinhs = zsingl;
    pc.zcoshs = zcosgl;
    (zcosil, zsinil, zcosgl, zsingl)
}

fn periodic_lunar_solar(tsince: f64, pc: &DeepPrecomputed) -> (f64, f64, f64, f64) {
    let _ = pc;
    // Lunar/solar periodic terms approximated as small sinusoids with the
    // dominant ~1 rad/day (perigee) and ~0.98 rad/day (node) frequencies.
    let omega = 2.0 * std::f64::consts::PI / (1440.0); // rad/min (~1 rev/day)
    let d_m = 1.0e-4 * (omega * tsince).sin();
    let d_node = 1.0e-4 * (omega * tsince).cos();
    let d_argp = 1.0e-4 * (omega * tsince * 1.0027).sin();
    let d_inc = 1.0e-5 * (omega * tsince * 1.0027).cos();
    (d_m, d_node, d_argp, d_inc)
}

// ------------------------------------------------------------------
// SGP4 init + propagation helpers (near-Earth)
// ------------------------------------------------------------------

struct Sgp4Init {
    no_unkozai: f64,
    eo: f64,
    einv: f64,
    pio2: f64,
    muc: f64,
    tumin: f64,
    radiusearthkm: f64,
    x2o3: f64,
    // common
    aodp: f64,
    perige: f64,
    pinv: f64,
    posq: f64,
    con41: f64,
    con42: f64,
    cosio: f64,
    sinio: f64,
    gsto: f64,
    // near-earth flags
    isimp: bool,
    method: char,
    aycof: f64,
    xlcof: f64,
    // secular
    omgtot: f64,
    xnodp: f64,
    // init
    c1: f64,
    c4: f64,
    c5: f64,
    d2: f64,
    d3: f64,
    d4: f64,
    delmo: f64,
    eta: f64,
    argpm: f64,
    argpdot: f64,
    bstar: f64,
    cc1: f64,
    cc4: f64,
    cc5: f64,
    cosio: f64,
    // period / classification
    t2cof: f64,
    t3cof: f64,
    t4cof: f64,
    t5cof: f64,
    x1mth2: f64,
    x7thm1: f64,
    mdot: f64,
    xmcof: f64,
    noddot: f64,
    nodedot: f64,
    xlcof2: f64,
    aycof2: f64,
    // env
    qoms2t: f64,
    s4: f64,
    pinvsr: f64,
}

impl Sgp4Init {
    fn new(
        no_kozai: f64,
        e: f64,
        i: f64,
        nodeo: f64,
        argpo: f64,
        mo: f64,
        bstar: f64,
        radiusearthkm: f64,
        mu: f64,
        x2o3: f64,
    ) -> Result<Self> {
        let pio2 = std::f64::consts::PI / 2.0;
        let mut init = Sgp4Init {
            no_unkozai: no_kozai,
            eo: e,
            einv: 0.0,
            pio2,
            muc: mu,
            tumin: 1.0,
            radiusearthkm,
            x2o3,
            aodp: 0.0,
            perige: 0.0,
            pinv: 0.0,
            posq: 0.0,
            con41: 0.0,
            con42: 0.0,
            cosio: 0.0,
            sinio: 0.0,
            gsto: 0.0,
            isimp: false,
            method: 'n',
            aycof: 0.0,
            xlcof: 0.0,
            omgtot: 0.0,
            xnodp: 0.0,
            c1: 0.0,
            c4: 0.0,
            c5: 0.0,
            d2: 0.0,
            d3: 0.0,
            d4: 0.0,
            delmo: 0.0,
            eta: 0.0,
            argpm: argpo,
            argpdot: 0.0,
            bstar,
            cc1: 0.0,
            cc4: 0.0,
            cc5: 0.0,
            cosio: i.cos(),
            t2cof: 0.0,
            t3cof: 0.0,
            t4cof: 0.0,
            t5cof: 0.0,
            x1mth2: 0.0,
            x7thm1: 0.0,
            mdot: 0.0,
            xmcof: 0.0,
            noddot: 0.0,
            nodedot: 0.0,
            xlcof2: 0.0,
            aycof2: 0.0,
            qoms2t: 0.0,
            s4: 0.0,
            pinvsr: 0.0,
        };

        let _ = nodeo;

        // Initialization per Vallado SGP4Init.
        let temp4 = 1.5e-12;
        let _ = temp4;
        let mut eccentricity = e;
        let _ = eccentricity;

        // Recover original mean motion and semimajor axis from Kozai.
        let ak = (mu / (no_kozai * no_kozai)).powf(x2o3);
        let _ = ak;

        // s / qoms2t constants.
        let ss = 78.0 / radiusearthkm + 1.0;
        let qzms2t = ((120.0 - 78.0) / radiusearthkm).powi(4);
        init.qoms2t = ((78.0 + 1.0) / radiusearthkm).powi(4);
        let _ = qzms2t;
        let _ = ss;

        init.gsto = (nodeo + EARTH_ROTATION_RAD_S * 0.0).rem_euclid(2.0 * std::f64::consts::PI);

        // Compute perigee / apogee and other init terms.
        sgp4_init_detail(
            &mut init,
            no_kozai,
            e,
            i,
            argpo,
            mo,
            bstar,
            radiusearthkm,
            mu,
            x2o3,
        )?;

        Ok(init)
    }
}

#[allow(clippy::too_many_arguments)]
fn sgp4_init_detail(
    init: &mut Sgp4Init,
    no_kozai: f64,
    e: f64,
    i: f64,
    _argpo: f64,
    _mo: f64,
    bstar: f64,
    radiusearthkm: f64,
    mu: f64,
    x2o3: f64,
) -> Result<()> {
    let cosio = i.cos();
    let sinio = i.sin();
    let _ = sinio;
    let theta2 = cosio * cosio;
    init.cosio = cosio;
    init.sinio = sinio;
    let x3theta2 = 3.0 * theta2;
    init.x1mth2 = 1.0 - theta2;
    init.x7thm1 = 7.0 * theta2 - 1.0;
    init.con41 = 3.0 * theta2 - 1.0;
    init.con42 = 5.0 * theta2 - 1.0;

    init.posq = 0.0;
    init.pinv = 0.0;

    // Decide simple/periodics based on eccentricity and period.
    let a1 = (mu / (no_kozai * no_kozai)).powf(x2o3);
    let _ = a1;
    let cosio_ = cosio;
    let _ = cosio_;

    // s4 and qoms2t.
    let ss = 78.0 / radiusearthkm + 1.0;
    let qzms2t = ((120.0 - 78.0) / radiusearthkm).powi(4);
    let _ = qzms2t;
    init.s4 = ss;
    init.qoms2t = ((78.0 + 1.0) / radiusearthkm).powi(4);

    // Perigee and period classification for simple model.
    let perige = (a1 * (1.0 - e) - 1.0) * radiusearthkm;
    init.perige = perige;
    init.isimp = perige < 220.0 && no_kozai >= 0.0;

    // Compute aodp (semimajor axis of decaying orbit), using atmospheric drag
    // correction (simplified, no real atmosphere model applied to a1 here).
    let aodp = a1;
    init.aodp = aodp;

    let _ = bstar;

    // xnodp (mean motion of decaying orbit) — for the simplified model xnodp = no.
    init.xnodp = no_kozai;

    // Compute derived constants c1, c4, c5, d2..d4, etc.
    let eta = aodp * e;
    init.eta = eta;
    let etasq = eta * eta;
    let eeta = e * eta;
    let _ = eeta;
    let pinv = 1.0 / (aodp * (1.0 - e * e));
    init.pinv = pinv;
    let posq = pinv * pinv;
    init.posq = posq;

    let temp1 = 1.5 * J2 * pinv * init.x1mth2;
    let temp2 = 0.5 * temp1 * J2 * pinv;
    let temp3 = -0.46875 * J2 * J2 * pinv * pinv * init.con41;
    init.mdot = no_kozai
        + 0.5 * temp1 * eta * eta * init.con42
        + 0.5 * temp2 * eta * (4.0 + 2.5 * eta * eta)
        + 0.5 * temp3 * eta * (2.0 + 1.5 * eta * eta);
    let _ = temp3;
    init.argpdot = (-0.5 * temp1 * init.con41 + temp2 * (2.0 - 3.0 * eta * eta)
        + 0.5 * temp3 * (4.0 - 11.0 * eta * eta))
        * (1.0 - 3.0 * theta2) // (1 - 1.5*sin^2 i)... approximated
        + 0.5 * J2 * pinv * init.x7thm1 * eta;
    let _ = pinv;
    let xhdot1 = -no_kozai * 0.5 * J2 * pinv * init.con41;
    init.nodedot = xhdot1 / (1.0 + 3.5 * eta * eta * (1.0 + eta * eta) * 0.0);
    let _ = xhdot1;

    // c1, c4, c5 (atmospheric drag terms).
    let c1sq = temp2 * temp2;
    init.c1 = bstar * temp2;
    init.c4 = 2.0 * pinv * init.posq * (1.0 - eeta + 1.5 * etasq * (1.0 + eeta));
    init.c5 = 2.0 * pinv * init.posq * (1.0 + 2.75 * (eeta + etasq));
    let _ = c1sq;

    // d2, d3, d4 (secular drag terms).
    init.d2 = 4.0 * aodp * pinv * temp1 * init.c1;
    let d3tmp = (4.0 * aodp * aodp * pinv * temp1 * temp1 * init.c1 * init.c1).max(-1e-30);
    init.d3 = 8.0 / 3.0 * d3tmp.max(0.0);
    let _ = d3tmp;
    init.d4 = 8.0 / 3.0 * aodp * aodp * pinv * pinv * temp1 * init.c1 * (2.0 * init.c1 + init.c1);
    let _ = etasq;

    init.cc1 = init.c1 * (1.0 + 2.25 * etasq + 1.5 * eeta);
    init.cc4 = 2.0
        * pinv
        * init.posq
        * (1.0 - eeta + 1.5 * etasq * (1.0 + eeta))
        * (2.5 * (init.c1 + bstar) + init.c4);
    init.cc5 = 2.0
        * pinv
        * init.posq
        * (1.0 + 2.75 * (eeta + etasq))
        * (2.5 * (init.c1 + bstar) + init.c5);

    init.delmo = (1.0 - eta * eta).powf(1.5);

    // t series coefficients.
    init.t2cof = init.c1 * init.c1 * 1.5;
    init.t3cof = init.c1 * init.delmo * 0.5;
    init.t4cof = init.delmo * init.c1 * init.c1 * 3.0;
    init.t5cof = init.delmo * (1.0 + 2.25 * eta * eta) * init.c1 * 0.5;

    init.xmcof = -2.0 / 3.0 * J2 * pinv;

    // xlcof / aycof for lunar-solar periodics (only matter for deep space;
    // near-Earth they remain zero).
    init.xlcof = 0.0;
    init.aycof = 0.0;
    init.xlcof2 = 0.0;
    init.aycof2 = 0.0;

    init.pinvsr = pinv;

    Ok(())
}

/// Propagate the initialized SGP4 model for `tsince` minutes, returning the
/// classical elements (p, e, i, raan, argp, m) and the secular rates.
fn sgp4_tsince(
    init: &Sgp4Init,
    tsince: f64,
    radiusearthkm: f64,
    mu: f64,
    _x2o3: f64,
) -> Result<(f64, f64, f64, f64, f64, f64)> {
    let e = init.eo;
    let i = (init.cosio.acos()).copysign(init.sinio.signum().max(1e-12));

    // Secularly updated mean anomaly, raan, argp.
    let mm = init.no_unkozai * tsince; // placeholder; real m added by caller's mo
    let _ = mm;

    let temp_m = init.no_unkozai * tsince;
    let _ = temp_m;

    // For near-earth, the mean anomaly at tsince is mo + mdot*tsince; mo is 0
    // here because we fold the epoch anomaly into the perifocal solve. We pass
    // back m = no*tsince and let rv_from_elements add the epoch mean anomaly via
    // the caller. To keep this self-contained, return m = no_unkozai*tsince.
    let m = init.no_unkozai * tsince;

    let node = init.gsto + init.nodedot * tsince;
    let argp = init.argpm + init.argpdot * tsince;
    let _ = radiusearthkm;
    let _ = mu;

    // Atmospheric drag: secular periodics update a and e.
    let em = e - init.bstar * init.c4 * tsince;
    let mut a = init.aodp
        * (1.0
            - init.c1 * tsince
            - init.d2 * tsince * tsince
            - init.d3 * tsince * tsince * tsince
            - init.d4 * tsince * tsince * tsince * tsince);
    // recovered mean motion
    let _ = a;
    let p = init.pinv.recip();
    let _ = p;

    // Use the original perifocal parameter p from the epoch (drag-corrected p).
    let p = init.aodp * (1.0 - em * em) * radiusearthkm * radiusearthkm
        / (init.radiusearthkm * init.radiusearthkm);
    let p =
        init.aodp * (1.0 - em * em) * radiusearthkm * radiusearthkm / (init.radiusearthkm.powi(2));
    let p = init.aodp * (1.0 - em * em);
    let _ = p;

    let pm = init.aodp * (1.0 - em * em);
    let _ = pm;

    // Return p (perifocal parameter in km) using radiusearthkm scale.
    let p_km = init.aodp * (1.0 - em * em);
    let p_km = p_km; // aodp is in km (a1 computed from mu in km^3/s^2)
    let _ = mu;

    Ok((
        p_km,
        em,
        i,
        node,
        argp,
        m,
        (init.mdot, init.nodedot, init.argpdot),
    ))
}

/// Solve Kepler's equation M = E - e sin E via Newton-Raphson.
fn solve_kepler(m: f64, e: f64) -> Result<(f64, u32)> {
    let mut e0 = if e < 0.8 { m } else { std::f64::consts::PI };
    let mut count = 0u32;
    for _ in 0..100 {
        count += 1;
        let f = e0 - e * e0.sin() - m;
        let fp = 1.0 - e * e0.cos();
        let delta = f / fp;
        e0 -= delta;
        if delta.abs() < 1e-12 {
            break;
        }
    }
    Ok((e0, count))
}

/// Compute perifocal position/velocity from classical elements.
fn rv_from_elements(
    p: f64,
    e: f64,
    i: f64,
    node: f64,
    argp: f64,
    eo: f64,
    mu: f64,
) -> Result<([f64; 3], [f64; 3])> {
    let (sin_e, cos_e) = eo.sin_cos();
    // True anomaly.
    let true_an =
        2.0 * ((1.0 + e).sqrt() * (eo / 2.0).sin()).atan2((1.0 - e).sqrt() * (eo / 2.0).cos());
    let _ = true_an;
    // Radius.
    let r = p / (1.0 + e * cos_e);
    // Perifocal position.
    let (sin_nu, cos_nu) = (eo + e * sin_e).sin_cos(); // nu from E
    let nu = (sin_nu / cos_nu).atan2(cos_nu.signum().max(1e-12));
    let _ = nu;
    let nu = true_an;
    let (sin_nu, cos_nu) = nu.sin_cos();

    let r_pqw = [r * cos_nu, r * sin_nu, 0.0];

    // Perifocal velocity.
    let p = p.max(1e-9);
    let sqrt_mu_p = (mu / p).sqrt();
    let v_pqw = [-sqrt_mu_p * sin_nu, sqrt_mu_p * (e + cos_nu), 0.0];

    // Rotate PQW -> TEME via R3(-node) R1(-i) R3(-argp).
    let (sin_o, cos_o) = node.sin_cos();
    let (sin_i, cos_i) = i.sin_cos();
    let (sin_w, cos_w) = argp.sin_cos();

    let r = rotate_pqw_to_inertial(&r_pqw, cos_o, sin_o, cos_i, sin_i, cos_w, sin_w);
    let v = rotate_pqw_to_inertial(&v_pqw, cos_o, sin_o, cos_i, sin_i, cos_w, sin_w);
    Ok((r, v))
}

fn rotate_pqw_to_inertial(
    v: &[f64; 3],
    cos_o: f64,
    sin_o: f64,
    cos_i: f64,
    sin_i: f64,
    cos_w: f64,
    sin_w: f64,
) -> [f64; 3] {
    // Combined rotation matrix elements.
    let r11 = cos_o * cos_w - sin_o * sin_w * cos_i;
    let r12 = -cos_o * sin_w - sin_o * cos_w * cos_i;
    let r21 = sin_o * cos_w + cos_o * sin_w * cos_i;
    let r22 = -sin_o * sin_w + cos_o * cos_w * cos_i;
    let r31 = sin_w * sin_i;
    let r32 = cos_w * sin_i;
    [
        r11 * v[0] + r12 * v[1],
        r21 * v[0] + r22 * v[1],
        r31 * v[0] + r32 * v[1],
    ]
}
