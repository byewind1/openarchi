use super::Location;
use serde::{Deserialize, Serialize};
/// UTC Gregorian time only. Local-zone/DST interpretation belongs to the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Utc {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    /// Leap seconds are not accepted or silently normalized.
    pub second: u8,
}
impl Utc {
    pub fn julian_day(self) -> Result<f64, String> {
        let leap = self.year % 4 == 0 && (self.year % 100 != 0 || self.year % 400 == 0);
        let max_day = match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if leap {
                    29
                } else {
                    28
                }
            }
            _ => 0,
        };
        if !(1900..=2100).contains(&self.year)
            || self.day == 0
            || self.day > max_day
            || self.hour > 23
            || self.minute > 59
            || self.second > 59
        {
            return Err("Solar time requires a valid UTC Gregorian date in 1900-2100; leap seconds are unsupported".into());
        }
        let (y, m) = if self.month <= 2 {
            (self.year - 1, self.month as i32 + 12)
        } else {
            (self.year, self.month as i32)
        };
        let a = (y as f64 / 100.).floor();
        let b = 2. - a + (a / 4.).floor();
        Ok((365.25 * (y as f64 + 4716.)).floor()
            + (30.6001 * (m as f64 + 1.)).floor()
            + self.day as f64
            + b
            - 1524.5
            + (f64::from(self.hour) * 3600.
                + f64::from(self.minute) * 60.
                + f64::from(self.second))
                / 86400.)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub julian_day_utc: f64,
    /// Geographic east, north, up. Unit vector toward the geometric sun centre.
    pub east_north_up: [f64; 3],
    pub elevation_deg: f64,
    /// North=0, east=90. None at zenith/nadir, where azimuth has no meaning.
    pub azimuth_deg: Option<f64>,
}
fn rad(deg: f64) -> f64 {
    deg.to_radians()
}
/// Low-order orbital expansion, evaluated from explicit UTC. This is not SPA.
/// Coefficients describe mean longitude/anomaly, equation of centre and obliquity;
/// original Rust implementation of published astronomical equations (see contract).
pub fn position(location: Location, time: Utc) -> Result<Position, String> {
    location.validate()?;
    at_julian(location, time.julian_day()?)
}
pub(super) fn at_julian(location: Location, jd: f64) -> Result<Position, String> {
    location.validate()?;
    if !jd.is_finite() || !(2415020.5..2488434.5).contains(&jd) {
        return Err("Solar Julian day is outside 1900-2100".into());
    }
    let t = (jd - 2451545.) / 36525.;
    let mean_longitude = (280.46646 + t * (36000.76983 + 0.0003032 * t)).rem_euclid(360.);
    let mean_anomaly = rad(357.52911 + t * (35999.05029 - 0.0001537 * t));
    let eccentricity = 0.016708634 - t * (0.000042037 + 0.0000001267 * t);
    let centre = mean_anomaly.sin() * (1.914602 - t * (0.004817 + 0.000014 * t))
        + (2. * mean_anomaly).sin() * (0.019993 - 0.000101 * t)
        + (3. * mean_anomaly).sin() * 0.000289;
    let node = rad(125.04 - 1934.136 * t);
    let longitude = rad(mean_longitude + centre - 0.00569 - 0.00478 * node.sin());
    let obliquity = rad(23.
        + (26. + (21.448 - t * (46.815 + t * (0.00059 - 0.001813 * t))) / 60.) / 60.
        + 0.00256 * node.cos());
    let declination = (obliquity.sin() * longitude.sin()).asin();
    let y = (obliquity / 2.).tan().powi(2);
    let l = rad(mean_longitude);
    let equation_minutes = 4.
        * (y * (2. * l).sin() - 2. * eccentricity * mean_anomaly.sin()
            + 4. * eccentricity * y * mean_anomaly.sin() * (2. * l).cos()
            - 0.5 * y * y * (4. * l).sin()
            - 1.25 * eccentricity * eccentricity * (2. * mean_anomaly).sin())
        .to_degrees();
    let utc_minutes = ((jd + 0.5).fract() * 1440.).rem_euclid(1440.);
    let hour_angle = rad(
        (utc_minutes + equation_minutes + 4. * location.longitude_deg).rem_euclid(1440.) / 4.
            - 180.,
    );
    let lat = rad(location.latitude_deg);
    let (sd, cd) = declination.sin_cos();
    let (sh, ch) = hour_angle.sin_cos();
    let (sl, cl) = lat.sin_cos();
    let e = -cd * sh;
    let n = cl * sd - sl * cd * ch;
    let u = sl * sd + cl * cd * ch;
    let direction = glam::DVec3::new(e, n, u).normalize();
    Ok(Position {
        julian_day_utc: jd,
        east_north_up: direction.to_array(),
        elevation_deg: direction.z.clamp(-1., 1.).asin().to_degrees(),
        azimuth_deg: (e.hypot(n) > 1e-10).then(|| e.atan2(n).to_degrees().rem_euclid(360.)),
    })
}
