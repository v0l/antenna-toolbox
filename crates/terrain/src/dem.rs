#[cfg(not(target_arch = "wasm32"))]
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
        format!("{}/{n}/{n}.tif", source())
    }
}

enum Samples {
    Owned(Vec<f32>),
    #[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
const MAGIC: &[u8; 8] = b"ATDEM001";
#[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(not(target_arch = "wasm32"))]
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
            #[cfg(not(target_arch = "wasm32"))]
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
    #[cfg(target_arch = "wasm32")]
    flying: Arc<Mutex<std::collections::HashSet<TileId>>>,
    #[cfg(target_arch = "wasm32")]
    failed: Arc<Mutex<Option<String>>>,
}

impl Default for Dem {
    fn default() -> Self {
        let base = if cfg!(target_arch = "wasm32") {
            PathBuf::new()
        } else {
            dirs::cache_dir().unwrap_or_else(std::env::temp_dir)
        };
        let cache = base.join("antenna-toolbox").join("copernicus-glo30");
        Dem::with_cache(cache)
    }
}

impl Dem {
    pub fn with_cache(cache: PathBuf) -> Self {
        Dem {
            cache,
            tiles: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(target_arch = "wasm32")]
            flying: Arc::default(),
            #[cfg(target_arch = "wasm32")]
            failed: Arc::default(),
        }
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache
    }

    #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(not(target_arch = "wasm32"))]
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
        self.load_now(id)
    }

    #[cfg(target_arch = "wasm32")]
    fn load_now(&self, id: TileId) -> Result<Option<Arc<Tile>>, String> {
        Err(format!("{} has not been fetched yet", id.name()))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_now(&self, id: TileId) -> Result<Option<Arc<Tile>>, String> {
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
        #[cfg(target_arch = "wasm32")]
        let fetched: Vec<Result<Loaded, String>> = ids
            .iter()
            .map(|&id| {
                let t = self.tile(id)?;
                progress(done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1, ids.len());
                Ok((id, t))
            })
            .collect();
        #[cfg(not(target_arch = "wasm32"))]
        let pool =
            rayon::ThreadPoolBuilder::new().num_threads(6).build().map_err(|e| e.to_string())?;
        #[cfg(not(target_arch = "wasm32"))]
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

    pub fn ensure(&self, ids: &[TileId]) -> Readiness {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = ids;
            Readiness::Ready
        }
        #[cfg(target_arch = "wasm32")]
        {
            let Ok(have) = self.tiles.lock() else {
                return Readiness::Failed("tile cache poisoned".into());
            };
            let mut wanted = Vec::new();
            let mut done = 0;
            for &id in ids {
                if have.contains_key(&id) {
                    done += 1;
                } else {
                    wanted.push(id);
                }
            }
            drop(have);
            if let Some(e) = self.failed.lock().ok().and_then(|f| f.clone()) {
                return Readiness::Failed(e);
            }
            for id in wanted {
                self.fetch_async(id);
            }
            if done == ids.len() { Readiness::Ready } else { Readiness::Pending(done, ids.len()) }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn fetch_async(&self, id: TileId) {
        let Ok(mut flying) = self.flying.lock() else {
            return;
        };
        if !flying.insert(id) {
            return;
        }
        let (tiles, failed, flying_set) =
            (self.tiles.clone(), self.failed.clone(), self.flying.clone());
        let url = id.url();
        ehttp::fetch(ehttp::Request::get(url), move |res| {
            let outcome = match res {
                Ok(r) if r.ok => Tile::decode(&r.bytes).map(|t| Some(Arc::new(t))),
                Ok(r) if r.status == 403 || r.status == 404 => Ok(None),
                Ok(r) => Err(format!("{}: HTTP {}", id.name(), r.status)),
                Err(e) => Err(format!("{}: {e}", id.name())),
            };
            match outcome {
                Ok(t) => {
                    if let Ok(mut m) = tiles.lock() {
                        m.insert(id, t);
                    }
                }
                Err(e) => {
                    if let Ok(mut f) = failed.lock() {
                        *f = Some(e);
                    }
                }
            }
            if let Ok(mut f) = flying_set.lock() {
                f.remove(&id);
            }
        });
    }

    pub fn clear_failure(&self) {
        #[cfg(target_arch = "wasm32")]
        if let Ok(mut f) = self.failed.lock() {
            *f = None;
        }
    }

    pub fn elevation(&self, lat: f64, lon: f64) -> Result<f64, String> {
        Ok(self.tile(TileId::containing(lat, lon))?.map(|t| t.sample(lat, lon)).unwrap_or(0.0))
    }
}

type Loaded = (TileId, Option<Arc<Tile>>);

#[derive(Clone, Debug, PartialEq)]
pub enum Readiness {
    Ready,
    Pending(usize, usize),
    Failed(String),
}

static SOURCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn set_source(url: &str) {
    let _ = SOURCE.set(url.trim_end_matches('/').to_string());
}

pub fn source() -> &'static str {
    SOURCE.get().map(String::as_str).unwrap_or(BASE)
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
