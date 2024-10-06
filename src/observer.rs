use chrono::{DateTime, FixedOffset, Utc};

#[cfg(feature = "web")]
use {
    reqwest,
    serde::Deserialize,
    tracing::{info, warn},
    anyhow::{anyhow, Result},
};

#[derive(Clone, Debug)]
pub struct Observer {
    pub lat: f64,
    pub lon: f64,
    pub elev: f64,
    pub time: DateTime<FixedOffset>,
}

impl Default for Observer {
    fn default() -> Self {
        Self {
            lat: 0.0,
            lon: 0.0,
            elev: 0.0,
            time: Utc::now().into(),
        }
    }
}

#[cfg(feature = "web")]
#[derive(Deserialize)]
struct IpInfo {
    loc: String,
}

#[cfg(feature = "web")]
#[derive(Deserialize)]
struct TopoData {
    results: [TopoResults; 1],
}

#[cfg(feature = "web")]
#[derive(Deserialize)]
struct TopoResults {
    elevation: f64,
}

impl Observer {
    #[cfg(feature = "web")]
    pub fn from_ip() -> Result<Self> {
        // Location
        let info: IpInfo = reqwest::blocking::get("https://ipinfo.io/json")?.json()?;
        let mut loc_parts = info.loc.split_terminator(',').map(|s| s.parse::<f64>());
        let (Some(Ok(lat)), Some(Ok(lon))) = (loc_parts.next(), loc_parts.next()) else {
            return Err(anyhow!("Failed to parse location from web request"));
        };
        info!("Got a location of {lat}, {lon} from https://ipinfo.io");

        // Elevation
        let elev = match reqwest::blocking::get(format!(
            "https://api.opentopodata.org/v1/etopo1?locations={lat},{lon}"
        ))
        .and_then(|res| res.json::<TopoData>())
        {
            Ok(topo) => {
                let elev = topo.results[0].elevation;
                info!("Got an elevation of {elev}m from https://opentopodata.org");
                elev
            }
            Err(e) => {
                warn!("Failed to get elevation from https://opentopodata.org: {e}");
                0.0
            }
        };

        Ok(Self {
            lat,
            lon,
            elev,
            time: Utc::now().into(),
        })
    }

    pub fn from_lat_lon(lat: f64, lon: f64) -> Self {
        Self {
            lat,
            lon,
            elev: 0.0,
            time: Utc::now().into(),
        }
    }

    pub fn with_elevation(mut self, elevation: f64) -> Self {
        self.elev = elevation;
        self
    }

    pub fn with_time(mut self, time: DateTime<FixedOffset>) -> Self {
        self.time = time;
        self
    }

    pub fn with_current_time(self) -> Self {
        self.with_time(Utc::now().into())
    }

    pub fn position(&self) -> (f64, f64, f64) {
        (self.lon, self.lat, self.elev)
    }
}
