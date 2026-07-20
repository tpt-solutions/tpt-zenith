// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Diagnostic: compare this propagator against tcppver.out per satellite.

use orbital_mechanics::propagator::{Propagator, StateVector};
use orbital_mechanics::tle::Tle;
use std::collections::HashMap;

fn parse_cases(text: &str) -> Vec<(String, String)> {
    let mut cases = Vec::new();
    let mut line1: Option<String> = None;
    for line in text.lines() {
        if line.starts_with('#') { continue; }
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

struct Row { tsince: f64, pos: [f64; 3] }

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
                toks[0].parse::<f64>(), toks[1].parse::<f64>(),
                toks[2].parse::<f64>(), toks[3].parse::<f64>()) {
                let r = (x*x+y*y+z*z).sqrt();
                if tsince.abs() <= 100_000.0 && r <= 100_000.0 {
                    if let Some(b) = blocks.last_mut() {
                        b.push(Row { tsince, pos: [x,y,z] });
                    }
                }
            }
        }
    }
    blocks
}

fn perr(a: &StateVector, e: &[f64;3]) -> f64 {
    ((a.position_km[0]-e[0]).powi(2)+(a.position_km[1]-e[1]).powi(2)+(a.position_km[2]-e[2]).powi(2)).sqrt()
}

fn main() {
    let tle = include_str!("../../tests/fixtures/sgp4/SGP4-VER.TLE");
    let out = include_str!("../../tests/fixtures/sgp4/tcppver.out");
    let cases = parse_cases(tle);
    let blocks = parse_expected(out);
    let detail: std::collections::HashSet<&str> = ["28129","28057","06251","28350","88888"].iter().copied().collect();
    let mut res: HashMap<String,(f64,String)> = HashMap::new();
    for ((l1,l2),rows) in cases.iter().zip(blocks.iter()) {
        let sat = l1[2..7].to_string();
        let t = Tle::parse_lines(None, l1, l2).expect("parse");
        let p = match Propagator::from_tle(&t) { Ok(p)=>p, Err(e)=>{ res.insert(sat,(f64::INFINITY,format!("init_err {e}"))); continue; } };
        let mut worst = 0.0_f64;
        let mut kind = "ok".to_string();
        for r in rows {
            match p.propagate(r.tsince) {
                Ok(s) => {
                    let e = perr(&s, &r.pos);
                    if e>worst { worst = e; kind="cm-fail".to_string(); }
                    if detail.contains(sat.as_str()) && (r.tsince as i64) == 0 {
                        println!("DETAIL {sat} t=0 ref={:?}", r.pos);
                        println!("DETAIL {sat} t=0 got={:?}", s.position_km);
                        println!("DETAIL {sat} t=0 vel={:?}", s.velocity_kms);
                    }
                }
                Err(_) => { kind = "prop_err".to_string(); }
            }
        }
        res.insert(sat, (worst, kind));
    }
    let mut items: Vec<_> = res.into_iter().collect();
    items.sort_by(|a,b| a.0.cmp(&b.0));
    for (sat,(w,k)) in items {
        let tag = if w <= 1.0e-2 { "PASS".to_string() } else { k };
        println!("{sat} {tag:>9} worst={w:.3e} km");
    }
}
