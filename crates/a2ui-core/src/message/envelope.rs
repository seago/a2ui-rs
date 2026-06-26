use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "version")]
pub enum ServerEnvelope {
    #[serde(rename = "v1.0")]
    V1_0(super::server_to_client::V1_0ServerMessage),
}

impl ServerEnvelope {
    pub fn from_json(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "version")]
pub enum ClientEnvelope {
    #[serde(rename = "v1.0")]
    V1_0(super::client_to_server::V1_0ClientMessage),
}

impl ClientEnvelope {
    pub fn from_json(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}
