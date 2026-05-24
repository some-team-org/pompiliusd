use reqwest::StatusCode;
use serde::Serialize;
use tokio::net::TcpStream;
use zbus::interface;

use crate::{
    error::CloudError,
    json_result::to_ok,
    rclone_api::{Rclone, RcloneApi},
};

pub mod cache;
pub mod entities;
pub mod error;
pub mod json_result;
pub mod rclone_api;
pub mod setup_conf_dir;

type Result<T> = std::result::Result<T, CloudError>;

pub trait CloudApi {
    fn list_profiles(&self) -> impl Future<Output = String>;
    fn get_provider_options(&self, provider_type: &str) -> impl Future<Output = String>;
    fn get_files_status(
        &self,
        profile_name: &str,
        paths: Vec<String>,
    ) -> impl Future<Output = String>;
    fn create_profile(
        &self,
        profile_name: &str,
        domain: &str,
        parameters: &str,
    ) -> impl Future<Output = String>;
    fn delete_profile(&self, profile_name: &str) -> impl Future<Output = String>;
    fn mount(
        &self,
        profile_name: &str,
        file_path: &str,
        cache_max_size: &str,
        cache_max_age: &str,
    ) -> impl Future<Output = String>;
    fn link(&self, profile_name: &str, path: &str) -> impl Future<Output = String>;
    fn cache_directory(&self, path: &str) -> impl Future<Output = String>;
    fn refresh(&self, profile_name: &str, path: &str) -> impl Future<Output = String>;
    fn delete_cache_file(&self, profile_name: &str, path: &str) -> impl Future<Output = String>;
    fn delete_cache_directory(
        &self,
        profile_name: &str,
        path: &str,
    ) -> impl Future<Output = String>;
    fn delete_cache_path(&self, profile_name: &str, path: &str) -> impl Future<Output = String>;
    fn about(&self, profile_name: &str) -> impl Future<Output = String>;
    fn list_available_providers(&self) -> impl Future<Output = String>;
}

pub struct Cloud {
    pub rclone: Rclone,
}

impl Cloud {
    async fn check_internet_connection() -> Result<()> {
        let _ = TcpStream::connect("209.85.233.101:80").await?;
        Ok(())
    }
    pub async fn executor<T, F, Fut>(&self, func: F) -> Result<T>
    where
        T: Serialize,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        Cloud::check_internet_connection().await?;
        func().await
    }
}

#[interface(name = "org.zbus.pompiliusd")]
impl CloudApi for Cloud {
    async fn list_profiles(&self) -> String {
        match self.executor(|| self.rclone.list_profiles()).await {
            Ok(res) => to_ok(StatusCode::OK, res),
            Err(err) => err.into(),
        }
    }

    async fn get_provider_options(&self, provider_type: &str) -> String {
        match self
            .executor(|| self.rclone.get_provider_options(provider_type))
            .await
        {
            Ok(res) => to_ok(StatusCode::OK, res),
            Err(err) => err.into(),
        }
    }

    async fn get_files_status(&self, profile_name: &str, paths: Vec<String>) -> String {
        match self
            .executor(|| self.rclone.get_files_status(profile_name, paths))
            .await
        {
            Ok(res) => to_ok(StatusCode::OK, res),
            Err(err) => err.into(),
        }
    }

    async fn create_profile(&self, profile_name: &str, domain: &str, parameters: &str) -> String {
        match self
            .executor(|| self.rclone.create_config(profile_name, domain, parameters))
            .await
        {
            Ok(res) => to_ok(StatusCode::OK, res),
            Err(err) => err.into(),
        }
    }

    async fn delete_profile(&self, profile_name: &str) -> String {
        match self
            .executor(|| self.rclone.delete_profile(profile_name))
            .await
        {
            Ok(res) => to_ok(StatusCode::OK, res),
            Err(err) => err.into(),
        }
    }

    async fn mount(
        &self,
        profile_name: &str,
        file_path: &str,
        cache_max_size: &str,
        cache_max_age: &str,
    ) -> String {
        match self
            .executor(|| {
                self.rclone
                    .mount(profile_name, file_path, cache_max_size, cache_max_age)
            })
            .await
        {
            Ok(res) => to_ok(StatusCode::OK, res),
            Err(err) => err.into(),
        }
    }

    async fn link(&self, profile_name: &str, path: &str) -> String {
        match self.executor(|| self.rclone.link(profile_name, path)).await {
            Ok(res) => to_ok(StatusCode::OK, res),
            Err(err) => err.into(),
        }
    }

    async fn cache_directory(&self, path: &str) -> String {
        match self.executor(|| self.rclone.cache_directory(path)).await {
            Ok(res) => to_ok(StatusCode::OK, res),
            Err(err) => err.into(),
        }
    }

    async fn refresh(&self, profile_name: &str, path: &str) -> String {
        match self
            .executor(|| self.rclone.refresh(profile_name, path))
            .await
        {
            Ok(res) => to_ok(StatusCode::OK, res),
            Err(err) => err.into(),
        }
    }

    async fn delete_cache_file(&self, profile_name: &str, path: &str) -> String {
        match self
            .executor(|| self.rclone.delete_cache_file(profile_name, path))
            .await
        {
            Ok(res) => to_ok(StatusCode::OK, res),
            Err(err) => err.into(),
        }
    }

    async fn delete_cache_directory(&self, profile_name: &str, path: &str) -> String {
        match self
            .executor(|| self.rclone.delete_cache_directory(profile_name, path))
            .await
        {
            Ok(res) => to_ok(StatusCode::OK, res),
            Err(err) => err.into(),
        }
    }

    async fn delete_cache_path(&self, profile_name: &str, path: &str) -> String {
        match self
            .executor(|| self.rclone.delete_cache_path(profile_name, path))
            .await
        {
            Ok(res) => to_ok(StatusCode::OK, res),
            Err(err) => err.into(),
        }
    }

    async fn about(&self, profile_name: &str) -> String {
        match self.rclone.about(profile_name).await {
            Ok(res) => to_ok(StatusCode::OK, res),
            Err(err) => err.into(),
        }
    }

    async fn list_available_providers(&self) -> String {
        match self.rclone.list_available_providers().await {
            Ok(res) => to_ok(StatusCode::OK, res),
            Err(err) => err.into(),
        }
    }
}
