use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub enabled: bool,

    pub key: String,
    pub encryption_output: PathBuf,
    pub decryption_output: PathBuf,
    pub inject_level_filename: bool,

    #[serde(default)]
    pub compression: Compression,

    pub cache: Cache,
    pub logging: Logging,
}

#[derive(Deserialize, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Compression {
    #[default]
    None,
    #[serde(rename = "7z")]
    SevenZip,
}

#[derive(Deserialize)]
pub struct Cache {
    pub enabled: bool,
    pub cache_file: String,
}

#[derive(Deserialize)]
pub struct Logging {
    pub enabled: bool,
    pub log_file: String,
    pub max_level: Option<String>,
}
