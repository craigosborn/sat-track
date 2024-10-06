#![cfg(all(feature = "cli", feature = "web"))]

use chrono::DateTime;
use clap::{arg, command, Command};
use sat_track::{transform, Observer, Satellite};
use tracing::warn;

fn cli() -> Command {
    command!()
        .allow_negative_numbers(true)
        .arg(
            arg!(-i --id <CATNR> "Target's NORAD Catalog Number (1-9 digits)")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            arg!(-x --lon <LONGITUDE> "Observer's longitude in degrees")
                .value_parser(clap::value_parser!(f64)),
        )
        .arg(
            arg!(-y --lat <LATITUDE> "Observer's latitude in degrees")
                .value_parser(clap::value_parser!(f64)),
        )
        .arg(
            arg!(-z --elev <ELEVATION> "Observer's elevation in meters MSL")
                .value_parser(clap::value_parser!(f64)),
        )
        .arg(
            arg!(-t --time <TIME> "Time of observation YYYY-MM-DDTHH-mm-SS.sss+HH:mm")
                .value_parser(clap::value_parser!(String)),
        )
}

fn main() {
    let args = cli().get_matches();

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Observer
    let mut observer = match (args.get_one("lat"), args.get_one("lon")) {
        (Some(lat), Some(lon)) => Observer::from_lat_lon(*lat, *lon),
        _ => {
            Observer::from_ip().unwrap_or_default()
        }
    };
    if let Some(elev) = args.get_one("elev") {
        observer.elev = *elev;
    }
    if let Some(time) = args.get_one::<String>("time").as_ref() {
        match DateTime::parse_from_rfc3339(time) {
            Ok(t) => observer.time = t,
            Err(e) => warn!("Failed to parse time: {e}"),
        }
    }

    // Target
    let id = args.get_one("id").cloned().unwrap_or(25544);
    let sat = Satellite::from_norad_cat(id).unwrap();

    // Look
    let geo_prediction = sat.predict(&observer.time);
    let position = geo_prediction.position;
    let look = transform::tally(
        observer.position(),
        (position.0, position.1, position.2 * 1e3),
    );

    // Output
    println!("{sat:?}");
    println!(
        "Prediction {{ lat: {:.3} deg, lon: {:.3} deg, alt: {:.3} km, speed: {:.3} km/h}}",
        position.1, position.0, position.2, geo_prediction.speed
    );
    println!("{observer:?}");
    println!(
        "Look {{ azimuth: {:.3?} deg, elevation: {:.3} deg, range: {:.3} km }}",
        look.0,
        look.1,
        look.2 / 1e3
    );
}
