//! Endpoint management
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Endpoint {
    pub ip: String,
    pub port: u16,
    pub ready: bool,
}
