use chrono:: Utc;
use clap::Parser;
use reqwest;
use sgp4;
use std::f64::consts::PI;
use std::io::Read;
// use std::ops::Add;
// use std::time::{Duration, SystemTime};

//https://gis.stackexchange.com/questions/58923/calculating-view-angle

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = String::from("25544"))]
    id: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let tle = match get_tle_by_id(&args.id) {
        Some(v) => v,
        None => panic!("Failed to fetch TLE for ID: {}", args.id),
    };

    println!("{:?}", &tle);

    let elements = sgp4::Elements::from_tle(
        Some(tle[0].to_owned()),
        tle[1].as_bytes(),
        tle[2].as_bytes(),
    )
    .unwrap();

    let constants = sgp4::Constants::from_elements(&elements).unwrap();

    // const DIFF_J2000_UNIX: Duration = Duration::from_secs(946684800);
    // const JULIAN_YEAR_SECONDS: f64 = 365.25 * 86400.0;
    // let current_julian_epoch_years = SystemTime::now()
    //     .duration_since(SystemTime::UNIX_EPOCH.add(DIFF_J2000_UNIX))
    //     .unwrap()
    //     .as_secs_f64()
    //     / JULIAN_YEAR_SECONDS;
    // let delta_minutes = (current_julian_epoch_years - elements.epoch()) * 365.25 * 86400.0 / 60.0;
    let timestamp = Utc::now().naive_utc();
    let elapsed_ms = timestamp
        .signed_duration_since(elements.datetime)
        .num_milliseconds();
    let gmst = sgp4::iau_epoch_to_sidereal_time(
        elements.epoch() + ((elapsed_ms as f64) / (31_557_600.0 * 1000.0)),
    );
    println!("    gmst = {}", gmst);

    let prediction = constants.propagate((elapsed_ms as f64) / (1000.0 * 60.0)).unwrap();
    let sat = eci_to_geodetic(
        prediction.position[0],
        prediction.position[1],
        prediction.position[2],
        gmst
    );
    println!(
        "    lla position = {:?} km",
        sat
    );
    println!("    eci position = {:?} km", prediction.position);
    println!("    eci velocity = {:?} km.s⁻¹", prediction.velocity);

    let observer = (-36.87, 174.71, 0.0);

    let look = tally(observer, (sat.0, sat.1, sat.2*1e3));
    println!("    look = {:?}", look);

    Ok(())
}

fn get_tle_by_id(id: &String) -> Option<[String; 3]> {
    let url = format!(
        "https://celestrak.org/NORAD/elements/gp.php?CATNR={}&FORMAT=TLE",
        id
    );
    let mut res = reqwest::blocking::get(url).unwrap();

    let mut body = String::new();
    res.read_to_string(&mut body).unwrap();

    let mut lines = body.lines();
    const EMPTY_STRING: String = String::new();
    let mut tle: [String; 3] = [EMPTY_STRING; 3];
    for i in 0..3 {
        let line = lines.next();
        if line.is_some() {
            tle[i] = String::from(line.unwrap());
        } else {
            return None;
        }
    }
    Some(tle)
}

// const SEMI_MAJOR_AXIS: f64 = 6_378_137.0;
// const SEMI_MINOR_AXIS: f64 = 6_356_752.314245;
// const FIRST_ECC2: f64 = 6.69437999014e-3;
// const SECOND_ECC2: f64 = 6.73949674228e-3;

// fn ecef2lla(ecef_vec: &[f64; 3]) -> [f64; 3] {
//     // https://crates.io/crates/coord_transforms/1.4.0
//     let x = ecef_vec[0] * 1e3;
//     let y = ecef_vec[2] * 1e3;
//     let z = ecef_vec[1] * 1e3;
//     let mut ret_vec = [0.0; 3];
//     let p = (x.powi(2) + y.powi(2)).sqrt();
//     let theta = (z * SEMI_MAJOR_AXIS).atan2(p * SEMI_MINOR_AXIS);
//     let x_top = z + SECOND_ECC2 * SEMI_MINOR_AXIS * theta.sin().powi(3);
//     let x_bot = p - FIRST_ECC2 * SEMI_MAJOR_AXIS * theta.cos().powi(3);
//     let lat = x_top.atan2(x_bot);
//     let long = y.atan2(x);
//     let n = SEMI_MAJOR_AXIS / (1.0 - FIRST_ECC2 * (lat.sin() * lat.sin())).sqrt();
//     ret_vec[2] = (p / lat.cos()) - n;
//     ret_vec[1] = lat * 180.0 / std::f64::consts::PI;
//     ret_vec[0] = long * 180.0 / std::f64::consts::PI;

//     ret_vec
// }

