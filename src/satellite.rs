use std::fmt::Debug;

use crate::transform;
use anyhow::Result;
use chrono::{DateTime, FixedOffset, TimeZone};
use sgp4::{self, Constants, Elements};

pub struct Satellite {
    pub id: Option<u64>,
    pub tle: Option<Tle>,
    pub elements: Elements,
    pub constants: Constants,
}

impl Debug for Satellite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Satellite")
            .field(
                "name",
                &self
                    .elements
                    .object_name
                    .as_ref()
                    .unwrap_or(&"Unknown".to_string())
                    .trim(),
            )
            .field("epoch", &self.elements.datetime)
            .finish()
    }
}

pub type Tle = [String; 3];

pub struct GeoPrediction {
    pub position: (f64, f64, f64),
    pub speed: f64,
    pub gmst: f64,
}

impl Satellite {
    #[cfg(feature = "web")]
    pub fn from_norad_cat(id: u64) -> Result<Self> {
        let text = reqwest::blocking::get(format!(
            "https://celestrak.org/NORAD/elements/gp.php?CATNR={id}&FORMAT=TLE",
        ))?
        .text()?;

        let tle = text
            .lines()
            .take(3)
            .map(|line| line.to_string())
            .collect::<Vec<String>>()
            .try_into()
            .map_err(|_| anyhow::anyhow!("Failed to build a TLE from the celestrak.org response"))?;

        Self::from_tle(tle)
    }

    pub fn from_tle(tle: Tle) -> Result<Self> {
        let elements = Elements::from_tle(
            Some(tle[0].to_owned()),
            tle[1].as_bytes(),
            tle[2].as_bytes(),
        )?;

        let constants = Constants::from_elements(&elements)?;

        Ok(Self {
            id: Some(elements.norad_id),
            tle: Some(tle),
            elements,
            constants,
        })
    }

    pub fn predict(&self, time: &DateTime<FixedOffset>) -> GeoPrediction {
        let epoch = time.offset().from_utc_datetime(&self.elements.datetime);

        let elapsed_ms = time.signed_duration_since(epoch).num_milliseconds();

        let prediction = self
            .constants
            .propagate(sgp4::MinutesSinceEpoch((elapsed_ms as f64) / (1000.0 * 60.0)))
            .unwrap();

        let gmst = sgp4::iau_epoch_to_sidereal_time(
            self.elements.epoch() + ((elapsed_ms as f64) / (31_557_600.0 * 1000.0)),
        ); // TODO

        let position = transform::eci_to_geodetic(
            prediction.position[0],
            prediction.position[1],
            prediction.position[2],
            gmst,
        );

        let v = prediction.velocity;
        let speed = (v[0].powi(2) + v[1].powi(2) + v[2].powi(2)).sqrt() * 3.6e3; // km/s to km/h

        GeoPrediction {
            position,
            speed,
            gmst,
        }
    }
}
