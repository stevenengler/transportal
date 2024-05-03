use clap::Parser;
use serde::{de, Deserialize, Deserializer};

use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

/// Command-line arguments.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Args {
    /// Path to the configuration file in TOML format.
    pub config: PathBuf,
}

/// Configuration file.
#[derive(Debug, Deserialize)]
pub struct Config {
    pub connection: ConfigConnection,
    #[serde(default)]
    pub security: ConfigSecurity,
    #[serde(default)]
    pub performance: ConfigPerformance,
}

#[derive(Debug, Deserialize)]
pub struct ConfigConnection {
    /// The socket address to bind the server to. Ex: `127.0.0.1:80` or
    /// `unix:/home/user/transportal.sock`.
    pub bind_address: CompatSocketAddr,
    /// If binding to a unix socket, these octal permissions will be used for the socket file. The
    /// umask is ignored. Ex: `600`.
    #[serde(deserialize_with = "de_octal")]
    #[serde(default = "default_octal_600")]
    pub bind_unix_perms: u32,
    #[serde(flatten)]
    pub rpc_url: RpcUrl,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ConfigSecurity {
    /// Whether the `Secure` attribute is set on cookies. If true, the browser must connect over
    /// HTTPS, localhost, or an onion service. Otherwise, authentication won't work correctly.
    pub secure_cookie_attribute: bool,
}

impl Default for ConfigSecurity {
    fn default() -> Self {
        Self {
            secure_cookie_attribute: true,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ConfigPerformance {
    /// The interval in milliseconds at which the server polls Transmission for each SSE connection.
    pub poll_interval_ms: u64,
}

impl Default for ConfigPerformance {
    fn default() -> Self {
        Self {
            poll_interval_ms: 1000,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct RpcUrl {
    /// The URL base used to connect to Transmission's RPC server. Ex: `http://127.0.0.1:9091`.
    rpc_url_base: String,
    /// The URL path used to connect to Transmission's RPC server. Ex: `/transmission/rpc`. Must
    /// have a leading slash.
    #[serde(deserialize_with = "de_url_leading_slash")]
    rpc_url_path: String,
}

impl std::fmt::Display for RpcUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}{}", self.rpc_url_base, self.rpc_url_path)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
#[serde(expecting = "data did not match an IP socket address or unix socket address")]
pub enum CompatSocketAddr {
    Ip(SocketAddr),
    Unix(UnixSocketAddr),
}

#[derive(Clone, Debug)]
pub struct UnixSocketAddr(String);

impl FromStr for UnixSocketAddr {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix("unix:")
            .map(ToString::to_string)
            .map(Self)
            .ok_or("string did not contain the 'unix:' prefix")
    }
}

impl<'de> Deserialize<'de> for UnixSocketAddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl std::fmt::Display for UnixSocketAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "unix:{}", self.0)
    }
}

impl UnixSocketAddr {
    pub fn path(&self) -> &str {
        &self.0
    }
}

fn default_octal_600() -> u32 {
    u32::from_str_radix("600", 8).unwrap()
}

fn de_url_leading_slash<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let val = String::deserialize(deserializer)?;
    if val.starts_with('/') {
        Ok(val)
    } else {
        Err(serde::de::Error::custom(format!(
            r#"the url path "{val}" must have a leading "/""#
        )))
    }
}

fn de_octal<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let val = String::deserialize(deserializer)?;
    u32::from_str_radix(&val, 8).map_err(serde::de::Error::custom)
}
