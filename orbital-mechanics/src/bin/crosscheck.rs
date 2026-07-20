// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cross-check: faithful re-implementation of the reference (dnwrnr) near-earth
//! SGP4 using THIS crate's constants, to localize the bug in the main port.

use orbital_mechanics::tle::Tle;

const AE: f64 = 1.0;
const XJ2: f64 = 1.082616e-3;
const XJ3: f64 = -2.53881e-6;
const XJ4: f64 = -1.65597e-6;
const XKMPER: f64 = 6378.137;
const XKE: f64 = 7.43668811206102e-02;
const CK2: f64 = 0.5 * XJ2 * AE * AE;
const CK4: f64 = -0.375 * XJ4 * AE * AE * AE * AE;
const Q0: f64 = 120.0;
const S0: f64 = 78.0;
const QOMS2T: f64 =
    ((Q0 - S0) / XKMPER) * ((Q0 - S0) / XKMPER) * ((Q0 - S0) / XKMPER) * ((Q0 - S0) / XKMPER);
const A3OVK2: f64 = -XJ3 / CK2 * AE * AE * AE;
const X2O3: f64 = 2.0 / 3.0;
const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

struct Sat {
    inclo: f64,
    nodeo: f64,
    ecco: f64,
    argpo: f64,
    mo: f64,
    no: f64, // rad/min
    bstar: f64,
    // initl-derived
    con41: f64,
    x1mth2: f64,
    x7thm1: f64,
    xlcof: f64,
    aycof: f64,
    c1: f64,
    c4: f64,
    mdot: f64,
    omgdot: f64,
    xnodot: f64,
    xnodcf: f64,
    t2cof: f64,
    c5: f64,
    omgcof: f64,
    xmcof: f64,
    delmo: f64,
    sinmo: f64,
    d2: f64,
    d3: f64,
    d4: f64,
    t3cof: f64,
    t4cof: f64,
    t5cof: f64,
    eta: f64,
}

