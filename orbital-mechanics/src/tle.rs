// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Two-line element set (TLE) parsing and orbital element storage.
//!
//! Parses the standard 69-character TLE format used by NORAD / Space Track.
//! Reference: <https://celestrak.org/NORAD/documentation/tle-fmt.php>

use crate::error::{OrbitError, Result};

/// A parsed two-line element set.
#[derive(Debug, Clone, PartialEq)]
pub struct Tle {
    /// Satellite name (from the optional line 0), if present.
    pub name: Option<String>,
    /// NORAD catalog number.
    pub satellite_number: u32,
    /// International designator, e.g. "26001A".
    pub international_designator: String,
    /// Epoch: fractional year (e.g. 2026.123...).
    pub epoch_year: u32,
    pub epoch_day: f64,
    /// First time derivative of mean motion (ballistic coefficient), rev/day^2.
    pub mean_motion_dot: f64,
    /// Second time derivative of mean motion, rev/day^3.
    pub mean_motion_ddot: f64,
    /// BSTAR drag term.
    pub bstar: f64,
    /// Orbital element set number.
    pub element_set_number: u32,
    /// Inclination, degrees.
    pub inclination_deg: f64,
    /// Right ascension of ascending node, degrees.
    pub raan_deg: f64,
    /// Eccentricity (no decimal point in TLE; leading 0 omitted).
    pub eccentricity: f64,
    /// Argument of perigee, degrees.
    pub arg_perigee_deg: f64,
    /// Mean anomaly, degrees.
    pub mean_anomaly_deg: f64,
    /// Mean motion, revolutions per day.
    pub mean_motion: f64,
    /// Revolution number at epoch.
    pub rev_number_at_epoch: u32,
}

impl Tle {
    /// Parse a TLE from a single string containing 2 or 3 lines.
    ///
    /// If 3 lines are supplied, the first is treated as the satellite name.
    pub fn parse(input: &str) -> Result<Self> {
        let lines: Vec<&str> = input.lines().map(|l| l.trim_end()).collect();
        match lines.len() {
            2 => Tle::parse_lines(None, lines[0], lines[1]),
            3 => Tle::parse_lines(Some(lines[0].to_string()), lines[1], lines[2]),
            n => Err(OrbitError::InvalidLineCount(n)),
        }
    }

    /// Parse from explicit line 1 and line 2 (and optional name).
    pub fn parse_lines(name: Option<String>, line1: &str, line2: &str) -> Result<Self> {
        // Standard TLE lines are 69 chars, but verification TLEs append extra
        // start/stop/step fields. We accept >= 69 and use the first 69 chars.
        if line1.len() < 69 {
            return Err(OrbitError::InvalidLineLength(line1.len()));
        }
        if line2.len() < 69 {
            return Err(OrbitError::InvalidLineLength(line2.len()));
        }
        let line1 = &line1[..69];
        let line2 = &line2[..69];
        Tle::check_checksum(line1)?;
        Tle::check_checksum(line2)?;

        let satellite_number = Tle::u32(line1, 2..7, "satellite_number")?;
        let international_designator = line1[9..17].trim().to_string();
        let (epoch_year, epoch_day) = Tle::parse_epoch(&line1[18..32])?;
        let mean_motion_dot = Tle::parse_decimal_field(&line1[33..43], "mean_motion_dot")?;
        let mean_motion_ddot = Tle::parse_exp_field(&line1[44..52], "mean_motion_ddot")?;
        let bstar = Tle::parse_exp_field(&line1[53..61], "bstar")?;
        let element_set_number = Tle::u32(line1, 64..68, "element_set_number")?;

        let inclination_deg = Tle::f64(line2, 8..16, "inclination")?;
        let raan_deg = Tle::f64(line2, 17..25, "raan")?;
        let eccentricity = Tle::parse_eccentricity(&line2[26..33])?;
        let arg_perigee_deg = Tle::f64(line2, 34..42, "arg_perigee")?;
        let mean_anomaly_deg = Tle::f64(line2, 43..51, "mean_anomaly")?;
        let mean_motion = Tle::f64(line2, 52..63, "mean_motion")?;
        let rev_number_at_epoch = Tle::u32(line2, 63..68, "rev_number_at_epoch")?;

        Ok(Tle {
            name,
            satellite_number,
            international_designator,
            epoch_year,
            epoch_day,
            mean_motion_dot,
            mean_motion_ddot,
            bstar,
            element_set_number,
            inclination_deg,
            raan_deg,
            eccentricity,
            arg_perigee_deg,
            mean_anomaly_deg,
            mean_motion,
            rev_number_at_epoch,
        })
    }

    fn check_checksum(line: &str) -> Result<()> {
        let mut sum: u32 = 0;
        for ch in line.bytes().take(68) {
            match ch {
                b'0'..=b'9' => sum += (ch - b'0') as u32,
                b'-' => sum += 1,
                _ => {}
            }
        }
        let expected = (line.as_bytes()[68] - b'0') as u8;
        let computed = (sum % 10) as u8;
        if computed != expected {
            Err(OrbitError::ChecksumMismatch(computed, expected))
        } else {
            Ok(())
        }
    }

