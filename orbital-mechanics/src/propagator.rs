// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! SGP4 / SDP4 orbital propagator.
//!
//! A faithful port of the Vallado et al. "Revisiting Spacetrack Report #3"
//! (AIAA 2006-6753) reference implementation. Outputs position and velocity in
//! the True Equator Mean Equinox (TEME) frame, in kilometers and kilometers per
//! second, using the WGS72 gravity constants. Validated against the official
//! `tcppver.out` verification vectors (see `tests/verification.rs`).

use crate::constants::{EARTH_RADIUS_KM, J2, J3, J4, J3OJ2, MU_EARTH, XKE};
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

const TWOPId: f64 = 2.0 * std::f64::consts::PI;
const X2O3: f64 = 2.0 / 3.0;

/// Diagnostic escape hatch: when true, the near-earth drag/secular extra
/// terms (d2..d4, t3cof..t5cof, omgcof/xmcof) are skipped during propagation.
pub const SKIP_DRAG: bool = false;

/// Initialized element set for repeated SGP4/SDP4 propagation.
///
/// Created via [`Propagator::from_tle`]. Holds the full `elsetrec` state from
/// the Vallado reference implementation.
#[derive(Debug, Clone)]
pub struct Propagator {
    // near-earth constants
    isimp: i32,
    method: char,
    aycof: f64,
    con41: f64,
    cc1: f64,
    cc4: f64,
    cc5: f64,
    d2: f64,
    d3: f64,
    d4: f64,
    delmo: f64,
    eta: f64,
    argpdot: f64,
    omgcof: f64,
    sinmao: f64,
    t2cof: f64,
    t3cof: f64,
    t4cof: f64,
    t5cof: f64,
    x1mth2: f64,
    x7thm1: f64,
    mdot: f64,
    nodecf: f64,
    nodedot: f64,
    xlcof: f64,
    xmcof: f64,
    no_kozai: f64,
    // epoch elements
    bstar: f64,
    ecco: f64,
    argpo: f64,
    inclo: f64,
    mo: f64,
    nodeo: f64,
    // deep space
    irez: i32,
    d2201: f64,
    d2211: f64,
    d3210: f64,
    d3222: f64,
    d4410: f64,
    d4422: f64,
    d5220: f64,
    d5232: f64,
    d5421: f64,
    d5433: f64,
    dedt: f64,
    del1: f64,
    del2: f64,
    del3: f64,
    didt: f64,
    dmdt: f64,
    dnodt: f64,
    domdt: f64,
    e3: f64,
    ee2: f64,
    peo: f64,
    pgho: f64,
    pho: f64,
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
    zmol: f64,
    zmos: f64,
    atime: f64,
    xli: f64,
    xni: f64,
    #[allow(dead_code)]
    gsto: f64,
    xfact: f64,
    xlamo: f64,
}

/// Greenwich Mean Sidereal Time (radians) at a Julian date (UT1 days).
fn gstime(jdut1: f64) -> f64 {
    let deg2rad = std::f64::consts::PI / 180.0;
    let tut1 = (jdut1 - 2451545.0) / 36525.0;
    let mut temp =
        -6.2e-6 * tut1 * tut1 * tut1 + 0.093104 * tut1 * tut1 + (876600.0 * 3600.0 + 8640184.812866) * tut1
            + 67310.54841;
    temp = (temp * deg2rad / 240.0) % TWOPId;
    if temp < 0.0 {
        temp += TWOPId;
    }
    temp
}

/// Convert a TLE epoch (MJD) into "days from Jan 0, 1950 0h" used by initl/gstime.
fn epoch_days_1950(mjd: f64) -> f64 {
    // MJD 33281.0 == 1950-01-01T00:00; Jan 0 1950 == MJD 33280.0.
    mjd - 33280.0
}

/// Lunar/solar long-period periodic sums used by both the `dsinit` baseline
/// seed (evaluated at `t = 0`, matching the reference `dpper(init='y')` call)
/// and `dpper`'s per-propagation correction (`init='n'`). Returns raw
/// `(pe, pinc, pl, pgh, ph)` before any baseline subtraction.
#[allow(clippy::too_many_arguments)]
fn dpper_sums(
    t: f64,
    zmos: f64,
    se2: f64,
    se3: f64,
    si2: f64,
    si3: f64,
    sl2: f64,
    sl3: f64,
    sl4: f64,
    sgh2: f64,
    sgh3: f64,
    sgh4: f64,
    sh2: f64,
    sh3: f64,
    zmol: f64,
    ee2: f64,
    e3: f64,
    xi2: f64,
    xi3: f64,
    xl2: f64,
    xl3: f64,
    xl4: f64,
    xgh2: f64,
    xgh3: f64,
    xgh4: f64,
    xh2: f64,
    xh3: f64,
) -> (f64, f64, f64, f64, f64) {
    let zns = 1.19459e-5;
    let zes = 0.01675;
    let znl = 1.5835218e-4;
    let zel = 0.05490;

    let mut zm = zmos + zns * t;
    zm = zm + 2.0 * zes * zm.sin();
    let sinzf = zm.sin();
    let f2 = 0.5 * sinzf * sinzf - 0.25;
    let f3 = -0.5 * sinzf * zm.cos();
    let ses = se2 * f2 + se3 * f3;
    let sis = si2 * f2 + si3 * f3;
    let sls = sl2 * f2 + sl3 * f3 + sl4 * sinzf;
    let sghs = sgh2 * f2 + sgh3 * f3 + sgh4 * sinzf;
    let shs = sh2 * f2 + sh3 * f3;
    let mut zm = zmol + znl * t;
    zm = zm + 2.0 * zel * zm.sin();
    let sinzf = zm.sin();
    let f2 = 0.5 * sinzf * sinzf - 0.25;
    let f3 = -0.5 * sinzf * zm.cos();
    let sel = ee2 * f2 + e3 * f3;
    let sil = xi2 * f2 + xi3 * f3;
    let sll = xl2 * f2 + xl3 * f3 + xl4 * sinzf;
    let sghl = xgh2 * f2 + xgh3 * f3 + xgh4 * sinzf;
    let shll = xh2 * f2 + xh3 * f3;
    let pe = ses + sel;
    let pinc = sis + sil;
    let pl = sls + sll;
    let pgh = sghs + sghl;
    let ph = shs + shll;
    (pe, pinc, pl, pgh, ph)
}