fn init(t: &Tle) -> Sat {
    let n0 = t.mean_motion / 1440.0 * TWO_PI;
    let inclo = t.inclination_deg * std::f64::consts::PI / 180.0;
    let ecco = t.eccentricity;
    let nodeo = t.raan_deg * std::f64::consts::PI / 180.0;
    let argpo = t.arg_perigee_deg * std::f64::consts::PI / 180.0;
    let mo = t.mean_anomaly_deg * std::f64::consts::PI / 180.0;
    let bstar = t.bstar;

    let cosio = inclo.cos();
    let sinio = inclo.sin();
    let theta2 = cosio * cosio;
    let theta4 = theta2 * theta2;
    let x3thm1 = 3.0 * theta2 - 1.0;
    let x1mth2 = 1.0 - theta2;
    let x7thm1 = 7.0 * theta2 - 1.0;
    let xlcof = if (cosio + 1.0).abs() > 1.5e-12 {
        0.125 * A3OVK2 * sinio * (3.0 + 5.0 * cosio) / (1.0 + cosio)
    } else {
        0.125 * A3OVK2 * sinio * (3.0 + 5.0 * cosio) / 1.5e-12
    };
    let aycof = 0.25 * A3OVK2 * sinio;

    let eosq = ecco * ecco;
    let betao2 = 1.0 - eosq;
    let betao = betao2.sqrt();
    let con42 = 1.0 - 5.0 * theta2;
    let con41 = -con42 - theta2 - theta2;

    let a1 = (XKE / n0).powf(X2O3);
    let temp = 1.5 * CK2 * x3thm1 / (betao * betao2);
    let del1 = temp / (a1 * a1);
    let a0 = a1 * (1.0 - del1 * (1.0 / 3.0 + del1 * (1.0 + del1 * 134.0 / 81.0)));
    let del0 = temp / (a0 * a0);
    let n = n0 / (1.0 + del0);
    let a = a0 / (1.0 - del0);

    let perigee = (a * (1.0 - ecco) - AE) * XKMPER;
    let use_simple = perigee < 220.0;

    let mut s4 = S0 / XKMPER + AE;
    let mut qoms24 = QOMS2T;
    if perigee < 156.0 {
        s4 = perigee - 78.0;
        if perigee < 98.0 {
            s4 = 20.0;
        }
        qoms24 = ((120.0 - s4) * XKMPER / XKMPER).powi(4);
        s4 = s4 / XKMPER + AE;
    }

    let pinvsq = 1.0 / (a * a * betao2 * betao2);
    let tsi = 1.0 / (a - s4);
    let eta = a * ecco * tsi;
    let etasq = eta * eta;
    let eeta = ecco * eta;
    let psisq = (1.0 - etasq).abs();
    let coef = qoms24 * tsi.powi(4);
    let coef1 = coef / psisq.powf(3.5);
    let c2 = coef1
        * n
        * (a * (1.0 + 1.5 * etasq + eeta * (4.0 + etasq))
            + 0.75 * CK2 * tsi / psisq * x3thm1 * (8.0 + 3.0 * etasq * (8.0 + etasq)));
    let c1 = bstar * c2;
    let c4 = 2.0
        * n
        * coef1
        * a
        * betao2
        * (eta * (2.0 + 0.5 * etasq) + ecco * (0.5 + 2.0 * etasq)
            - 2.0 * CK2 * tsi / (a * psisq)
                * (-3.0 * x3thm1 * (1.0 - 2.0 * eeta + etasq * (1.5 - 0.5 * eeta))
                    + 0.75 * x1mth2 * (2.0 * etasq - eeta * (1.0 + etasq)) * (2.0 * argpo).cos()));
    let temp1 = 3.0 * CK2 * pinvsq * n;
    let temp2 = temp1 * CK2 * pinvsq;
    let temp3 = 1.25 * CK4 * pinvsq * pinvsq * n;
    let mdot = n
        + 0.5 * temp1 * betao * x3thm1
        + 0.0625 * temp2 * betao * (13.0 - 78.0 * theta2 + 137.0 * theta4);
    let x1m5th = 1.0 - 5.0 * theta2;
    let omgdot = -0.5 * temp1 * x1m5th
        + 0.0625 * temp2 * (7.0 - 114.0 * theta2 + 395.0 * theta4)
        + temp3 * (3.0 - 36.0 * theta2 + 49.0 * theta4);
    let xhdot1 = -temp1 * cosio;
    let xnodot =
        xhdot1 + (0.5 * temp2 * (4.0 - 19.0 * theta2) + 2.0 * temp3 * (3.0 - 7.0 * theta2)) * cosio;
    let xnodcf = 3.5 * betao2 * xhdot1 * c1;
    let t2cof = 1.5 * c1;

    let mut c3 = 0.0;
    if ecco > 1.0e-4 {
        c3 = coef * tsi * A3OVK2 * n * AE * sinio / ecco;
    }
    let c5 = 2.0 * coef1 * a * betao2 * (1.0 + 2.75 * (etasq + eeta) + eeta * etasq);
    let omgcof = bstar * c3 * argpo.cos();
    let mut xmcof = 0.0;
    if ecco > 1.0e-4 {
        xmcof = -X2O3 * coef * bstar * AE / eeta;
    }
    let delmo = (1.0 + eta * mo.cos()).powi(3);
    let sinmo = mo.sin();

    let (d2, d3, d4, t3cof, t4cof, t5cof) = if !use_simple {
        let c1sq = c1 * c1;
        let d2 = 4.0 * a * tsi * c1sq;
        let temp = d2 * tsi * c1 / 3.0;
        let d3 = (17.0 * a + s4) * temp;
        let d4 = 0.5 * temp * a * tsi * (221.0 * a + 31.0 * s4) * c1;
        let t3cof = d2 + 2.0 * c1sq;
        let t4cof = 0.25 * (3.0 * d3 + c1 * (12.0 * d2 + 10.0 * c1sq));
        let t5cof =
            0.2 * (3.0 * d4 + 12.0 * c1 * d3 + 6.0 * d2 * d2 + 15.0 * c1sq * (2.0 * d2 + c1sq));
        (d2, d3, d4, t3cof, t4cof, t5cof)
    } else {
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    };

    if ecco > 0.0085 && ecco < 0.0088 {
        eprintln!(
            "XCINIT n={} mdot={} temp1={} pinvsq={} a={} betao2={}",
            n, mdot, temp1, pinvsq, a, betao2
        );
    }

    Sat {
        inclo,
        nodeo,
        ecco,
        argpo,
        mo,
        no: n,
        bstar,
        con41,
        x1mth2,
        x7thm1,
        xlcof,
        aycof,
        c1,
        c4,
        mdot,
        omgdot,
        xnodot,
        xnodcf,
        t2cof,
        c5,
        omgcof,
        xmcof,
        delmo,
        sinmo,
        d2,
        d3,
        d4,
        t3cof,
        t4cof,
        t5cof,
        eta,
    }
}