    fn parse_epoch(s: &str) -> Result<(u32, f64)> {
        let s = s.trim();
        let (y_str, d_str) = s.split_at(2);
        let yy: u32 = y_str
            .trim()
            .parse()
            .map_err(|_| OrbitError::InvalidEpoch(s.to_string()))?;
        let year = if yy < 57 { 2000 + yy } else { 1900 + yy };
        let day: f64 = d_str
            .trim()
            .parse()
            .map_err(|_| OrbitError::InvalidEpoch(s.to_string()))?;
        Ok((year, day))
    }

    fn parse_eccentricity(s: &str) -> Result<f64> {
        let s = s.trim();
        let v: f64 = s
            .parse()
            .map_err(|_| OrbitError::ParseField { field: "eccentricity", value: s.to_string() })?;
        Ok(v * 1e-7)
    }

    /// Parse an exponential TLE field. TLE columns for `mean_motion_ddot` and
    /// `bstar` omit the leading `0.`: e.g. " 12345-6" means 0.12345e-6 and
    /// " 00000-0" means 0.0. We insert the implicit decimal point before
    /// parsing.
    fn parse_exp_field(s: &str, field: &'static str) -> Result<f64> {
        let s = s.trim();
        if s.is_empty() || s.chars().all(|c| c == '0' || c == ' ' || c == '-' || c == '+') {
            return Ok(0.0);
        }
        let sign = if s.starts_with('-') { "-" } else { "" };
        let digits = s.trim_start_matches(['+', '-']).trim();
        // Find the exponent marker '+' or '-' that begins the exponent part
        // (the last sign in the string, introduced by e.g. "-6").
        let exp_idx = digits.rfind(['+', '-']);
        let (mantissa, exp) = match exp_idx {
            Some(i) => {
                let m = &digits[..i];
                let e = &digits[i..];
                (m, e)
            }
            None => (digits, ""),
        };
        let combined = format!("{sign}0.{mantissa}e{exp}");
        let v: f64 = combined
            .parse()
            .map_err(|_| OrbitError::ParseField { field, value: s.to_string() })?;
        Ok(v)
    }

    fn parse_decimal_field(s: &str, field: &'static str) -> Result<f64> {
        let s = s.trim();
        let v: f64 = s
            .parse()
            .map_err(|_| OrbitError::ParseField { field, value: s.to_string() })?;
        Ok(v)
    }

    fn u32(s: &str, range: std::ops::Range<usize>, field: &'static str) -> Result<u32> {
        let v = s[range]
            .trim()
            .parse()
            .map_err(|_| OrbitError::ParseField { field, value: s.to_string() })?;
        Ok(v)
    }

    fn f64(s: &str, range: std::ops::Range<usize>, field: &'static str) -> Result<f64> {
        let v = s[range]
            .trim()
            .parse()
            .map_err(|_| OrbitError::ParseField { field, value: s.to_string() })?;
        Ok(v)
    }

    /// Epoch as a modified Julian date (MJD, days since 1858-11-17).
    pub fn epoch_mjd(&self) -> f64 {
        let a = if self.epoch_year < 1900 { 0 } else { 0 };
        let _ = a;
        // Fractional day -> MJD via day-of-year.
        // MJD at start of year = 365.25*(year-1858)-0.5 approx; use precise formula.
        let mjd_year_start = mjd_at_year_start(self.epoch_year);
        mjd_year_start + (self.epoch_day - 1.0)
    }
}

fn is_leap(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Modified Julian Date at the start (Jan 0.0 / Dec 31 of prior year) of `year`.
fn mjd_at_year_start(year: u32) -> f64 {
    // MJD of 2000-01-01T00:00 = 51544.0
    let days_from_2000 = if year >= 2000 {
        let mut d = 0i64;
        for y in 2000..year {
            d += if is_leap(y) { 366 } else { 365 };
        }
        d
    } else {
        let mut d = 0i64;
        for y in year..2000 {
            d -= if is_leap(y) { 366 } else { 365 };
        }
        d
    };
    // 2000-01-01 is day 1 of 2000; the "year start" (Jan 0.0) is one day earlier.
    (51544.0 - 1.0) + days_from_2000 as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISS: &str = "ISS (ZARYA)\n\
1 25544U 98067A   24015.50000000  .00016717  00000-0  10270-3 0  9004\n\
2 25544  51.6400 208.9163 0007652 360.0000 130.3994 15.49815308 90008";

    #[test]
    fn parses_iss_fields() {
        let tle = Tle::parse(ISS).unwrap();
        assert_eq!(tle.satellite_number, 25544);
        assert_eq!(tle.international_designator, "98067A");
        assert_eq!(tle.epoch_year, 2024);
        assert!((tle.epoch_day - 15.5).abs() < 1e-9);
        assert!((tle.inclination_deg - 51.64).abs() < 1e-6);
        assert!((tle.eccentricity - 0.0007652).abs() < 1e-10);
        assert!((tle.mean_motion - 15.49815308).abs() < 1e-8);
        assert_eq!(tle.name.as_deref(), Some("ISS (ZARYA)"));
    }

    #[test]
    fn rejects_bad_checksum() {
        let mut lines: Vec<&str> = ISS.lines().collect();
        // Corrupt the last checksum digit.
        let bad = lines[2].clone();
        let mut chars: Vec<u8> = bad.bytes().collect();
        chars[68] = if chars[68] == b'0' { b'9' } else { b'0' };
        lines[2] = std::str::from_utf8(&chars).unwrap();
        let joined = lines.join("\n");
        assert!(Tle::parse(&joined).is_err());
    }
}