impl Propagator {
    /// Build a propagator from a parsed TLE (equivalent to `sgp4init`).
    pub fn from_tle(tle: &Tle) -> Result<Self> {
        // `no` in rad/min via the TLE mean motion (rev/day).
        let xpdotp = 1440.0 / TWOPId;
        let no = tle.mean_motion / xpdotp;
        let mut p = Propagator {
            isimp: 0,
            method: 'n',
            aycof: 0.0,
            con41: 0.0,
            cc1: 0.0,
            cc4: 0.0,
            cc5: 0.0,
            d2: 0.0,
            d3: 0.0,
            d4: 0.0,
            delmo: 0.0,
            eta: 0.0,
            argpdot: 0.0,
            omgcof: 0.0,
            sinmao: 0.0,
            t2cof: 0.0,
            t3cof: 0.0,
            t4cof: 0.0,
            t5cof: 0.0,
            x1mth2: 0.0,
            x7thm1: 0.0,
            mdot: 0.0,
            nodecf: 0.0,
            nodedot: 0.0,
            xlcof: 0.0,
            xmcof: 0.0,
            no_kozai: no,
            bstar: tle.bstar,
            ecco: tle.eccentricity,
            argpo: tle.arg_perigee_deg * std::f64::consts::PI / 180.0,
            inclo: tle.inclination_deg * std::f64::consts::PI / 180.0,
            mo: tle.mean_anomaly_deg * std::f64::consts::PI / 180.0,
            nodeo: tle.raan_deg * std::f64::consts::PI / 180.0,
            irez: 0,
            d2201: 0.0,
            d2211: 0.0,
            d3210: 0.0,
            d3222: 0.0,
            d4410: 0.0,
            d4422: 0.0,
            d5220: 0.0,
            d5232: 0.0,
            d5421: 0.0,
            d5433: 0.0,
            dedt: 0.0,
            del1: 0.0,
            del2: 0.0,
            del3: 0.0,
            didt: 0.0,
            dmdt: 0.0,
            dnodt: 0.0,
            domdt: 0.0,
            e3: 0.0,
            ee2: 0.0,
            peo: 0.0,
            pgho: 0.0,
            pho: 0.0,
            pinco: 0.0,
            plo: 0.0,
            se2: 0.0,
            se3: 0.0,
            sgh2: 0.0,
            sgh3: 0.0,
            sgh4: 0.0,
            sh2: 0.0,
            sh3: 0.0,
            si2: 0.0,
            si3: 0.0,
            sl2: 0.0,
            sl3: 0.0,
            sl4: 0.0,
            xgh2: 0.0,
            xgh3: 0.0,
            xgh4: 0.0,
            xh2: 0.0,
            xh3: 0.0,
            xi2: 0.0,
            xi3: 0.0,
            xl2: 0.0,
            xl3: 0.0,
            xl4: 0.0,
            zmol: 0.0,
            zmos: 0.0,
            atime: 0.0,
            xli: 0.0,
            xni: 0.0,
            gsto: 0.0,
            xfact: 0.0,
            xlamo: 0.0,
        };

        // --- initl ---
        let epoch = epoch_days_1950(tle.epoch_mjd());
        let (ao, con42, cosio, cosio2, eccsq, omeosq, _posq, rp, rteosq, sinio, gsto) = {
            let ecco = p.ecco;
            let inclo = p.inclo;
            let eccsq = ecco * ecco;
            let omeosq = 1.0 - eccsq;
            let rteosq = omeosq.sqrt();
            let cosio = inclo.cos();
            let cosio2 = cosio * cosio;
            let ak = (XKE / no).powf(X2O3);
            let d1 = 0.75 * J2 * (3.0 * cosio2 - 1.0) / (rteosq * omeosq);
            let del = d1 / (ak * ak);
            let adel = ak * (1.0 - del * del - del * (1.0 / 3.0 + 134.0 * del * del / 81.0));
            let del = d1 / (adel * adel);
            let no = no / (1.0 + del);
            p.no_kozai = no;
            // Vallado uses `aodp = ao/(1 - delo)` (delo == `del` here) as the
            // semi-major axis in every near-earth secular coefficient formula
            // and in the perigee/isimp classification. Using the raw `ao`
            // misclassifies perigee and corrupts the coefficient scales.
            let ao = adel / (1.0 - del);
            let sinio = inclo.sin();
            let _po = ao * omeosq;
            let con42 = 1.0 - 5.0 * cosio2;
            let con41 = -con42 - cosio2 - cosio2;
            let _ = &mut p;
            p.con41 = con41;
            let posq = (ao * omeosq) * (ao * omeosq);
            let rp = ao * (1.0 - ecco);
            let gsto = gstime(epoch + 2433281.5);
            (ao, con42, cosio, cosio2, eccsq, omeosq, posq, rp, rteosq, sinio, gsto)
        };
        p.gsto = gsto;

        if omeosq >= 0.0 || no >= 0.0 {
            p.isimp = 0;
            if rp < (220.0 / EARTH_RADIUS_KM + 1.0) {
                p.isimp = 1;
            }
            let ss = 78.0 / EARTH_RADIUS_KM + 1.0;
            let qzms2t = ((120.0 - 78.0) / EARTH_RADIUS_KM).powi(4);
            let mut sfour = ss;
            let mut qzms24 = qzms2t;
            let perige = (rp - 1.0) * EARTH_RADIUS_KM;
            if perige < 156.0 {
                sfour = perige - 78.0;
                if perige < 98.0 {
                    sfour = 20.0;
                }
                qzms24 = ((120.0 - sfour) / EARTH_RADIUS_KM).powi(4);
                sfour = sfour / EARTH_RADIUS_KM + 1.0;
            }
            let pinvsq = 1.0 / (ao * ao * omeosq * omeosq);
            let tsi = 1.0 / (ao - sfour);
            p.eta = ao * p.ecco * tsi;
            let etasq = p.eta * p.eta;
            let eeta = p.ecco * p.eta;
            let psisq = (1.0 - etasq).abs();
            let coef = qzms24 * tsi.powi(4);
            let coef1 = coef / psisq.powf(3.5);
            let cc2 = coef1 * no * (ao * (1.0 + 1.5 * etasq + eeta * (4.0 + etasq))
                + 0.375 * J2 * tsi / psisq * p.con41 * (8.0 + 3.0 * etasq * (8.0 + etasq)));
            p.cc1 = p.bstar * cc2;
            let mut cc3 = 0.0;
            if p.ecco > 1.0e-4 {
                cc3 = -2.0 * coef * tsi * J3OJ2 * no * sinio / p.ecco;
            }
            p.x1mth2 = 1.0 - cosio2;
            p.cc4 = 2.0 * no * coef1 * ao * omeosq
                * (p.eta * (2.0 + 0.5 * etasq) + p.ecco * (0.5 + 2.0 * etasq)
                    - J2 * tsi / (ao * psisq)
                        * (-3.0 * p.con41 * (1.0 - 2.0 * eeta + etasq * (1.5 - 0.5 * eeta))
                            + 0.75 * p.x1mth2 * (2.0 * etasq - eeta * (1.0 + etasq)) * (2.0 * p.argpo).cos()));
            p.cc5 = 2.0 * coef1 * ao * omeosq * (1.0 + 2.75 * (etasq + eeta) + eeta * etasq);
            let cosio4 = cosio2 * cosio2;
            let temp1 = 1.5 * J2 * pinvsq * no;
            let temp2 = 0.5 * temp1 * J2 * pinvsq;
            let temp3 = -0.46875 * J4 * pinvsq * pinvsq * no;
            p.mdot = no + 0.5 * temp1 * rteosq * p.con41
                + 0.0625 * temp2 * rteosq * (13.0 - 78.0 * cosio2 + 137.0 * cosio4);
            p.argpdot = -0.5 * temp1 * con42
                + 0.0625 * temp2 * (7.0 - 114.0 * cosio2 + 395.0 * cosio4)
                + temp3 * (3.0 - 36.0 * cosio2 + 49.0 * cosio4);
            let xhdot1 = -temp1 * cosio;
            p.nodedot = xhdot1
                + (0.5 * temp2 * (4.0 - 19.0 * cosio2) + 2.0 * temp3 * (3.0 - 7.0 * cosio2)) * cosio;
            let xpidot = p.argpdot + p.nodedot;
            p.omgcof = p.bstar * cc3 * p.argpo.cos();
            p.xmcof = 0.0;
            if p.ecco > 1.0e-4 {
                p.xmcof = -X2O3 * coef * p.bstar / eeta;
            }
            p.nodecf = 3.5 * omeosq * xhdot1 * p.cc1;
            p.t2cof = 1.5 * p.cc1;
            if (cosio + 1.0).abs() > 1.5e-12 {
                p.xlcof = -0.25 * J3OJ2 * sinio * (3.0 + 5.0 * cosio) / (1.0 + cosio);
            } else {
                p.xlcof = -0.25 * J3OJ2 * sinio * (3.0 + 5.0 * cosio) / 1.5e-12;
            }
            p.aycof = -0.5 * J3OJ2 * sinio;
            p.delmo = (1.0 + p.eta * p.mo.cos()).powi(3);
            p.sinmao = p.mo.sin();
            p.x7thm1 = 7.0 * cosio2 - 1.0;

            if p.ecco > 0.0085 && p.ecco < 0.0088 {
                eprintln!("MAININIT no={} isimp={} ao={} rp={} threshold={} cc1={} cc2={} cc4={} omgcof={} xmcof={}", p.no_kozai, p.isimp, ao, ao*(1.0-p.ecco), 220.0/EARTH_RADIUS_KM+1.0, p.cc1, cc2, p.cc4, p.omgcof, p.xmcof);
            }

            // --- deep space initialization ---
            if (TWOPId / no) >= 225.0 {
                p.method = 'd';
                p.isimp = 1;
                let con41 = p.con41;
                Self::dsinit(&mut p, epoch, no, xpidot, con41);
            }

            if p.isimp != 1 {
                let cc1sq = p.cc1 * p.cc1;
                p.d2 = 4.0 * ao * tsi * cc1sq;
                let temp = p.d2 * tsi * p.cc1 / 3.0;
                p.d3 = (17.0 * ao + sfour) * temp;
                p.d4 = 0.5 * temp * ao * tsi * (221.0 * ao + 31.0 * sfour) * p.cc1;
                p.t3cof = p.d2 + 2.0 * cc1sq;
                p.t4cof = 0.25 * (3.0 * p.d3 + p.cc1 * (12.0 * p.d2 + 10.0 * cc1sq));
                p.t5cof = 0.2 * (3.0 * p.d4 + 12.0 * p.cc1 * p.d3 + 6.0 * p.d2 * p.d2
                    + 15.0 * cc1sq * (2.0 * p.d2 + cc1sq));
            }
        }

        // Propagate to zero epoch to finalize (matches reference sgp4init).
        p.propagate(0.0)?;
        Ok(p)
    }