fn propagate(s: &Sat, tsince: f64) -> [f64; 3] {
    let xmdf = s.mo + s.mdot * tsince;
    let omgadf = s.argpo + s.omgdot * tsince;
    let xnoddf = s.nodeo + s.xnodot * tsince;
    let mut omega = omgadf;
    let mut xmp = xmdf;
    let tsq = tsince * tsince;
    let xnode = xnoddf + s.xnodcf * tsq;
    let mut tempa = 1.0 - s.c1 * tsince;
    let mut tempe = s.bstar * s.c4 * tsince;
    let mut templ = s.t2cof * tsq;

    let delomg = s.omgcof * tsince;
    let delm = s.xmcof * ((1.0 + s.eta * xmdf.cos()).powi(3) - s.delmo);
    let temp = delomg + delm;
    xmp += temp;
    omega -= temp;
    let tcube = tsq * tsince;
    let tfour = tsince * tcube;
    tempa = tempa - s.d2 * tsq - s.d3 * tcube - s.d4 * tfour;
    tempe += s.bstar * s.c5 * (xmp.sin() - s.sinmo);
    templ += s.t3cof * tcube + tfour * (s.t4cof + tsince * s.t5cof);

    let a = s.no * s.no; // unused placeholder to avoid unused warning; a via sat
    let _ = a;
    let a_km = (XKE / s.no).powf(X2O3) * XKMPER; // not used directly
    let _ = a_km;
    let a_val = (XKE / s.no).powf(X2O3) * tempa * tempa;
    let e = s.ecco - tempe;
    let xl = xmp + omega + xnode + s.no * templ;
    if s.ecco > 0.0085 && s.ecco < 0.0088 {
        eprintln!(
            "XC t={} a_val={} xl={} xmp={} omega={} xnode={} templ={} tempa={} tempe={} e={}",
            tsince, a_val, xl, xmp, omega, xnode, templ, tempa, tempe, e
        );
    }
    let beta2 = 1.0 - e * e;
    let xn = XKE / a_val.powf(1.5);
    let axn = e * omega.cos();
    let temp11 = 1.0 / (a_val * beta2);
    let xll = temp11 * s.xlcof * axn;
    let aynl = temp11 * s.aycof;
    let xlt = xl + xll;
    let ayn = e * omega.sin() + aynl;
    let elsq = axn * axn + ayn * ayn;
    let capu = xlt.rem_euclid(TWO_PI) - xnode;
    let mut epw = capu;
    for _ in 0..10 {
        let sinepw = epw.sin();
        let cosepw = epw.cos();
        let ecose = axn * cosepw + ayn * sinepw;
        let esine = axn * sinepw - ayn * cosepw;
        let f = capu - epw + esine;
        if f.abs() < 1.0e-12 {
            break;
        }
        let fdot = 1.0 - ecose;
        epw += f / fdot;
    }
    let cosepw = epw.cos();
    let sinepw = epw.sin();
    let esine = axn * sinepw - ayn * cosepw;
    let ecose = axn * cosepw + ayn * sinepw;
    let temp21 = 1.0 - elsq;
    let pl = a_val * temp21;
    let r = a_val * (1.0 - ecose);
    let temp31 = 1.0 / r;
    let rdot = XKE * a_val.sqrt() * esine * temp31;
    let rfdot = XKE * pl.sqrt() * temp31;
    let temp32 = a_val * temp31;
    let betal = temp21.sqrt();
    let temp33 = 1.0 / (1.0 + betal);
    let cosu = temp32 * (cosepw - axn + ayn * esine * temp33);
    let sinu = temp32 * (sinepw - ayn - axn * esine * temp33);
    let u = sinu.atan2(cosu);
    let sin2u = 2.0 * sinu * cosu;
    let cos2u = 2.0 * cosu * cosu - 1.0;
    let temp41 = 1.0 / pl;
    let temp42 = CK2 * temp41;
    let temp43 = temp42 * temp41;
    let rk = r * (1.0 - 1.5 * temp43 * betal * s.con41) + 0.5 * temp42 * s.x1mth2 * cos2u;
    let uk = u - 0.25 * temp43 * s.x7thm1 * sin2u;
    let xnodek = xnode + 1.5 * temp43 * s.inclo.cos() * sin2u;
    let xinck = s.inclo + 1.5 * temp43 * s.inclo.cos() * s.inclo.sin() * cos2u;
    let rdotk = rdot - xn * temp42 * s.x1mth2 * sin2u;
    let rfdotk = rfdot + xn * temp42 * (s.x1mth2 * cos2u + 1.5 * s.con41);
    let sinuk = uk.sin();
    let cosuk = uk.cos();
    let sinik = xinck.sin();
    let cosik = xinck.cos();
    let sinnok = xnodek.sin();
    let cosnok = xnodek.cos();
    let xmx = -sinnok * cosik;
    let xmy = cosnok * cosik;
    let ux = xmx * sinuk + cosnok * cosuk;
    let uy = xmy * sinuk + sinnok * cosuk;
    let uz = sinik * sinuk;
    let vx = xmx * cosuk - cosnok * sinuk;
    let vy = xmy * cosuk - sinnok * sinuk;
    let vz = sinik * cosuk;
    let position_km = [rk * ux * XKMPER, rk * uy * XKMPER, rk * uz * XKMPER];
    let _ = (rdotk, rfdotk, vx, vy, vz);
    position_km
}

