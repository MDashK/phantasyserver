#![deny(unsafe_code)]
#![warn(clippy::missing_const_for_fn)]

pub mod flags;
pub mod inventory;
pub mod map;
#[cfg(feature = "ship")]
pub mod master_ship;
pub mod quest;
pub mod stats;

use inventory::DefaultClassesData;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid input")]
    InvalidInput,
    #[error("Unknown hostkey: {0:?}")]
    UnknownHostkey(Vec<u8>),
    #[error("Operation timed out")]
    Timeout,
    #[error("No ship discovery response")]
    NoDiscoverResponse,

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[cfg(feature = "json")]
    #[error("JSON error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[cfg(feature = "toml")]
    #[error("Toml Deserialization error: {0}")]
    TomlDecodeError(#[from] toml::de::Error),

    #[cfg(feature = "rmp")]
    #[error("MP Serialization error: {0}")]
    RMPEncodeError(#[from] rmp_serde::encode::Error),
    #[cfg(feature = "rmp")]
    #[error("MP Deserialization error: {0}")]
    RMPDecodeError(#[from] rmp_serde::decode::Error),

    #[error("bincode encode error: {0}")]
    BincodeEncodeError(#[from] bincode::error::EncodeError),
    #[error("bincode decode error: {0}")]
    BincodeDecodeError(#[from] bincode::error::DecodeError),

    #[error("Invalid file format")]
    InvalidFileFormat,
    #[cfg(feature = "ship")]
    #[error("ECDSA error: {0}")]
    P256ECDSAError(#[from] p256::ecdsa::Error),
    #[cfg(feature = "ship")]
    #[error("Elliptic curve error: {0}")]
    P256ECError(#[from] p256::elliptic_curve::Error),
    #[cfg(feature = "ship")]
    #[error("Invalid key length")]
    HKDFError,
    #[cfg(feature = "ship")]
    #[error("AEAD error: {0}")]
    AEADError(String),
}

pub trait SerDeFile: Serialize + DeserializeOwned {
    #[cfg(feature = "rmp")]
    fn load_from_mp_file<T: AsRef<std::path::Path>>(path: T) -> Result<Self, Error> {
        let data = std::fs::File::open(path)?;
        let names =
            Self::deserialize(&mut rmp_serde::Deserializer::new(data).with_human_readable())?;
        Ok(names)
    }
    #[cfg(feature = "rmp")]
    fn load_from_mp_comp<T: AsRef<std::path::Path>>(path: T) -> Result<Self, Error> {
        let data = zstd::Decoder::new(std::fs::File::open(path)?)?;
        let names =
            Self::deserialize(&mut rmp_serde::Deserializer::new(data).with_human_readable())?;
        Ok(names)
    }
    #[cfg(feature = "json")]
    fn load_from_json_file<T: AsRef<std::path::Path>>(path: T) -> Result<Self, Error> {
        let data = std::fs::read_to_string(path)?;
        let names = serde_json::from_str(&data)?;
        Ok(names)
    }
    #[cfg(not(feature = "json"))]
    fn load_from_json_file<T: AsRef<std::path::Path>>(_: T) -> Result<Self, Error> {
        Err(Error::InvalidFileFormat)
    }
    #[cfg(feature = "toml")]
    fn load_from_toml_file<T: AsRef<std::path::Path>>(path: T) -> Result<Self, Error> {
        let data = std::fs::read_to_string(path)?;
        let data = toml::from_str(&data)?;
        Ok(data)
    }
    #[cfg(not(feature = "toml"))]
    fn load_from_toml_file<T>(_: T) -> Result<Self, Error> {
        Err(Error::InvalidFileFormat)
    }
    fn load_file<T: AsRef<std::path::Path>>(path: T) -> Result<Self, Error> {
        let Some(ext) = path.as_ref().extension().and_then(|e| e.to_str()) else {
            return Err(Error::InvalidFileFormat);
        };
        match ext {
            "json" => {
                if cfg!(feature = "json") {
                    Self::load_from_json_file(path)
                } else {
                    Err(Error::InvalidFileFormat)
                }
            }
            "toml" => {
                if cfg!(feature = "toml") {
                    Self::load_from_toml_file(path)
                } else {
                    Err(Error::InvalidFileFormat)
                }
            }
            _ => Err(Error::InvalidFileFormat),
        }
    }
    #[cfg(feature = "rmp")]
    fn save_to_mp_file<T: AsRef<std::path::Path>>(&self, path: T) -> Result<(), Error> {
        let file = std::fs::File::create(path)?;
        self.serialize(&mut rmp_serde::Serializer::new(file).with_human_readable())?;
        // std::io::Write::write_all(&mut file, &rmp_serde::to_vec(self)?)?;
        Ok(())
    }
    #[cfg(feature = "rmp")]
    fn save_to_mp_comp<T: AsRef<std::path::Path>>(&self, path: T) -> Result<(), Error> {
        let file = zstd::Encoder::new(std::fs::File::create(path)?, 0)?.auto_finish();
        self.serialize(&mut rmp_serde::Serializer::new(file).with_human_readable())?;
        // std::io::Write::write_all(&mut file, &rmp_serde::to_vec(self)?)?;
        Ok(())
    }
    #[cfg(feature = "json")]
    fn save_to_json_file<T: AsRef<std::path::Path>>(&self, path: T) -> Result<(), Error> {
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(file, self)?;
        Ok(())
    }
    fn save_bin_comp<T: AsRef<std::path::Path>>(&self, path: T) -> Result<(), Error> {
        let mut file = zstd::Encoder::new(std::fs::File::create(path)?, 0)?.auto_finish();
        bincode::serde::encode_into_std_write(self, &mut file, bincode::config::standard())?;
        Ok(())
    }
}
impl<T: Serialize + DeserializeOwned> SerDeFile for T {}

#[derive(Serialize, serde::Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ServerData {
    pub maps: HashMap<String, map::MapData>,
    pub quests: Vec<quest::QuestData>,
    pub item_params: inventory::ItemParameters,
    pub player_stats: stats::PlayerStats,
    pub enemy_stats: stats::AllEnemyStats,
    pub attack_stats: Vec<stats::AttackStats>,
    pub default_classes: DefaultClassesData,
}

pub fn name_to_id(name: &str) -> u32 {
    name.chars().fold(0u32, |acc, c| {
        acc ^ ((acc << 6).overflowing_add((acc >> 2).overflowing_sub(0x61c88647 - c as u32).0)).0
    })
}
