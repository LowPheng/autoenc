use std::{
    collections::BTreeMap,
    fs::Metadata,
    io::ErrorKind,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Cache {
    version: u32,
    files: BTreeMap<PathBuf, FileCache>,

    #[serde(skip)]
    cache_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
pub struct FileCache {
    pub mtime: u64,
    pub size: u64,
    pub encrypt: bool,
}

impl Cache {
    const CACHE_VERSION: u32 = 1;

    pub fn new(cache_path: PathBuf) -> Self {
        Self {
            version: Self::CACHE_VERSION,
            files: BTreeMap::new(),
            cache_path,
        }
    }

    pub fn from_file(cache_path: PathBuf) -> Self {
        let Ok(data) = std::fs::read(&cache_path).inspect_err(|error| {
            if error.kind() != ErrorKind::NotFound {
                log::warn!("Load cache file error: {error}");
            }
        }) else {
            return Self::new(cache_path);
        };
        let Ok(mut cache) = postcard::from_bytes::<Self>(&data)
            .inspect_err(|error| log::info!("Parse cache file error: {error}"))
        else {
            return Self::new(cache_path);
        };
        if cache.version == Self::CACHE_VERSION {
            cache.cache_path = cache_path;
            cache
        } else {
            log::info!("Cache version mismatch");
            Self::new(cache_path)
        }
    }

    pub fn get_and_check(&self, path: &Path, metadata: &Metadata) -> Option<&FileCache> {
        let cache = self.files.get(path)?;
        if metadata.last_write_time() == cache.mtime && metadata.len() == cache.size {
            Some(cache)
        } else {
            None
        }
    }

    pub fn update(&mut self, path: PathBuf, metadata: &Metadata, encrypt: bool) {
        self.files.insert(
            path,
            FileCache {
                mtime: metadata.last_write_time(),
                size: metadata.len(),
                encrypt,
            },
        );

        let Ok(data) = postcard::to_stdvec(self)
            .inspect_err(|error| log::error!("Serialize cache data error: {error}"))
        else {
            return;
        };
        _ = std::fs::write(&self.cache_path, data)
            .inspect_err(|error| log::error!("Write cache file error: {error}"));
    }
}