fn perr(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn main() {
    let tle = include_str!("../../tests/fixtures/sgp4/SGP4-VER.TLE");
    let out = include_str!("../../tests/fixtures/sgp4/tcppver.out");
    let cases = parse_cases(tle);
    let blocks = parse_expected(out);
    for ((l1, l2), rows) in cases.iter().zip(blocks.iter()) {
        let sat = &l1[2..7];
        let t = match Tle::parse_lines(None, l1, l2) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let s = init(&t);
        let mut worst = 0.0_f64;
        for r in rows {
            let got = propagate(&s, r.tsince);
            let e = perr(&got, &r.pos);
            if e > worst {
                worst = e;
            }
            if sat == "88888" {
                println!(
                    "XC88888 t={:.1} ref={:?} got={:?} err={:.3e}",
                    r.tsince, r.pos, got, e
                );
            }
        }
        println!("CROSS {sat} worst_pos_km={:.3e}", worst);
    }
}

fn parse_cases(text: &str) -> Vec<(String, String)> {
    let mut cases = Vec::new();
    let mut line1: Option<String> = None;
    for line in text.lines() {
        if line.starts_with('#') {
            continue;
        }
        if line.starts_with("1 ") {
            line1 = Some(line.to_string());
        } else if line.starts_with("2 ") {
            if let Some(l1) = line1.take() {
                cases.push((l1, line.to_string()));
            }
        }
    }
    cases
}

struct Row {
    tsince: f64,
    pos: [f64; 3],
}
fn parse_expected(text: &str) -> Vec<Vec<Row>> {
    let mut blocks: Vec<Vec<Row>> = Vec::new();
    for line in text.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        if toks.len() == 2 && toks[1] == "xx" {
            blocks.push(Vec::new());
            continue;
        }
        if toks.len() >= 7 {
            if let (Ok(tsince), Ok(x), Ok(y), Ok(z)) = (
                toks[0].parse::<f64>(),
                toks[1].parse::<f64>(),
                toks[2].parse::<f64>(),
                toks[3].parse::<f64>(),
            ) {
                let r = (x * x + y * y + z * z).sqrt();
                if tsince.abs() <= 100_000.0 && r <= 100_000.0 {
                    if let Some(b) = blocks.last_mut() {
                        b.push(Row {
                            tsince,
                            pos: [x, y, z],
                        });
                    }
                }
            }
        }
    }
    blocks
}
