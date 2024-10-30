use std::{env, error, fs::File, io::Read};

use serde::Deserialize;

pub const PROCY_CONFIG_PATH_ENV: &str = "PROCY_CONFIG_PATH";

#[derive(Debug, Deserialize)]
pub struct ForwardAddrStrPair {
    pub listen_addr: String,
    pub backend_addr: String,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub forward_addr_pairs: Vec<ForwardAddrStrPair>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<LogConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogConfig {
    #[serde(default)]
    pub log_path: String,

    #[serde(default)]
    pub log_level: String,
}

impl Config {
    pub fn new(env_conf_path: &str) -> Result<Self, Box<dyn error::Error>> {
        if let Ok(conf_path) = env::var(env_conf_path) {
            if !conf_path.is_empty() {
                let mut file = File::open(&conf_path)?;
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                return Ok(toml::from_str(&contents)?);
            }
        }

        Ok(Config {
            forward_addr_pairs: Vec::new(),
            log: None,
        })
    }
}
