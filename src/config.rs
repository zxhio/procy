use serde::Deserialize;

pub const PROCY_CONFIG_PATH_ENV: &str = "PROCY_CONFIG_PATH";

#[derive(Debug, Deserialize)]
pub struct Config {
    pub forward_addr_pairs: Vec<ForwardAddrStrPair>,
}

#[derive(Debug, Deserialize)]
pub struct ForwardAddrStrPair {
    pub listen_addr: String,
    pub backend_addr: String,
}
