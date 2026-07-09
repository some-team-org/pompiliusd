use thiserror::Error;
use zbus::DBusError;

#[derive(DBusError, Debug)]
#[zbus(prefix = "org.zbus.pompiliusd.Error")]
pub enum CloudError {
    #[zbus(name="Reqwest")]
    Reqwest(String),
    #[zbus(name="Parse")]
    Parse(String),
    // #[error("Rclone error: {0}")]
    #[zbus(name="Rclone")]
    Rclone(String),
    #[zbus(name="Convert")]
    Convert(String),
    #[zbus(name="IO")]
    IO(String),
}

impl From<reqwest::Error> for CloudError {
    fn from(err: reqwest::Error) -> Self {
        CloudError::Reqwest(format!("Reqwest error: {}", err))
    }
}

impl From<std::io::Error> for CloudError {
    fn from(err: std::io::Error) -> Self {
        CloudError::IO(format!("IO error: {}", err))
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

#[derive(Error, Debug)]
pub enum RcloneError {
    #[error("Failed to spawn rclone process: {0}")]
    ProcessSpawn(#[source] std::io::Error),

    #[error("Error while waiting for rclone process: {0}")]
    ProcessWait(#[source] std::io::Error),

    #[error("Rclone authentication timed out")]
    AuthTimeout,

    #[error("Provider '{0}' not found in rclone configuration")]
    ProviderNotFound(String),

    #[error("Failed to cache file")]
    FailedCacheFile,

    #[error("Failed to evict from cache")]
    FailedEvictFromCache,

    #[error("Link didn't generate")]
    LinkNotGenerated,

    #[error("Mount failed")]
    MountFailed,

    #[error("Create config failed")]
    CreateConfigFailed,
}
