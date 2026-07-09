use std::collections::HashMap;

use serde::Serialize;
use tokio::net::TcpStream;
use zbus::interface;

use crate::{
    error::CloudError,
    rclone_api::{Rclone, RcloneApi},
};

pub mod cache;
pub mod entities;
pub mod error;
pub mod rclone_api;
pub mod setup_conf_dir;

type Result<T> = std::result::Result<T, CloudError>;

pub trait CloudApi {
    fn list_profiles(&self) -> impl Future<Output = Result<Vec<(String, String)>>>;

    fn get_provider_options(
        &self,
        provider_type: &str,
    ) -> impl Future<Output = Result<Vec<String>>>;

    fn get_files_status(
        &self,
        profile_name: &str,
        paths: Vec<String>,
    ) -> impl Future<Output = Result<HashMap<String, String>>>;

    fn create_profile(
        &self,
        profile_name: &str,
        domain: &str,
        parameters: &str,
    ) -> impl Future<Output = Result<String>>;

    fn delete_profile(&self, profile_name: &str) -> impl Future<Output = Result<String>>;

    fn mount(
        &self,
        profile_name: &str,
        file_path: &str,
        cache_max_size: &str,
        cache_max_age: &str,
    ) -> impl Future<Output = Result<String>>;

    fn link(&self, profile_name: &str, path: &str) -> impl Future<Output = Result<String>>;

    fn cache_directory(&self, path: &str) -> impl Future<Output = Result<String>>;

    fn refresh(&self, profile_name: &str, path: &str) -> impl Future<Output = Result<String>>;

    fn delete_cache_file(
        &self,
        profile_name: &str,
        path: &str,
    ) -> impl Future<Output = Result<String>>;

    fn delete_cache_directory(
        &self,
        profile_name: &str,
        path: &str,
    ) -> impl Future<Output = Result<String>>;

    fn delete_cache_path(
        &self,
        profile_name: &str,
        path: &str,
    ) -> impl Future<Output = Result<String>>;

    fn about(&self, profile_name: &str) -> impl Future<Output = Result<String>>;

    fn list_available_providers(&self) -> impl Future<Output = Result<Vec<String>>>;
}

pub struct Cloud {
    pub rclone: Rclone,
}

impl Cloud {
    async fn check_internet_connection() -> Result<()> {
        let _ = TcpStream::connect("209.85.233.101:80").await?;
        Ok(())
    }

    async fn executor<T, F, Fut>(&self, func: F) -> Result<T>
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
    async fn list_profiles(&self) -> Result<Vec<(String, String)>> {
        self.executor(|| self.rclone.list_profiles()).await
    }

    async fn get_provider_options(&self, provider_type: &str) -> Result<Vec<String>> {
        self.executor(|| self.rclone.get_provider_options(provider_type))
            .await
    }

    async fn get_files_status(
        &self,
        profile_name: &str,
        paths: Vec<String>,
    ) -> Result<HashMap<String, String>> {
        self.executor(|| self.rclone.get_files_status(profile_name, paths))
            .await
    }

    async fn create_profile(
        &self,
        profile_name: &str,
        domain: &str,
        parameters: &str,
    ) -> Result<String> {
        self.executor(|| self.rclone.create_config(profile_name, domain, parameters))
            .await
    }

    async fn delete_profile(&self, profile_name: &str) -> Result<String> {
        self.executor(|| self.rclone.delete_profile(profile_name))
            .await
    }

    async fn mount(
        &self,
        profile_name: &str,
        file_path: &str,
        cache_max_size: &str,
        cache_max_age: &str,
    ) -> Result<String> {
        self.executor(|| {
            self.rclone
                .mount(profile_name, file_path, cache_max_size, cache_max_age)
        })
        .await
    }

    async fn link(&self, profile_name: &str, path: &str) -> Result<String> {
        self.executor(|| self.rclone.link(profile_name, path)).await
    }

    async fn cache_directory(&self, path: &str) -> Result<String> {
        self.executor(|| self.rclone.cache_directory(path)).await
    }

    async fn refresh(&self, profile_name: &str, path: &str) -> Result<String> {
        self.executor(|| self.rclone.refresh(profile_name, path))
            .await
    }

    async fn delete_cache_file(&self, profile_name: &str, path: &str) -> Result<String> {
        self.executor(|| self.rclone.delete_cache_file(profile_name, path))
            .await
    }

    async fn delete_cache_directory(&self, profile_name: &str, path: &str) -> Result<String> {
        self.executor(|| self.rclone.delete_cache_directory(profile_name, path))
            .await
    }

    async fn delete_cache_path(&self, profile_name: &str, path: &str) -> Result<String> {
        self.executor(|| self.rclone.delete_cache_path(profile_name, path))
            .await
    }

    async fn about(&self, profile_name: &str) -> Result<String> {
        let about_resp = self.executor(|| self.rclone.about(profile_name)).await?;
        let res_in_json = serde_json::to_string(&about_resp)?;
        Ok(res_in_json)
    }

    async fn list_available_providers(&self) -> Result<Vec<String>> {
        self.executor(|| self.rclone.list_available_providers())
            .await
    }
}