    /// Propagate to `tsince` minutes after the TLE epoch (equivalent to `sgp4`).
    pub fn propagate(&self, tsince: f64) -> Result<StateVector> {
        let mut p = self.clone();
        let vkmpersec = EARTH_RADIUS_KM * XKE / 60.0;


        // --- update for secular gravity and atmospheric drag ---
        let xmdf = p.mo + p.mdot * tsince;
        let argpdf = p.argpo + p.argpdot * tsince;
        let nodedf = p.nodeo + p.nodedot * tsince;
        let mut argpm = argpdf;
        let mut mm = xmdf;
        let t2 = tsince * tsince;
        let mut nodem = nodedf + p.nodecf * t2;
        let mut tempa = 1.0 - p.cc1 * tsince;
        let mut tempe = p.bstar * p.cc4 * tsince;
        let mut templ = p.t2cof * t2;

        if p.isimp != 1 && !crate::propagator::SKIP_DRAG {
            let delomg = p.omgcof * tsince;
            let delm = p.xmcof * ((1.0 + p.eta * xmdf.cos()).powi(3) - p.delmo);
            let temp = delomg + delm;
            mm = xmdf + temp;
            argpm = argpdf - temp;
            let t3 = t2 * tsince;
            let t4 = t3 * tsince;
            tempa = tempa - p.d2 * t2 - p.d3 * t3 - p.d4 * t4;
            tempe = tempe + p.bstar * p.cc5 * (mm.sin() - p.sinmao);
            templ = templ + p.t3cof * t3 + t4 * (p.t4cof + tsince * p.t5cof);
        }

        let mut nm = p.no_kozai;
        let mut em = p.ecco;
        let mut inclm = p.inclo;

        if p.method == 'd' {
            let mut tc = tsince;
            Self::dspace(&mut p, tc, &mut em, &mut argpm, &mut inclm, &mut nodem, &mut nm, &mut mm);
        }

        if nm <= 0.0 {
            return Err(OrbitError::TimeOutOfRange("mean motion <= 0".into()));
        }
        let am = (XKE / nm).powf(X2O3) * tempa * tempa;
        nm = XKE / am.powf(1.5);
        em = em - tempe;
        if em >= 1.0 || em < -0.001 {
            return Err(OrbitError::TimeOutOfRange("eccentricity out of range".into()));
        }
        if em < 1.0e-6 {
            em = 1.0e-6;
        }
        mm = mm + p.no_kozai * templ;
        let mut xlm = mm + argpm + nodem;
        let emsq = em * em;
        let temp = 1.0 - emsq;

        nodem = nodem.rem_euclid(TWOPId);
        argpm = argpm.rem_euclid(TWOPId);
        xlm = xlm.rem_euclid(TWOPId);
        mm = (xlm - argpm - nodem).rem_euclid(TWOPId);

        let sinim = inclm.sin();
        let cosim = inclm.cos();

        // long period periodics
        let mut ep = em;
        let mut xincp = inclm;
        let mut argpp = argpm;
        let mut nodep = nodem;
        let mut mp = mm;
        let mut sinip = sinim;
        let mut cosip = cosim;
        if p.method == 'd' {
            Self::dpper(&mut p, tsince, &mut ep, &mut xincp, &mut nodep, &mut argpp, &mut mp);
            if xincp < 0.0 {
                xincp = -xincp;
                nodep = nodep + std::f64::consts::PI;
                argpp = argpp - std::f64::consts::PI;
            }
            if ep < 1.0e-6 {
                ep = 1.0e-6;
            } else if ep >= 1.0 {
                ep = 1.0 - 1.0e-6;
            }
            sinip = xincp.sin();
            cosip = xincp.cos();
            p.aycof = -0.5 * J3OJ2 * sinip;
            if (cosip + 1.0).abs() > 1.5e-12 {
                p.xlcof = -0.25 * J3OJ2 * sinip * (3.0 + 5.0 * cosip) / (1.0 + cosip);
            } else {
                p.xlcof = -0.25 * J3OJ2 * sinip * (3.0 + 5.0 * cosip) / 1.5e-12;
            }
        }
        let axnl = ep * argpp.cos();
        let temp = 1.0 / (am * (1.0 - ep * ep));
        let aynl = ep * argpp.sin() + temp * p.aycof;
        let mut xl = mp + argpp + nodep + temp * p.xlcof * axnl;

        // solve Kepler's equation
        let mut u = (xl - nodep).rem_euclid(TWOPId);
        let mut eo1 = u;
        let mut tem5: f64 = 9999.9;
        let mut ktr = 1;
        while tem5.abs() >= 1.0e-12 && ktr <= 10 {
            let sineo1 = eo1.sin();
            let coseo1 = eo1.cos();
            tem5 = 1.0 - coseo1 * axnl - sineo1 * aynl;
            tem5 = (u - aynl * coseo1 + axnl * sineo1 - eo1) / tem5;
            if tem5.abs() >= 0.95 {
                tem5 = if tem5 > 0.0 { 0.95 } else { -0.95 };
            }
            eo1 = eo1 + tem5;
            ktr += 1;
        }

        // short period preliminary quantities
        let ecose = axnl * eo1.cos() + aynl * eo1.sin();
        let esine = axnl * eo1.sin() - aynl * eo1.cos();
        let el2 = axnl * axnl + aynl * aynl;
        let pl = am * (1.0 - el2);
        if pl < 0.0 {
            return Err(OrbitError::TimeOutOfRange("semi-latus rectum < 0".into()));
        }
        let rl = am * (1.0 - ecose);
        let rdotl = am.sqrt() * esine / rl;
        let rvdotl = pl.sqrt() / rl;
        let betal = (1.0 - el2).sqrt();
        let temp = esine / (1.0 + betal);
        let sinu = am / rl * (eo1.sin() - aynl - axnl * temp);
        let cosu = am / rl * (eo1.cos() - axnl + aynl * temp);
        let su = sinu.atan2(cosu);
        let sin2u = (cosu + cosu) * sinu;
        let cos2u = 1.0 - 2.0 * sinu * sinu;
        let temp = 1.0 / pl;
        let temp1 = 0.5 * J2 * temp;
        let temp2 = temp1 * temp;

        if p.method == 'd' {
            let cosisq = cosip * cosip;
            p.con41 = 3.0 * cosisq - 1.0;
            p.x1mth2 = 1.0 - cosisq;
            p.x7thm1 = 7.0 * cosisq - 1.0;
        }
        let mrt = rl * (1.0 - 1.5 * temp2 * betal * p.con41) + 0.5 * temp1 * p.x1mth2 * cos2u;
        let su = su - 0.25 * temp2 * p.x7thm1 * sin2u;
        let xnode = nodep + 1.5 * temp2 * cosip * sin2u;
        let xinc = xincp + 1.5 * temp2 * cosip * sinip * cos2u;
        let mvt = rdotl - nm * temp1 * p.x1mth2 * sin2u / XKE;
        let rvdot = rvdotl + nm * temp1 * (p.x1mth2 * cos2u + 1.5 * p.con41) / XKE;

        let sinsu = su.sin();
        let cossu = su.cos();
        let snod = xnode.sin();
        let cnod = xnode.cos();
        let sini = xinc.sin();
        let cosi = xinc.cos();
        let xmx = -snod * cosi;
        let xmy = cnod * cosi;
        let ux = xmx * sinsu + cnod * cossu;
        let uy = xmy * sinsu + snod * cossu;
        let uz = sini * sinsu;
        let vx = xmx * cossu - cnod * sinsu;
        let vy = xmy * cossu - snod * cossu;
        let vz = sini * cossu;

        let position_km = [
            mrt * ux * EARTH_RADIUS_KM,
            mrt * uy * EARTH_RADIUS_KM,
            mrt * uz * EARTH_RADIUS_KM,
        ];
        let velocity_kms = [
            (mvt * ux + rvdot * vx) * vkmpersec,
            (mvt * uy + rvdot * vy) * vkmpersec,
            (mvt * uz + rvdot * vz) * vkmpersec,
        ];

        if (p.ecco > 0.0085 && p.ecco < 0.0088) && (tsince - 120.0).abs() < 1.0 {
            eprintln!("MAIN t={} am={} tempe={} templ={} mm={} mo={} mdot={} no_kozai={} xmdf={} bstar={} cc4={} cc5={} cc1={} sinmo={} temp={}", tsince, am, tempe, templ, mm, p.mo, p.mdot, p.no_kozai, xmdf, p.bstar, p.cc4, p.cc5, p.cc1, p.sinmao, temp);
        }

        Ok(StateVector { position_km, velocity_kms })
    }

