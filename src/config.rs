use std::{env, error, fs::File, io::Read, net::SocketAddr};

use serde::Deserialize;

pub const PROCY_CONFIG_PATH_ENV: &str = "PROCY_CONFIG_PATH";

#[derive(Debug, Deserialize)]
pub struct ForwardAddressConfig {
    #[serde(default, deserialize_with = "deserialize_optional_socket_addr")]
    pub listen_addr: Option<SocketAddr>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<u16>,

    #[serde(default, deserialize_with = "deserialize_optional_socket_addr")]
    pub source_addr: Option<SocketAddr>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_port: Option<u16>,

    #[serde(deserialize_with = "deserialize_socket_addr")]
    pub backend_addr: SocketAddr,
}

fn deserialize_socket_addr<'de, D>(deserializer: D) -> Result<SocketAddr, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

fn deserialize_optional_socket_addr<'de, D>(deserializer: D) -> Result<Option<SocketAddr>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<String> = Deserialize::deserialize(deserializer)?;
    match s {
        Some(addr_str) => Ok(Some(addr_str.parse().map_err(serde::de::Error::custom)?)),
        None => Ok(None),
    }
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub forward_addresses: Vec<ForwardAddressConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    #[serde(default)]
    pub level: Option<String>,
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
            forward_addresses: Vec::new(),
            logging: None,
        })
    }
}
