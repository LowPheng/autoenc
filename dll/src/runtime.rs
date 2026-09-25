use std::{
    fs::File,
    io::{Cursor, Read, Seek, Write},
    path::PathBuf,
    str::FromStr,
    sync::Mutex,
};

use anyhow::{Context, Result, anyhow};
use log::LevelFilter;
use windows_sys::Win32::System::Environment::GetCurrentDirectoryW;

use crate::{
    cache::Cache,
    config::{Compression, Config},
    logging::Logger,
    transform::{self, DecryptionState},
    utils,
};

pub struct Runtime {
    pub cache: Option<Mutex<Cache>>,

    pub key: [u8; 32],
    pub encryption_path: PathBuf,
    pub decryption_path: PathBuf,
    pub inject_level_filename: bool,

    pub compression: bool,

    current_path: PathBuf,
}

pub enum FileRouting<'a> {
    Original,
    Route(PathBuf, &'a str),
}

impl Runtime {
    pub fn initialize() -> Result<Option<Self>> {
        let config_data = std::fs::read_to_string("autoenc.toml")
            .with_context(|| "Failed to open config file 'autoenc.toml'")?;
        let config: Config = toml::from_str(&config_data)?;
        if !config.enabled {
            return Ok(None);
        }

        if config.logging.enabled {
            _ = log::set_logger(Box::leak(Box::new(Logger::new(PathBuf::from(
                config.logging.log_file,
            ))?)));
            log::set_max_level(
                config
                    .logging
                    .max_level
                    .and_then(|level| LevelFilter::from_str(&level).ok())
                    .unwrap_or(LevelFilter::Info),
            );
        }

        let key: [u8; 32] = config
            .key
            .as_bytes()
            .try_into()
            .map_err(|_| anyhow!("Key must be 32 characters"))?;

        let cache = if !config.cache.enabled {
            None
        } else {
            Some(Mutex::new(Cache::from_file(PathBuf::from(
                config.cache.cache_file,
            ))))
        };

        let current_path =
            utils::get_directory_wide(|buf, len| unsafe { GetCurrentDirectoryW(len, buf) });

        Ok(Some(Self {
            cache,
            key,
            encryption_path: config.encryption_output,
            decryption_path: config.decryption_output,
            inject_level_filename: config.inject_level_filename,
            compression: config.compression == Compression::SevenZip,
            current_path,
        }))
    }

    pub fn handle_file<'a>(&self, file: PathBuf, mode: &'a str) -> Result<FileRouting<'a>> {
        let original_path = file;
        if !original_path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("lua"))
        {
            return Ok(FileRouting::Original);
        }

        log::debug!("fopen: {}", original_path.to_string_lossy());

        // Newer version like Classic 4.0.0 uses absolute path for script loading.
        let rel_path = original_path.strip_prefix(&self.current_path).ok();
        let path = rel_path.unwrap_or(&original_path);
        if path.is_absolute()
            || path
                .parent()
                .is_none_or(|parent| parent.as_os_str().is_empty())
        {
            // May be persistent file, skipping.
            log::info!("Skip: {}", original_path.to_string_lossy());
            return Ok(FileRouting::Original);
        }

        if mode.contains('r')
            && let Ok(metadata) = std::fs::metadata(&original_path)
            && metadata.is_file()
        {
            if let Some(cache) = self.cache.as_ref().map(|cache| cache.lock().unwrap())
                && let Some(cache_file) = cache.get_and_check(path, &metadata)
            {
                let dir = if cache_file.encrypt {
                    &self.encryption_path
                } else {
                    &self.decryption_path
                };
                let target_path = if let Some(rel_path) = rel_path {
                    let mut path = self.current_path.join(dir);
                    path.push(rel_path);
                    path
                } else {
                    dir.join(&original_path)
                };

                if target_path.exists() {
                    log::info!("Cache Hit: {}", original_path.to_string_lossy());
                    if cache_file.encrypt {
                        return Ok(FileRouting::Route(target_path, mode));
                    } else {
                        return Ok(FileRouting::Original);
                    }
                }
            }

            let file = File::open(&original_path)?;
            let data: Option<(Box<dyn Read>, u64)> =
                match transform::decrypt(file, &self.key, self.compression)? {
                    DecryptionState::Encrypted(data) => {
                        log::info!("Decrypt: {}", original_path.to_string_lossy());

                        let dec_path = if let Some(rel_path) = rel_path {
                            let mut path = self.current_path.join(&self.decryption_path);
                            path.push(rel_path);
                            path
                        } else {
                            self.decryption_path.join(&original_path)
                        };
                        utils::write_file(dec_path, data)?;

                        None
                    }
                    DecryptionState::Unencrypted(mut file, len) => {
                        log::info!("Encrypt: {}", original_path.to_string_lossy());

                        file.rewind()?;
                        Some((Box::new(file), len))
                    }
                    DecryptionState::Uncompressed(data) => {
                        log::info!("Encrypt: {}", original_path.to_string_lossy());

                        let len = data.len();
                        Some((Box::new(Cursor::new(data)), len as u64))
                    }
                };

            let (result, encrypt) = if let Some((read, len)) = data {
                let data = transform::encrypt(
                    read,
                    len,
                    &path.to_string_lossy().replace('\\', "/"),
                    &self.key,
                    self.compression,
                )?;
                let enc_path = if let Some(rel_path) = rel_path {
                    let mut path = self.current_path.join(&self.encryption_path);
                    path.push(rel_path);
                    path
                } else {
                    self.encryption_path.join(&original_path)
                };
                utils::write_file(&enc_path, data)?;

                (FileRouting::Route(enc_path, mode), true)
            } else {
                (FileRouting::Original, false)
            };

            if let Some(mut cache) = self.cache.as_ref().map(|cache| cache.lock().unwrap()) {
                cache.update(path.to_path_buf(), &metadata, encrypt);
            }
            Ok(result)
        } else if mode.contains('w') && self.inject_level_filename {
            let Some(filename) = original_path.file_name() else {
                return Ok(FileRouting::Original);
            };
            log::info!("Inject Filename: {}", original_path.to_string_lossy());
            let mut file = File::create(&original_path)?;

            write!(file, "filename = \"")?;
            file.write_all(filename.as_encoded_bytes())?; // Not escaped
            writeln!(file, "\"")?;

            Ok(FileRouting::Route(original_path, "ab"))
        } else {
            Ok(FileRouting::Original)
        }
    }
}
