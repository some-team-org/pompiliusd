use zbus::DBusError;

#[derive(DBusError, Debug)]
#[zbus(prefix = "org.zbus.pompiliusd.Error")]
pub enum CloudError {
    Reqwest(String),
    Parse(String),
    // #[error("Rclone error: {0}")]
    Rclone(String),
    Convert(String),
    Io(String),
}

impl From<reqwest::Error> for CloudError {
    fn from(err: reqwest::Error) -> Self {
        CloudError::Reqwest(format!("Reqwest error: {}", err))
    }
}

impl From<std::io::Error> for CloudError {
    fn from(err: std::io::Error) -> Self {
        CloudError::Io(format!("IO error: {}", err))
    }
}

impl From<serde_json::Error> for CloudError {
    fn from(err: serde_json::Error) -> Self {
        CloudError::Parse(format!("Parse json error: {}", err))
    }
}

impl From<toml::ser::Error> for CloudError {
    fn from(err: toml::ser::Error) -> Self {
        CloudError::Parse(format!("Parse toml error: {}", err))
    }
}
