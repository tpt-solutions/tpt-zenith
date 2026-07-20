// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deep space: dscom + dsinit + dpper + dspace.

use super::{Propagator, TWOPId, X2O3};
use crate::constants::XKE;

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
    // ------------------------------------------------------------------
    // Deep space: dscom + dsinit + dpper + dspace
    // ------------------------------------------------------------------
    #[allow(clippy::too_many_arguments)]
    pub(super) fn dsinit(&mut self, epoch: f64, no: f64, xpidot: f64, _con41: f64) {
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
    pub(super) fn dpper(
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
    pub(super) fn dspace(
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
