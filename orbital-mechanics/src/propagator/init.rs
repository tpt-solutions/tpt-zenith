// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `sgp4init`: building a [`Propagator`](super::Propagator) from a parsed TLE.

use super::time::{epoch_days_1950, gstime};
use super::{Propagator, TWOPId, X2O3};
use crate::constants::{EARTH_RADIUS_KM, J2, J3OJ2, J4, XKE};
use crate::error::Result;
use crate::tle::Tle;

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
            (
                ao, con42, cosio, cosio2, eccsq, omeosq, posq, rp, rteosq, sinio, gsto,
            )
        };
        p.gsto = gsto;
        // The corrected (Brouwer "unkozai") mean motion computed above only
        // lives inside that block's scope; every secular-rate formula below
        // (cc1/cc4/cc5, mdot, argpdot, nodedot, dsinit's `no`, and the isimp
        // perigee test) must use it, not the raw TLE mean motion.
        let no = p.no_kozai;

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
            let cc2 = coef1
                * no
                * (ao * (1.0 + 1.5 * etasq + eeta * (4.0 + etasq))
                    + 0.375 * J2 * tsi / psisq * p.con41 * (8.0 + 3.0 * etasq * (8.0 + etasq)));
            p.cc1 = p.bstar * cc2;
            let mut cc3 = 0.0;
            if p.ecco > 1.0e-4 {
                cc3 = -2.0 * coef * tsi * J3OJ2 * no * sinio / p.ecco;
            }
            p.x1mth2 = 1.0 - cosio2;
            p.cc4 = 2.0
                * no
                * coef1
                * ao
                * omeosq
                * (p.eta * (2.0 + 0.5 * etasq) + p.ecco * (0.5 + 2.0 * etasq)
                    - J2 * tsi / (ao * psisq)
                        * (-3.0 * p.con41 * (1.0 - 2.0 * eeta + etasq * (1.5 - 0.5 * eeta))
                            + 0.75
                                * p.x1mth2
                                * (2.0 * etasq - eeta * (1.0 + etasq))
                                * (2.0 * p.argpo).cos()));
            p.cc5 = 2.0 * coef1 * ao * omeosq * (1.0 + 2.75 * (etasq + eeta) + eeta * etasq);
            let cosio4 = cosio2 * cosio2;
            let temp1 = 1.5 * J2 * pinvsq * no;
            let temp2 = 0.5 * temp1 * J2 * pinvsq;
            let temp3 = -0.46875 * J4 * pinvsq * pinvsq * no;
            p.mdot = no
                + 0.5 * temp1 * rteosq * p.con41
                + 0.0625 * temp2 * rteosq * (13.0 - 78.0 * cosio2 + 137.0 * cosio4);
            p.argpdot = -0.5 * temp1 * con42
                + 0.0625 * temp2 * (7.0 - 114.0 * cosio2 + 395.0 * cosio4)
                + temp3 * (3.0 - 36.0 * cosio2 + 49.0 * cosio4);
            let xhdot1 = -temp1 * cosio;
            p.nodedot = xhdot1
                + (0.5 * temp2 * (4.0 - 19.0 * cosio2) + 2.0 * temp3 * (3.0 - 7.0 * cosio2))
                    * cosio;
            let xpidot = p.argpdot + p.nodedot;
            p.omgcof = p.bstar * cc3 * p.argpo.cos();
            p.xmcof = 0.0;
            if p.ecco > 1.0e-4 {
                p.xmcof = -X2O3 * coef * p.bstar / eeta;
            }
            p.nodecf = 3.5 * omeosq * xhdot1 * p.cc1;
            p.t2cof = 1.5 * p.cc1;
            (p.xlcof, p.aycof) = super::xlcof_aycof(sinio, cosio);
            p.delmo = (1.0 + p.eta * p.mo.cos()).powi(3);
            p.sinmao = p.mo.sin();
            p.x7thm1 = 7.0 * cosio2 - 1.0;

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
                p.t5cof = 0.2
                    * (3.0 * p.d4
                        + 12.0 * p.cc1 * p.d3
                        + 6.0 * p.d2 * p.d2
                        + 15.0 * cc1sq * (2.0 * p.d2 + cc1sq));
            }
        }

        // Propagate to zero epoch to finalize (matches reference sgp4init).
        p.propagate(0.0)?;
        Ok(p)
    }
}
