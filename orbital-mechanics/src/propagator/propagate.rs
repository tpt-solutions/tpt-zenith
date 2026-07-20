// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `sgp4`: propagating an initialized [`Propagator`](super::Propagator) forward in time.

use super::{Propagator, StateVector, TWOPId, X2O3};
use crate::constants::{EARTH_RADIUS_KM, J2, XKE};
use crate::error::{OrbitError, Result};

impl Propagator {
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

        if p.isimp != 1 {
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
            Self::dspace(
                &mut p, tc, &mut em, &mut argpm, &mut inclm, &mut nodem, &mut nm, &mut mm,
            );
        }

        if nm <= 0.0 {
            return Err(OrbitError::TimeOutOfRange("mean motion <= 0".into()));
        }
        let am = (XKE / nm).powf(X2O3) * tempa * tempa;
        nm = XKE / am.powf(1.5);
        em = em - tempe;
        if em >= 1.0 || em < -0.001 {
            return Err(OrbitError::TimeOutOfRange(
                "eccentricity out of range".into(),
            ));
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
            Self::dpper(
                &mut p, tsince, &mut ep, &mut xincp, &mut nodep, &mut argpp, &mut mp,
            );
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
            (p.xlcof, p.aycof) = super::xlcof_aycof(sinip, cosip);
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
        let vy = xmy * cossu - snod * sinsu;
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

        Ok(StateVector {
            position_km,
            velocity_kms,
        })
    }
}