fn eci_to_geodetic(x: f64, y: f64, z: f64, gmst: f64) -> (f64, f64, f64) {
    let theta = y.atan2(x);
    let theta = if theta < 0.0 { theta + 2.0 * PI } else { theta };
    let lambda_e = theta - gmst;
    let lambda_e = if lambda_e > PI {
        lambda_e - 2.0 * PI
    } else {
        lambda_e
    };
    let lambda_e = if lambda_e < -PI {
        lambda_e + 2.0 * PI
    } else {
        lambda_e
    };
    let lon_deg = lambda_e.to_degrees();
    let r_km = (x.powi(2) + y.powi(2)).sqrt();
    let (lat_deg, alt_km) = compute_geodetic_coords_2d(r_km, z);
    (lat_deg, lon_deg, alt_km)
}

fn compute_geodetic_coords_2d(r_km: f64, z_km: f64) -> (f64, f64) {
    const A_SQ: f64 = 40680631590769.0;
    const B_SQ: f64 = 40408299984087.0;
    const E_SQ: f64 = 0.00669437999014;
    const E_TWO_SQ: f64 = 0.00673949674228;
    let r = r_km * 1000.0;
    let r_sq = r.powi(2);
    let z = z_km * 1000.0;
    let z_sq = z.powi(2);
    let ee_sq = A_SQ - B_SQ;
    let ff = 54.0 * B_SQ * z_sq;
    let gg = r_sq + ((1.0 - E_SQ) * z_sq) - (E_SQ * ee_sq);
    let cc = (E_SQ.powi(2) * ff * r_sq) / gg.powi(3);
    let ss = (1.0 + cc + (cc.powi(2) + 2.0 * cc).sqrt()).cbrt();
    let pp = ff / (3.0 * (ss + ss.recip() + 1.0).powi(2) * gg.powi(2));
    let qq = (1.0 + 2.0 * E_SQ.powi(2) * pp).sqrt();
    let r_o = ((-(pp * E_SQ * r)) / (1.0 + qq))
        + ((0.5 * A_SQ * (1.0 + qq.recip()))
            - ((pp * (1.0 - E_SQ) * z_sq) / (qq * (1.0 + qq)))
            - (0.5 * pp * r_sq))
            .sqrt();
    let uu = ((r - E_SQ * r_o).powi(2) + z_sq).sqrt();
    let vv = ((r - E_SQ * r_o).powi(2) + (1.0 - E_SQ) * z_sq).sqrt();
    let z_o = B_SQ * z / (A_SQ.sqrt() * vv);
    let lat_deg = (z + E_TWO_SQ * z_o).atan2(r).to_degrees();
    let alt_km = uu * (1.0 - z_o / z) * 0.001;
    (lat_deg, alt_km)
}


fn tally(llh1: (f64,f64,f64), llh2: (f64,f64,f64))->(f64,f64,f64) {
    // https://gis.stackexchange.com/questions/58923/calculating-view-angle
    let (x,y,z) = llh_to_ecef(llh1.0.to_radians(), llh1.1.to_radians(), llh1.2);
    
    let (x2,y2,z2) = llh_to_ecef(llh2.0.to_radians(), llh2.1.to_radians(), llh2.2);

    let dx = x2 - x;
    let dy = y2 - y;
    let dz = z2 - z;
    
    let distance = (dx.powi(2) + dy.powi(2) + dz.powi(2)).sqrt();

    let elevation = 90.0 - ((x*dx+y*dy+z*dz)/((x.powi(2)+y.powi(2)+z.powi(2))*(dx.powi(2)+dy.powi(2)+dz.powi(2))).sqrt()).acos().to_degrees();

    let sin_azimuth = (-y*dx+x*dy)/((x.powi(2)+y.powi(2))*(dx.powi(2)+dy.powi(2)+dz.powi(2))).sqrt();

    let cos_azimuth = (-z*x*dx-z*y*dy+(x.powi(2)+y.powi(2))*dz)/((x.powi(2)+y.powi(2)+z.powi(2))*(x.powi(2)+y.powi(2))*(dx.powi(2)+dy.powi(2)+dz.powi(2))).sqrt();
    
    let mut azimuth = (sin_azimuth/cos_azimuth).atan().to_degrees();
    if azimuth < 0.0 {
        azimuth = azimuth + 360.0;
    }

    (azimuth, elevation, distance)

}

fn llh_to_ecef(lat_radians: f64, lon_radians: f64, height_meters: f64) -> (f64,f64,f64) {
    // https://en.wikipedia.org/wiki/Geographic_coordinate_conversion#From_geodetic_to_ECEF_coordinates
    const SEMI_MAJOR_AXIS_METERS: f64 = 6_378_137.0;
    const SEMI_MINOR_AXIS_METERS: f64 = 6_356_752.3;
    
    let e_squared = 1.0 - SEMI_MINOR_AXIS_METERS.powi(2)/SEMI_MAJOR_AXIS_METERS.powi(2);
    let n = SEMI_MAJOR_AXIS_METERS / (1.0 - e_squared*lat_radians.sin().powi(2)).sqrt();

    let x_meters = (n + height_meters)*lon_radians.cos()*lat_radians.cos();
    let y_meters = (n + height_meters)*lon_radians.sin()*lat_radians.cos();
    let z_meters = ((1.0 - e_squared)*n + height_meters)*lat_radians.sin();
    
    (x_meters, y_meters, z_meters)
}