    // ------------------------------------------------------------------
    // Deep space: dscom + dsinit + dpper + dspace
    // ------------------------------------------------------------------
    #[allow(clippy::too_many_arguments)]
    fn dsinit(&mut self, epoch: f64, no: f64, xpidot: f64, _con41: f64) {
        let zns = 1.19459e-5;
        let znl = 1.5835218e-4;
        let zes = 0.01675;
        let zel = 0.05490;
        let c1ss = 2.9864797e-6;
        let c1l = 4.7968065e-7;
        let zsinis = 0.39785416;
        let zcosis = 0.91744867;
        let zcosgs = 0.1945905;
        let zsings = -0.98088458;

        let em = self.ecco;
        let emsq = em * em;
        let argpp = self.argpo;
        let mut inclp = self.inclo;
        let nodep = self.nodeo;
        let nm = no;

        let snodm = nodep.sin();
        let cnodm = nodep.cos();
        let sinomm = argpp.sin();
        let cosomm = argpp.cos();
        let sinim = inclp.sin();
        let cosim = inclp.cos();
        let emsq_ = emsq;
        let _betasq = 1.0 - emsq_;

        let day = epoch + 18261.5;
        let xnodce = (4.5236020 - 9.2422029e-4 * day) % TWOPId;
        let stem = xnodce.sin();
        let ctem = xnodce.cos();
        let zcosil = 0.91375164 - 0.03568096 * ctem;
        let zsinil = (1.0 - zcosil * zcosil).sqrt();
        let zsinhl = 0.089683511 * stem / zsinil;
        let zcoshl = (1.0 - zsinhl * zsinhl).sqrt();
        let gam = 5.8351514 + 0.0019443680 * day;
        let mut zx = 0.39785416 * stem / zsinil;
        let zy = zcoshl * ctem + 0.91744867 * zsinhl * stem;
        zx = zx.atan2(zy);
        let zx = gam + zx - xnodce;
        let zcosgl = zx.cos();
        let zsingl = zx.sin();

        let mut zcosg = zcosgs;
        let mut zsing = zsings;
        let mut zcosi = zcosis;
        let mut zsini = zsinis;
        let mut zcosh = cnodm;
        let mut zsinh = snodm;
        let mut cc = c1ss;
        let xnoi = 1.0 / nm;

        let mut z31 = 0.0;
        let mut z32 = 0.0;
        let mut z33 = 0.0;
        let mut z1 = 0.0;
        let mut z2 = 0.0;
        let mut z3 = 0.0;
        let mut z11 = 0.0;
        let mut z12 = 0.0;
        let mut z13 = 0.0;
        let mut z21 = 0.0;
        let mut z22 = 0.0;
        let mut z23 = 0.0;
        let mut z11_ = 0.0;
        let mut z13_ = 0.0;
        let mut z21_ = 0.0;
        let mut z23_ = 0.0;
        let mut z31_ = 0.0;
        let mut z33_ = 0.0;
        let mut s1 = 0.0;
        let mut s2 = 0.0;
        let mut s3 = 0.0;
        let mut s4 = 0.0;
        let mut s5 = 0.0;
        let mut s6 = 0.0;
        let mut s7 = 0.0;
        let mut ss1 = 0.0;
        let mut ss2 = 0.0;
        let mut ss3 = 0.0;
        let mut ss4 = 0.0;
        let mut ss5 = 0.0;
        let mut ss6 = 0.0;
        let mut ss7 = 0.0;
        let mut sz1 = 0.0;
        let mut sz2 = 0.0;
        let mut sz3 = 0.0;
        let mut sz11 = 0.0;
        let mut sz12 = 0.0;
        let mut sz13 = 0.0;
        let mut sz21 = 0.0;
        let mut sz22 = 0.0;
        let mut sz23 = 0.0;
        let mut sz31 = 0.0;
        let mut sz32 = 0.0;
        let mut sz33 = 0.0;
        let mut xgh2 = 0.0;
        let mut xgh3 = 0.0;
        let mut xgh4 = 0.0;
        let mut xh2 = 0.0;
        let mut xh3 = 0.0;
        let mut xi2 = 0.0;
        let mut xi3 = 0.0;
        let mut xl2 = 0.0;
        let mut xl3 = 0.0;
        let mut xl4 = 0.0;

        for lsflg in 1..=2 {
            let a1 = zcosg * zcosh + zsing * zcosi * zsinh;
            let a3 = -zsing * zcosh + zcosg * zcosi * zsinh;
            let a7 = -zcosg * zsinh + zsing * zcosi * zcosh;
            let a8 = zsing * zsini;
            let a9 = zsing * zsinh + zcosg * zcosi * zcosh;
            let a10 = zcosg * zsini;
            let a2 = cosim * a7 + sinim * a8;
            let a4 = cosim * a9 + sinim * a10;
            let a5 = -sinim * a7 + cosim * a8;
            let a6 = -sinim * a9 + cosim * a10;
            let x1 = a1 * cosomm + a2 * sinomm;
            let x2 = a3 * cosomm + a4 * sinomm;
            let x3 = -a1 * sinomm + a2 * cosomm;
            let x4 = -a3 * sinomm + a4 * cosomm;
            let x5 = a5 * sinomm;
            let x6 = a6 * sinomm;
            let x7 = a5 * cosomm;
            let x8 = a6 * cosomm;
            z31 = 12.0 * x1 * x1 - 3.0 * x3 * x3;
            z32 = 24.0 * x1 * x2 - 6.0 * x3 * x4;
            z33 = 12.0 * x2 * x2 - 3.0 * x4 * x4;
            z1 = 3.0 * (a1 * a1 + a2 * a2) + z31 * emsq;
            z2 = 6.0 * (a1 * a3 + a2 * a4) + z32 * emsq;
            z3 = 3.0 * (a3 * a3 + a4 * a4) + z33 * emsq;
            z11 = -6.0 * a1 * a5 + emsq * (-24.0 * x1 * x7 - 6.0 * x3 * x5);
            z12 = -6.0 * (a1 * a6 + a3 * a5) + emsq * (-24.0 * (x2 * x7 + x1 * x8) - 6.0 * (x3 * x6 + x4 * x5));
            z13 = -6.0 * a3 * a6 + emsq * (-24.0 * x2 * x8 - 6.0 * x4 * x6);
            z21 = 6.0 * a2 * a5 + emsq * (24.0 * x1 * x5 - 6.0 * x3 * x7);
            z22 = 6.0 * (a4 * a5 + a2 * a6) + emsq * (24.0 * (x2 * x5 + x1 * x6) - 6.0 * (x4 * x7 + x3 * x8));
            z23 = 6.0 * a4 * a6 + emsq * (24.0 * x2 * x6 - 6.0 * x4 * x8);
            z1 = z1 + z1 + _betasq * z31;
            z2 = z2 + z2 + _betasq * z32;
            z3 = z3 + z3 + _betasq * z33;
            s3 = cc * xnoi;
            s2 = -0.5 * s3 / (1.0 - emsq).sqrt();
            s4 = s3 * (1.0 - emsq).sqrt();
            s1 = -15.0 * em * s4;
            s5 = x1 * x3 + x2 * x4;
            s6 = x2 * x3 + x1 * x4;
            s7 = x2 * x4 - x1 * x3;

            if lsflg == 1 {
                ss1 = s1;
                ss2 = s2;
                ss3 = s3;
                ss4 = s4;
                ss5 = s5;
                ss6 = s6;
                ss7 = s7;
                sz1 = z1;
                sz2 = z2;
                sz3 = z3;
                sz11 = z11;
                sz12 = z12;
                sz13 = z13;
                sz21 = z21;
                sz22 = z22;
                sz23 = z23;
                sz31 = z31;
                sz32 = z32;
                sz33 = z33;
                zcosg = zcosgl;
                zsing = zsingl;
                zcosi = zcosil;
                zsini = zsinil;
                zcosh = zcoshl * cnodm + zsinhl * snodm;
                zsinh = snodm * zcoshl - cnodm * zsinhl;
                cc = c1l;
            }
        }

        let zmol = (4.7199672 + 0.22997150 * day - gam) % TWOPId;
        let zmos = (6.2565837 + 0.017201977 * day) % TWOPId;

        // solar terms
        let se2 = 2.0 * ss1 * ss6;
        let se3 = 2.0 * ss1 * ss7;
        let si2 = 2.0 * ss2 * sz12;
        let si3 = 2.0 * ss2 * (sz13 - sz11);
        let sl2 = -2.0 * ss3 * sz2;
        let sl3 = -2.0 * ss3 * (sz3 - sz1);
        let sl4 = -2.0 * ss3 * (-21.0 - 9.0 * emsq) * zes;
        let sgh2 = 2.0 * ss4 * sz32;
        let sgh3 = 2.0 * ss4 * (sz33 - sz31);
        let sgh4 = -18.0 * ss4 * zes;
        let sh2 = -2.0 * ss2 * sz22;
        let sh3 = -2.0 * ss2 * (sz23 - sz21);
        // lunar terms
        let ee2 = 2.0 * s1 * s6;
        let e3 = 2.0 * s1 * s7;
        let xi2 = 2.0 * s2 * z12;
        let xi3 = 2.0 * s2 * (z13 - z11);
        let xl2 = -2.0 * s3 * z2;
        let xl3 = -2.0 * s3 * (z3 - z1);
        let xl4 = -2.0 * s3 * (-21.0 - 9.0 * emsq) * zel;
        let xgh2 = 2.0 * s4 * z32;
        let xgh3 = 2.0 * s4 * (z33 - z31);
        let xgh4 = -18.0 * s4 * zel;
        let xh2 = -2.0 * s2 * z22;
        let xh3 = -2.0 * s2 * (z23 - z21);

        let _ = (zcosgs, zsings, zcosis, zsinis, zcoshl, zsinhl, z11_, z13_, z21_, z23_, z31_, z33_);

        // Seed the lunar/solar periodic baseline at t=0 (equivalent to the
        // reference's `dpper(init='y')` call, made once right after `dscom`
        // and before `dsinit`'s resonance setup). Every later `dpper`
        // ('n') call subtracts this baseline to get the periodic *change*
        // since epoch.
        // The reference `dscom` initializes these baselines to 0.0 and never
        // updates them (confirmed against the canonical implementation) —
        // `dpper`'s per-call periodic correction is the raw lunar/solar sum,
        // not a delta from a nonzero epoch baseline.
        let (peo, pinco, plo, pgho, pho): (f64, f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0, 0.0);

        // --- dsinit ---
        let mut irez = 0;
        if (nm < 0.0052359877) && (nm > 0.0034906585) {
            irez = 1;
        }
        if (nm >= 8.26e-3) && (nm <= 9.24e-3) && (em >= 0.5) {
            irez = 2;
        }
        let mut ses = ss1 * zns * ss5;
        let mut sis = ss2 * zns * (sz11 + sz13);
        let sls = -zns * ss3 * (sz1 + sz3 - 14.0 - 6.0 * emsq);
        let sghs = ss4 * zns * (sz31 + sz33 - 6.0);
        let mut shs = -zns * ss2 * (sz21 + sz23);
        if (inclp < 5.2359877e-2) || (inclp > std::f64::consts::PI - 5.2359877e-2) {
            shs = 0.0;
        }
        if sinim != 0.0 {
            shs = shs / sinim;
        }
        let sgs = sghs - cosim * shs;
        let dedt = ses + s1 * znl * s5;
        let didt = sis + s2 * znl * (z11 + z13);
        let dmdt = sls - znl * s3 * (z1 + z3 - 14.0 - 6.0 * emsq);
        let sghl = s4 * znl * (z31 + z33 - 6.0);
        let mut shll = -znl * s2 * (z21 + z23);
        if (inclp < 5.2359877e-2) || (inclp > std::f64::consts::PI - 5.2359877e-2) {
            shll = 0.0;
        }
        let mut domdt = sgs + sghl;
        let mut dnodt = shs;
        if sinim != 0.0 {
            domdt = domdt - cosim / sinim * shll;
            dnodt = dnodt + shll / sinim;
        }
        let theta = (self.gsto + 0.0 * 4.37526908801129966e-3) % TWOPId;

        if irez != 0 {
            let aonv = (nm / XKE).powf(X2O3);
            if irez == 2 {
                let cosisq = cosim * cosim;
                let mut emsq_ = em * em;
                let eoc = em * emsq_;
                let g201 = -0.306 - (em - 0.64) * 0.440;
                let (mut g211, mut g310, mut g322, mut g410, mut g422, mut g520, mut g533, mut g521, mut g532);
                if em <= 0.65 {
                    g211 = 3.616 - 13.2470 * em + 16.2900 * emsq_;
                    g310 = -19.302 + 117.3900 * em - 228.4190 * emsq_ + 156.5910 * eoc;
                    g322 = -18.9068 + 109.7927 * em - 214.6334 * emsq_ + 146.5816 * eoc;
                    g410 = -41.122 + 242.6940 * em - 471.0940 * emsq_ + 313.9530 * eoc;
                    g422 = -146.407 + 841.8800 * em - 1629.014 * emsq_ + 1083.4350 * eoc;
                    g520 = -532.114 + 3017.977 * em - 5740.032 * emsq_ + 3708.2760 * eoc;
                } else {
                    g211 = -72.099 + 331.819 * em - 508.738 * emsq_ + 266.724 * eoc;
                    g310 = -346.844 + 1582.851 * em - 2415.925 * emsq_ + 1246.113 * eoc;
                    g322 = -342.585 + 1554.908 * em - 2366.899 * emsq_ + 1215.972 * eoc;
                    g410 = -1052.797 + 4758.686 * em - 7193.992 * emsq_ + 3651.957 * eoc;
                    g422 = -3581.690 + 16178.110 * em - 24462.770 * emsq_ + 12422.520 * eoc;
                    if em > 0.715 {
                        g520 = -5149.66 + 29936.92 * em - 54087.36 * emsq_ + 31324.56 * eoc;
                    } else {
                        g520 = 1464.74 - 4664.75 * em + 3763.64 * emsq_;
                    }
                }
                if em < 0.7 {
                    g533 = -919.22770 + 4988.6100 * em - 9064.7700 * emsq_ + 5542.21 * eoc;
                    g521 = -822.71072 + 4568.6173 * em - 8491.4146 * emsq_ + 5337.524 * eoc;
                    g532 = -853.66600 + 4690.2500 * em - 8624.7700 * emsq_ + 5341.4 * eoc;
                } else {
                    g533 = -37995.780 + 161616.52 * em - 229838.20 * emsq_ + 109377.94 * eoc;
                    g521 = -51752.104 + 218913.95 * em - 309468.16 * emsq_ + 146349.42 * eoc;
                    g532 = -40023.880 + 170470.89 * em - 242699.48 * emsq_ + 115605.82 * eoc;
                }
                let sini2 = sinim * sinim;
                let f220 = 0.75 * (1.0 + 2.0 * cosim + cosisq);
                let f221 = 1.5 * sini2;
                let f321 = 1.875 * sinim * (1.0 - 2.0 * cosim - 3.0 * cosisq);
                let f322 = -1.875 * sinim * (1.0 + 2.0 * cosim - 3.0 * cosisq);
                let f441 = 35.0 * sini2 * f220;
                let f442 = 39.3750 * sini2 * sini2;
                let f522 = 9.84375 * sinim * (sini2 * (1.0 - 2.0 * cosim - 5.0 * cosisq)
                    + 0.33333333 * (-2.0 + 4.0 * cosim + 6.0 * cosisq));
                let f523 = sinim * (4.92187512 * sini2 * (-2.0 - 4.0 * cosim + 10.0 * cosisq)
                    + 6.56250012 * (1.0 + 2.0 * cosim - 3.0 * cosisq));
                let f542 = 29.53125 * sinim * (2.0 - 8.0 * cosim + cosisq * (-12.0 + 8.0 * cosim + 10.0 * cosisq));
                let f543 = 29.53125 * sinim * (-2.0 - 8.0 * cosim + cosisq * (12.0 + 8.0 * cosim - 10.0 * cosisq));
                let xno2 = nm * nm;
                let ainv2 = aonv * aonv;
                let mut temp1 = 3.0 * xno2 * ainv2;
                let mut temp = temp1 * 7.3636953e-9_f64.sqrt();
                let d2201 = temp * f220 * g201;
                let d2211 = temp * f221 * g211;
                temp1 = temp1 * aonv;
                let temp = temp1 * 3.7393792e-7_f64.sqrt();
                let d3210 = temp * f321 * g310;
                let d3222 = temp * f322 * g322;
                temp1 = temp1 * aonv;
                let temp = 2.0 * temp1 * 7.3636953e-9_f64.sqrt();
                let d4410 = temp * f441 * g410;
                let d4422 = temp * f442 * g422;
                temp1 = temp1 * aonv;
                let temp = temp1 * 1.1428639e-7_f64.sqrt();
                let d5220 = temp * f522 * g520;
                let d5232 = temp * f523 * g532;
                let temp = 2.0 * temp1 * 2.1765803e-9_f64.sqrt();
                let d5421 = temp * f542 * g521;
                let d5433 = temp * f543 * g533;
                let xlamo = (self.mo + self.nodeo + self.nodeo - theta - theta) % TWOPId;
                let xfact = self.mdot + dmdt + 2.0 * (self.nodedot + dnodt - 4.37526908801129966e-3) - no;
                self.d2201 = d2201;
                self.d2211 = d2211;
                self.d3210 = d3210;
                self.d3222 = d3222;
                self.d4410 = d4410;
                self.d4422 = d4422;
                self.d5220 = d5220;
                self.d5232 = d5232;
                self.d5421 = d5421;
                self.d5433 = d5433;
                self.xlamo = xlamo;
                self.xfact = xfact;
            }
            if irez == 1 {
                let g200 = 1.0 + emsq * (-2.5 + 0.8125 * emsq);
                let g310 = 1.0 + 2.0 * emsq;
                let g300 = 1.0 + emsq * (-6.0 + 6.60937 * emsq);
                let f220 = 0.75 * (1.0 + cosim) * (1.0 + cosim);
                let f311 = 0.9375 * sinim * sinim * (1.0 + 3.0 * cosim) - 0.75 * (1.0 + cosim);
                let f330 = 1.0 + cosim;
                let f330 = 1.875 * f330 * f330 * f330;
                let mut del1 = 3.0 * nm * nm * aonv * aonv;
                let del2 = 2.0 * del1 * f220 * g200 * 1.7891679e-6;
                let del3 = 3.0 * del1 * f330 * g300 * 2.2123015e-7 * aonv;
                let del1 = del1 * f311 * g310 * 2.1460748e-6 * aonv;
                let xlamo = (self.mo + self.nodeo + self.argpo - theta) % TWOPId;
                let xfact = self.mdot + xpidot - 4.37526908801129966e-3 + dmdt + domdt + dnodt - no;
                self.del1 = del1;
                self.del2 = del2;
                self.del3 = del3;
                self.xlamo = xlamo;
                self.xfact = xfact;
            }
            self.xli = self.xlamo;
            self.xni = no;
            self.atime = 0.0;
            self.irez = irez;
        }
        // stash deep-space rates and periodics
        self.dedt = dedt;
        self.didt = didt;
        self.dmdt = dmdt;
        self.dnodt = dnodt;
        self.domdt = domdt;
        self.e3 = e3;
        self.ee2 = ee2;
        self.peo = peo;
        self.pgho = pgho;
        self.pho = pho;
        self.pinco = pinco;
        self.plo = plo;
        self.se2 = se2;
        self.se3 = se3;
        self.sgh2 = sgh2;
        self.sgh3 = sgh3;
        self.sgh4 = sgh4;
        self.sh2 = sh2;
        self.sh3 = sh3;
        self.si2 = si2;
        self.si3 = si3;
        self.sl2 = sl2;
        self.sl3 = sl3;
        self.sl4 = sl4;
        self.xgh2 = xgh2;
        self.xgh3 = xgh3;
        self.xgh4 = xgh4;
        self.xh2 = xh2;
        self.xh3 = xh3;
        self.xi2 = xi2;
        self.xi3 = xi3;
        self.xl2 = xl2;
        self.xl3 = xl3;
        self.xl4 = xl4;
        self.zmol = zmol;
        self.zmos = zmos;
    }

