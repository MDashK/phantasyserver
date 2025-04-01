use crate::Error;
use clap::Parser;
use rsa::{
    RsaPrivateKey,
    pkcs8::{DecodePrivateKey, EncodePrivateKey},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub server_name: String,
    pub db_name: String,
    pub min_ship_id: u32,
    pub max_ship_id: u32,
    pub blocks: Vec<BlockSettings>,

    pub key_file: Option<String>,

    pub balance_port: u16,
    pub hostkeys_file: String,
    pub master_ship: Option<String>,
    pub master_ship_psk: String,
    pub data_file: Option<String>,
    pub log_dir: String,
    pub file_log_level: log::LevelFilter,
    pub console_log_level: log::LevelFilter,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Location of the settings file
    #[arg(short, long)]
    settings_file: Option<String>,
    /// Don't create settings file if it doesn't exist
    #[arg(long, default_value_t = false)]
    dont_create_settings: bool,
    /// Name of the server
    #[arg(short('N'), long)]
    server_name: Option<String>,
    /// Path to the DB file
    #[arg(short('D'), long)]
    db_path: Option<String>,
    /// Minimal ship list ID
    #[arg(long)]
    min_ship_id: Option<u32>,
    /// Maximum ship list ID
    #[arg(long)]
    max_ship_id: Option<u32>,
    /// Location of the RSA private key
    #[arg(short, long)]
    key_file: Option<String>,
    /// Master ship balance port
    #[arg(long)]
    balance_port: Option<u16>,
    /// Hostkeys file location
    #[arg(short('H'), long)]
    hostkeys_file: Option<String>,
    /// IP of the master ship
    #[arg(short, long)]
    master_ship_ip: Option<String>,
    /// Preshared key for master ship connection
    #[arg(short('P'), long)]
    master_ship_psk: Option<String>,
    /// Location of the logs directory
    #[arg(short, long)]
    log_dir: Option<String>,
    /// Log level of log files
    #[arg(short, long)]
    file_log_level: Option<log::LevelFilter>,
    /// Log level of console
    #[arg(short, long)]
    console_log_level: Option<log::LevelFilter>,
    /// Location of complied server data file
    #[arg(short, long)]
    data_path: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct BlockSettings {
    pub port: Option<u16>,
    pub name: String,
    pub max_players: u32,
    pub lobby_map: String,
}

macro_rules! args_to_settings {
    ($arg:expr => $set:expr) => {
        if let Some(x) = $arg {
            $set = x;
        }
    };
}

impl Settings {
    pub async fn load(path: &str) -> Result<Self, Error> {
        let args = Args::parse();
        let path = if let Some(path) = &args.settings_file {
            path
        } else {
            path
        };
        let mut settings = match tokio::fs::read_to_string(path).await {
            Ok(s) => toml::from_str(&s)?,
            Err(_) => Self::create_default(path).await?,
        };

        args_to_settings!(args.server_name => settings.server_name);
        args_to_settings!(args.db_path => settings.db_name);
        args_to_settings!(args.min_ship_id => settings.min_ship_id);
        args_to_settings!(args.max_ship_id => settings.max_ship_id);
        settings.key_file = args.key_file.or(settings.key_file);
        args_to_settings!(args.balance_port => settings.balance_port);
        args_to_settings!(args.hostkeys_file => settings.hostkeys_file);
        settings.master_ship = args.master_ship_ip.or(settings.master_ship);
        args_to_settings!(args.master_ship_psk => settings.master_ship_psk);
        args_to_settings!(args.log_dir => settings.log_dir);
        args_to_settings!(args.file_log_level => settings.file_log_level);
        args_to_settings!(args.console_log_level => settings.console_log_level);
        settings.data_file = args.data_path.or(settings.data_file);

        Ok(settings)
    }
    pub fn load_key(&self) -> Result<RsaPrivateKey, Error> {
        log::info!("Loading keypair");
        let key = match &self.key_file {
            Some(keyfile_path) => match std::fs::metadata(keyfile_path) {
                Ok(..) => RsaPrivateKey::read_pkcs8_pem_file(keyfile_path)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    log::warn!("Keyfile doesn't exist, creating...");
                    let key = RsaPrivateKey::new(&mut rand::thread_rng(), 1024)?;
                    key.write_pkcs8_pem_file(keyfile_path, rsa::pkcs8::LineEnding::default())?;
                    log::info!("Keyfile created.");
                    key
                }
                Err(e) => {
                    log::error!("Failed to load keypair: {e}");
                    return Err(e.into());
                }
            },
            None => {
                let key = RsaPrivateKey::new(&mut rand::thread_rng(), 1024)?;
                log::info!("Keyfile created.");
                key
            }
        };
        log::info!("Loaded keypair");
        Ok(key)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server_name: String::from("phantasyserver"),
            db_name: String::from("ship.db"),
            balance_port: 12000,
            min_ship_id: 1,
            max_ship_id: 10,
            blocks: vec![BlockSettings::default()],
            key_file: None,
            hostkeys_file: String::from("hostkeys.toml"),
            master_ship: None,
            master_ship_psk: String::from("master_ship_psk"),
            data_file: None,
            log_dir: String::from("logs"),
            file_log_level: log::LevelFilter::Info,
            console_log_level: log::LevelFilter::Debug,
        }
    }
}
impl Default for BlockSettings {
    fn default() -> Self {
        Self {
            port: None,
            name: "Block 1".to_string(),
            max_players: 32,
            lobby_map: "lobby".to_string(),
        }
    }
}

impl Settings {
    pub async fn create_default(path: &str) -> Result<Self, Error> {
        let mut settings = Self::default();
        settings.blocks.push(BlockSettings {
            port: Some(13002),
            name: "Block 2".into(),
            ..Default::default()
        });

        let toml_doc = toml::to_string_pretty(&settings)?;
        tokio::fs::write(path, toml_doc).await?;
        Ok(settings)
    }
}
