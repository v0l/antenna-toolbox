use rayon::prelude::*;
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

enum Samples {
    Owned(Vec<f32>),
    Mapped(memmap2::Mmap),
}

pub struct Tile {
    width: usize,
    height: usize,
    origin_lon: f64,
    origin_lat: f64,
    step_lon: f64,
    step_lat: f64,
    centre: f64,
    data: Samples,
}

const MAGIC: &[u8; 8] = b"ATDEM001";
const HEADER: usize = 64;

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
            data: Samples::Owned(data),
        })
    }

    fn write_raw(&self, path: &Path) -> Result<(), String> {
        let Samples::Owned(data) = &self.data else {
            return Ok(());
        };
        let mut out = Vec::with_capacity(HEADER + data.len() * 4);
        out.extend_from_slice(MAGIC);
        for v in [self.width as f64, self.height as f64, self.origin_lon, self.origin_lat] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in [self.step_lon, self.step_lat, self.centre] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.resize(HEADER, 0);
        for v in data {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let tmp = path.with_extension("part");
        std::fs::write(&tmp, &out).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    fn open_raw(path: &Path) -> Result<Tile, String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let map = unsafe { memmap2::Mmap::map(&file) }.map_err(|e| e.to_string())?;
        if map.len() < HEADER || &map[..8] != MAGIC {
            return Err(format!("{} is not a terrain tile", path.display()));
        }
        let f = |i: usize| f64::from_le_bytes(map[8 + i * 8..16 + i * 8].try_into().unwrap());
        let (width, height) = (f(0) as usize, f(1) as usize);
        if map.len() != HEADER + width * height * 4 {
            return Err(format!("{} is truncated", path.display()));
        }
        Ok(Tile {
            width,
            height,
            origin_lon: f(2),
            origin_lat: f(3),
            step_lon: f(4),
            step_lat: f(5),
            centre: f(6),
            data: Samples::Mapped(map),
        })
    }

    fn at(&self, x: usize, y: usize) -> f64 {
        let i = y.min(self.height - 1) * self.width + x.min(self.width - 1);
        match &self.data {
            Samples::Owned(v) => v[i] as f64,
            Samples::Mapped(m) => {
                let o = HEADER + i * 4;
                f32::from_le_bytes([m[o], m[o + 1], m[o + 2], m[o + 3]]) as f64
            }
        }
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

    fn download(&self, id: TileId) -> Result<Option<Vec<u8>>, String> {
        let file = self.cache.join(format!("{}.tif", id.name()));
        if let Ok(bytes) = std::fs::read(&file) {
            return Ok(Some(bytes));
        }
        match ureq::get(&id.url()).call() {
            Ok(mut resp) => {
                let bytes = resp
                    .body_mut()
                    .with_config()
                    .limit(200 * 1024 * 1024)
                    .read_to_vec()
                    .map_err(|e| e.to_string())?;
                Ok(Some(bytes))
            }
            Err(ureq::Error::StatusCode(403 | 404)) => Ok(None),
            Err(e) => Err(format!("{}: {e}", id.name())),
        }
    }

    fn load(&self, id: TileId) -> Result<Option<Tile>, String> {
        let raw = self.cache.join(format!("{}.f32", id.name()));
        let missing = self.cache.join(format!("{}.missing", id.name()));
        if raw.exists()
            && let Ok(t) = Tile::open_raw(&raw)
        {
            return Ok(Some(t));
        }
        if missing.exists() {
            return Ok(None);
        }
        std::fs::create_dir_all(&self.cache).map_err(|e| e.to_string())?;
        let Some(bytes) = self.download(id)? else {
            let _ = std::fs::write(&missing, b"");
            return Ok(None);
        };
        Tile::decode(&bytes)?.write_raw(&raw)?;
        let _ = std::fs::remove_file(self.cache.join(format!("{}.tif", id.name())));
        Tile::open_raw(&raw).map(Some)
    }

    pub fn tile(&self, id: TileId) -> Result<Option<Arc<Tile>>, String> {
        if let Some(t) = self.tiles.lock().map_err(|_| "poisoned")?.get(&id) {
            return Ok(t.clone());
        }
        let tile = self.load(id)?.map(Arc::new);
        self.tiles.lock().map_err(|_| "poisoned")?.insert(id, tile.clone());
        Ok(tile)
    }

    pub fn tiles_in(south: f64, west: f64, north: f64, east: f64) -> Vec<TileId> {
        (south.floor() as i32..=north.floor() as i32)
            .flat_map(|lat| {
                (west.floor() as i32..=east.floor() as i32)
                    .map(move |lon| TileId { lat, lon: (lon + 180).rem_euclid(360) - 180 })
            })
            .collect()
    }

    pub fn sampler(
        &self,
        south: f64,
        west: f64,
        north: f64,
        east: f64,
        progress: &(dyn Fn(usize, usize) + Sync),
    ) -> Result<Sampler, String> {
        let ids = Self::tiles_in(south, west, north, east);
        let done = std::sync::atomic::AtomicUsize::new(0);
        let pool =
            rayon::ThreadPoolBuilder::new().num_threads(6).build().map_err(|e| e.to_string())?;
        let fetched: Vec<Result<Loaded, String>> = pool.install(|| {
            ids.par_iter()
                .with_max_len(1)
                .map(|&id| {
                    let t = self.tile(id)?;
                    let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    progress(n, ids.len());
                    Ok((id, t))
                })
                .collect()
        });
        let tiles = fetched.into_iter().collect::<Result<HashMap<_, _>, String>>()?;
        Ok(Sampler { tiles })
    }

    pub fn elevation(&self, lat: f64, lon: f64) -> Result<f64, String> {
        Ok(self.tile(TileId::containing(lat, lon))?.map(|t| t.sample(lat, lon)).unwrap_or(0.0))
    }
}

type Loaded = (TileId, Option<Arc<Tile>>);

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
    fn a_raw_tile_maps_back_to_the_same_heights() {
        let tile = Tile {
            width: 3,
            height: 2,
            origin_lon: -9.0,
            origin_lat: 54.0,
            step_lon: 0.5,
            step_lat: 0.5,
            centre: 0.0,
            data: Samples::Owned(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.5]),
        };
        let path = std::env::temp_dir().join(format!("atdem-{}.f32", std::process::id()));
        tile.write_raw(&path).unwrap();
        let back = Tile::open_raw(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        for (lat, lon) in [(54.0, -9.0), (53.75, -8.25), (53.5, -8.0)] {
            assert_eq!(back.sample(lat, lon), tile.sample(lat, lon));
        }
        assert_eq!(back.sample(53.5, -8.0), 6.5);
    }

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
