use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tiff::decoder::{Decoder, DecodingResult};
use tiff::tags::Tag;

const BASE: &str = "https://copernicus-dem-30m.s3.amazonaws.com";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileId {
    pub lat: i32,
    pub lon: i32,
}

impl TileId {
    pub fn containing(lat: f64, lon: f64) -> Self {
        TileId { lat: lat.floor() as i32, lon: lon.floor() as i32 }
    }

    pub fn name(self) -> String {
        let ns = if self.lat >= 0 { 'N' } else { 'S' };
        let ew = if self.lon >= 0 { 'E' } else { 'W' };
        format!(
            "Copernicus_DSM_COG_10_{ns}{:02}_00_{ew}{:03}_00_DEM",
            self.lat.unsigned_abs(),
            self.lon.unsigned_abs()
        )
    }

    pub fn url(self) -> String {
        let n = self.name();
        format!("{BASE}/{n}/{n}.tif")
    }
}

pub struct Tile {
    width: usize,
    height: usize,
    origin_lon: f64,
    origin_lat: f64,
    step_lon: f64,
    step_lat: f64,
    centre: f64,
    data: Vec<f32>,
}

impl Tile {
    pub fn decode(bytes: &[u8]) -> Result<Tile, String> {
        let mut dec = Decoder::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
        let (w, h) = dec.dimensions().map_err(|e| e.to_string())?;
        let scale = dec.get_tag_f64_vec(Tag::ModelPixelScaleTag).map_err(|e| e.to_string())?;
        let tie = dec.get_tag_f64_vec(Tag::ModelTiepointTag).map_err(|e| e.to_string())?;
        let point = dec
            .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
            .ok()
            .and_then(|k| k.get(4..)?.chunks_exact(4).find(|e| e[0] == 1025).map(|e| e[3] == 2))
            .unwrap_or(false);
        let data = match dec.read_image().map_err(|e| e.to_string())? {
            DecodingResult::F32(v) => v,
            DecodingResult::I16(v) => v.into_iter().map(f32::from).collect(),
            _ => return Err("unexpected sample format".into()),
        };
        Ok(Tile {
            width: w as usize,
            height: h as usize,
            origin_lon: tie[3] - tie[0] * scale[0],
            origin_lat: tie[4] + tie[1] * scale[1],
            step_lon: scale[0],
            step_lat: scale[1],
            centre: if point { 0.0 } else { 0.5 },
            data,
        })
    }

    fn at(&self, x: usize, y: usize) -> f64 {
        self.data[y.min(self.height - 1) * self.width + x.min(self.width - 1)] as f64
    }

    pub fn sample(&self, lat: f64, lon: f64) -> f64 {
        let fx = ((lon - self.origin_lon) / self.step_lon - self.centre).max(0.0);
        let fy = ((self.origin_lat - lat) / self.step_lat - self.centre).max(0.0);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
        let top = self.at(x0, y0) * (1.0 - tx) + self.at(x0 + 1, y0) * tx;
        let bottom = self.at(x0, y0 + 1) * (1.0 - tx) + self.at(x0 + 1, y0 + 1) * tx;
        top * (1.0 - ty) + bottom * ty
    }
}

#[derive(Clone)]
pub struct Dem {
    cache: PathBuf,
    tiles: Arc<Mutex<HashMap<TileId, Option<Arc<Tile>>>>>,
}

impl Default for Dem {
    fn default() -> Self {
        let cache = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("antenna-toolbox")
            .join("copernicus-glo30");
        Dem::with_cache(cache)
    }
}

impl Dem {
    pub fn with_cache(cache: PathBuf) -> Self {
        Dem { cache, tiles: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache
    }

    fn fetch(&self, id: TileId) -> Result<Option<Vec<u8>>, String> {
        let file = self.cache.join(format!("{}.tif", id.name()));
        let missing = self.cache.join(format!("{}.missing", id.name()));
        if let Ok(bytes) = std::fs::read(&file) {
            return Ok(Some(bytes));
        }
        if missing.exists() {
            return Ok(None);
        }
        std::fs::create_dir_all(&self.cache).map_err(|e| e.to_string())?;
        match ureq::get(&id.url()).call() {
            Ok(mut resp) => {
                let bytes = resp
                    .body_mut()
                    .with_config()
                    .limit(200 * 1024 * 1024)
                    .read_to_vec()
                    .map_err(|e| e.to_string())?;
                let tmp = file.with_extension("part");
                std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
                std::fs::rename(&tmp, &file).map_err(|e| e.to_string())?;
                Ok(Some(bytes))
            }
            Err(ureq::Error::StatusCode(403 | 404)) => {
                let _ = std::fs::write(&missing, b"");
                Ok(None)
            }
            Err(e) => Err(format!("{}: {e}", id.name())),
        }
    }

    pub fn tile(&self, id: TileId) -> Result<Option<Arc<Tile>>, String> {
        if let Some(t) = self.tiles.lock().map_err(|_| "poisoned")?.get(&id) {
            return Ok(t.clone());
        }
        let tile = match self.fetch(id)? {
            Some(bytes) => Some(Arc::new(Tile::decode(&bytes)?)),
            None => None,
        };
        self.tiles.lock().map_err(|_| "poisoned")?.insert(id, tile.clone());
        Ok(tile)
    }

    pub fn sampler(&self, south: f64, west: f64, north: f64, east: f64) -> Result<Sampler, String> {
        let mut tiles = HashMap::new();
        for lat in south.floor() as i32..=north.floor() as i32 {
            for lon in west.floor() as i32..=east.floor() as i32 {
                let id = TileId { lat, lon: (lon + 180).rem_euclid(360) - 180 };
                tiles.insert(id, self.tile(id)?);
            }
        }
        Ok(Sampler { tiles })
    }

    pub fn elevation(&self, lat: f64, lon: f64) -> Result<f64, String> {
        Ok(self.tile(TileId::containing(lat, lon))?.map(|t| t.sample(lat, lon)).unwrap_or(0.0))
    }
}

pub struct Sampler {
    tiles: HashMap<TileId, Option<Arc<Tile>>>,
}

impl Sampler {
    pub fn elevation(&self, lat: f64, lon: f64) -> f64 {
        match self.tiles.get(&TileId::containing(lat, lon)) {
            Some(Some(t)) => t.sample(lat, lon),
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_names_follow_the_bucket_layout() {
        assert_eq!(
            TileId::containing(53.27, -9.05).name(),
            "Copernicus_DSM_COG_10_N53_00_W010_00_DEM"
        );
        assert_eq!(
            TileId::containing(-33.9, 151.2).name(),
            "Copernicus_DSM_COG_10_S34_00_E151_00_DEM"
        );
    }
}