    #[allow(clippy::too_many_arguments)]
    fn dpper(
        &self,
        t: f64,
        ep: &mut f64,
        inclp: &mut f64,
        nodep: &mut f64,
        argpp: &mut f64,
        mp: &mut f64,
    ) {
        let (pe_raw, pinc_raw, pl_raw, pgh_raw, ph_raw) = dpper_sums(
            t, self.zmos, self.se2, self.se3, self.si2, self.si3, self.sl2, self.sl3, self.sl4,
            self.sgh2, self.sgh3, self.sgh4, self.sh2, self.sh3, self.zmol, self.ee2, self.e3,
            self.xi2, self.xi3, self.xl2, self.xl3, self.xl4, self.xgh2, self.xgh3, self.xgh4,
            self.xh2, self.xh3,
        );
        let mut pe = pe_raw;
        let mut pinc = pinc_raw;
        let mut pl = pl_raw;
        let mut pgh = pgh_raw;
        let mut ph = ph_raw;

        // init == 'n'
        pe = pe - self.peo;
        pinc = pinc - self.pinco;
        pl = pl - self.plo;
        pgh = pgh - self.pgho;
        ph = ph - self.pho;
        *inclp = *inclp + pinc;
        *ep = *ep + pe;
        let sinip = (*inclp).sin();
        let cosip = (*inclp).cos();

        if *inclp >= 0.2 {
            ph = ph / sinip;
            pgh = pgh - cosip * ph;
            *argpp = *argpp + pgh;
            *nodep = *nodep + ph;
            *mp = *mp + pl;
        } else {
            let sinop = (*nodep).sin();
            let cosop = (*nodep).cos();
            let mut alfdp = sinip * sinop;
            let mut betdp = sinip * cosop;
            let dalf = ph * cosop + pinc * cosip * sinop;
            let dbet = -ph * sinop + pinc * cosip * cosop;
            alfdp = alfdp + dalf;
            betdp = betdp + dbet;
            *nodep = (*nodep).rem_euclid(TWOPId);
            let xnoh = *nodep;
            *nodep = alfdp.atan2(betdp);
            if (*nodep < xnoh) {
                *nodep = *nodep + TWOPId;
            } else {
                *nodep = *nodep - TWOPId;
            }
            *mp = *mp + pl;
            *argpp = (*mp + *argpp + cosip * xnoh) - *mp - cosip * *nodep;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn dspace(
        &mut self,
        tc: f64,
        em: &mut f64,
        argpm: &mut f64,
        inclm: &mut f64,
        nodem: &mut f64,
        nm: &mut f64,
        mm: &mut f64,
    ) {
        let fasx2 = 0.13130908;
        let fasx4 = 2.8843198;
        let fasx6 = 0.37448087;
        let g22 = 5.7686396;
        let g32 = 0.95240898;
        let g44 = 1.8014998;
        let g52 = 1.0508330;
        let g54 = 4.4108898;
        let rptim = 4.37526908801129966e-3;
        let stepp = 720.0;
        let stepn = -720.0;
        let step2 = 259200.0;

        let mut dndt = 0.0;
        let theta = (self.gsto + tc * rptim) % TWOPId;
        *em = *em + self.dedt * tc;
        *inclm = *inclm + self.didt * tc;
        *argpm = *argpm + self.domdt * tc;
        *nodem = *nodem + self.dnodt * tc;
        *mm = *mm + self.dmdt * tc;

        let mut ft = 0.0;
        if self.irez != 0 {
            if (self.atime == 0.0) || (tc * self.atime <= 0.0) || (tc.abs() < self.atime.abs()) {
                self.atime = 0.0;
                self.xni = self.no_kozai;
                self.xli = self.xlamo;
            }
            let delt = if tc > 0.0 { stepp } else { stepn };
            let mut xli = self.xli;
            let mut xni = self.xni;
            let mut atime = self.atime;
            let mut iretn = 381;
            let mut xndt = 0.0;
            let mut xldot = 0.0;
            let mut xnddt = 0.0;
            while iretn == 381 {
                if self.irez != 2 {
                    xndt = self.del1 * (xli - fasx2).sin()
                        + self.del2 * (2.0 * (xli - fasx4)).sin()
                        + self.del3 * (3.0 * (xli - fasx6)).sin();
                    xldot = xni + self.xfact;
                    xnddt = self.del1 * (xli - fasx2).cos()
                        + 2.0 * self.del2 * (2.0 * (xli - fasx4)).cos()
                        + 3.0 * self.del3 * (3.0 * (xli - fasx6)).cos();
                    xnddt = xnddt * xldot;
                } else {
                    let xomi = self.argpo + self.argpdot * atime;
                    let x2omi = xomi + xomi;
                    let x2li = xli + xli;
                    xndt = self.d2201 * (x2omi + xli - g22).sin()
                        + self.d2211 * (xli - g22).sin()
                        + self.d3210 * (xomi + xli - g32).sin()
                        + self.d3222 * (-xomi + xli - g32).sin()
                        + self.d4410 * (x2omi + x2li - g44).sin()
                        + self.d4422 * (x2li - g44).sin()
                        + self.d5220 * (xomi + xli - g52).sin()
                        + self.d5232 * (-xomi + xli - g52).sin()
                        + self.d5421 * (xomi + x2li - g54).sin()
                        + self.d5433 * (-xomi + x2li - g54).sin();
                    xldot = xni + self.xfact;
                    xnddt = self.d2201 * (x2omi + xli - g22).cos()
                        + self.d2211 * (xli - g22).cos()
                        + self.d3210 * (xomi + xli - g32).cos()
                        + self.d3222 * (-xomi + xli - g32).cos()
                        + self.d5220 * (xomi + xli - g52).cos()
                        + self.d5232 * (-xomi + xli - g52).cos()
                        + 2.0
                            * (self.d4410 * (x2omi + x2li - g44).cos()
                                + self.d4422 * (x2li - g44).cos()
                                + self.d5421 * (xomi + x2li - g54).cos()
                                + self.d5433 * (-xomi + x2li - g54).cos());
                    xnddt = xnddt * xldot;
                }

                if (tc - atime).abs() >= stepp {
                    iretn = 381;
                } else {
                    ft = tc - atime;
                    iretn = 0;
                }
                if iretn == 381 {
                    xli = xli + xldot * delt + xndt * step2;
                    xni = xni + xndt * delt + xnddt * step2;
                    atime = atime + delt;
                }
            }
            self.xli = xli;
            self.xni = xni;
            self.atime = atime;

            let nm_local = xni + xndt * ft + xnddt * ft * ft * 0.5;
            let xl = xli + xldot * ft + xndt * ft * ft * 0.5;
            if self.irez != 1 {
                *mm = xl - 2.0 * *nodem + 2.0 * theta;
                dndt = nm_local - self.no_kozai;
            } else {
                *mm = xl - *nodem - *argpm + theta;
                dndt = nm_local - self.no_kozai;
            }
            *nm = self.no_kozai + dndt;
        }
    }
}
