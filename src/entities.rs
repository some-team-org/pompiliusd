use std::collections::BTreeMap;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct RemoteConfig {
    #[serde(rename = "type")]
    pub r#type: String,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateParameters {
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl CreateParameters {
    pub fn into_string_map(self) -> HashMap<String, String> {
        self.extra
            .into_iter()
            .map(|(key, value)| {
                let val = match value {
                    Value::String(s) => s,
                    _ => value.to_string(),
                };

                (key, val)
            })
            .collect()
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub profiles: BTreeMap<String, String>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct RcloneOption {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Help")]
    pub help: String,
    #[serde(rename = "Required")]
    pub required: bool,
}

#[derive(Deserialize, Debug)]
pub struct RcloneProvider {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Options")]
    pub options: Vec<RcloneOption>,
}

#[derive(Deserialize, Debug)]
pub struct ProvidersResponse {
    pub providers: Vec<RcloneProvider>,
}

#[derive(Deserialize, Debug, Default)]
pub struct VfsMetadata {
    #[serde(rename = "Dirty")]
    pub dirty: bool,
}

#[derive(Deserialize, Debug, Default)]
pub struct VfsStatsResponse {
    pub metadata: Option<VfsMetadata>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct AboutResponse {
    pub total: Option<i64>,
    pub used: Option<i64>,
    pub free: Option<i64>,
    pub trashed: Option<i64>,
    pub other: Option<i64>,
}

#[derive(Deserialize, Debug, Default)]
pub struct CoreStatsResponse {
    pub transferring: Vec<Transferring>,
}

#[derive(Deserialize, Debug)]
pub struct Transferring {
    pub name: String,
}
