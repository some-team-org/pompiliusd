use crate::cache::get_all_files;
use crate::{entities::*, error::CloudError, setup_conf_dir};
use reqwest::Client;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicU32;
use std::time::Duration;
use std::{collections::HashMap, future::Future, process::Stdio};
use tokio::process::Command;
use tokio::time::timeout;

type Result<T> = std::result::Result<T, CloudError>;

/// PID rclone процесса для остановки процесса создания профиля в случае зависания
// NOTE: без ручного контроля игнорирование oauth-а может привести к вечному
// зависанию rclone-а на 53682 порту
static AUTH_PID: AtomicU32 = AtomicU32::new(0);

pub trait RcloneApi {
    fn delete_cache_path(
        &self,
        profile_name: &str,
        remote_path: &str,
    ) -> impl Future<Output = Result<String>>;

    fn list_profiles(&self) -> impl Future<Output = Result<Vec<(String, String)>>>;

    fn get_provider_options(
        &self,
        provider_type: &str,
    ) -> impl Future<Output = Result<Vec<serde_json::Value>>>;

    fn get_files_status(
        &self,
        profile_name: &str,
        paths: Vec<String>,
    ) -> impl Future<Output = Result<HashMap<String, String>>>;

    fn create_config(
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

    fn about(&self, profile_name: &str) -> impl Future<Output = Result<AboutResponse>>;
    fn list_available_providers(&self) -> impl Future<Output = Result<Vec<String>>>;
}

pub struct Rclone {
    pub client: Client,
    pub url: String,
}

impl Rclone {
    fn cleanup_auth_port() {
        let old_pid = AUTH_PID.swap(0, std::sync::atomic::Ordering::Relaxed);
        if old_pid != 0 {
            let _ = std::process::Command::new("kill")
                .arg("-9")
                .arg(old_pid.to_string())
                .status();
            println!("DEBUG: Killed hanging auth process with PID {}", old_pid);
        }
    }

    async fn delete_exists_profile(&self, profile_name: &str) -> Result<()> {
        let current_profiles = self.list_profiles().await?;
        if current_profiles
            .iter()
            .any(|(name, _)| name == profile_name)
        {
            println!("DEBUG: Deleting existing profile: {}", profile_name);
            let _ = self.delete_profile(profile_name).await?;
        }

        Ok(())
    }

    /// Переопределяем url для oauth облачных хранилищ, что пользователь мог выбирать аккаунты
    fn set_oauth_urls(&self, params: &mut HashMap<String, String>, domain: &str) {
        // TODO: доопределить все остальные сервисы с oauth
        let auth_url_override = match domain {
            "drive" => Some("https://accounts.google.com/o/oauth2/auth?prompt=select_account"),
            "dropbox" => Some(
                "https://www.dropbox.com/oauth2/authorize?force_reauthentication=true&force_reapprove=true",
            ),
            _ => None,
        };

        //  NOTE: Если для этого домена есть оверрайд, добавляем его в параметры для остальных
        //  rclone сам подставит дефолтный URL
        if let Some(url) = auth_url_override {
            params.insert("auth_url".to_string(), url.to_string());
        }
    }

    fn setup_create_config_args(
        &self,
        profile_name: &str,
        domain: &str,
        parameters: &str,
    ) -> Result<Vec<String>> {
        let mut params = serde_json::from_str::<CreateParameters>(parameters)?.into_string_map();
        let mut args = vec![
            "config".to_string(),
            "create".to_string(),
            profile_name.to_string(),
            domain.to_string(),
        ];

        self.set_oauth_urls(&mut params, domain);

        for (key, value) in params {
            args.push(key);
            args.push(value);
        }

        args.extend([
            "config_is_local".to_string(),
            "true".to_string(),
            "config_login_port".to_string(),
            "53682".to_string(),
            "--non-interactive".to_string(),
            "--quiet".to_string(),
        ]);
        Ok(args)
    }
}

impl RcloneApi for Rclone {
    async fn delete_cache_path(&self, profile_name: &str, remote_path: &str) -> Result<String> {
        let cache_base = format!(
            "{}/.cache/rclone/vfs/{}/",
            std::env::var("HOME").unwrap(),
            profile_name
        );
        let full_path = Path::new(&cache_base).join(remote_path);

        if full_path.exists() {
            if full_path.is_dir() {
                fs::remove_dir_all(&full_path)?;
            } else {
                fs::remove_file(&full_path)?;
            }

            if full_path.is_dir() {
                let _ = self.delete_cache_directory(profile_name, remote_path).await;
            } else {
                let _ = self.delete_cache_file(profile_name, remote_path).await;
            }

            Ok(format!(
                "Local cache for {} deleted. File will be downloaded again on request.",
                remote_path
            ))
        } else {
            Ok("File is not cached".to_string())
        }
    }

    async fn list_profiles(&self) -> Result<Vec<(String, String)>> {
        let response = self
            .client
            .post(format!("{}config/dump", self.url))
            .send()
            .await?;

        let data: HashMap<String, RemoteConfig> = response.json().await.map_err(|err| {
            CloudError::RcloneError(format!("Failed to parse providers: {}", err))
        })?;

        Ok(data
            .into_iter()
            .map(|(name, _type)| (name, _type.r#type))
            .collect())
    }

    async fn get_provider_options(&self, provider_type: &str) -> Result<Vec<serde_json::Value>> {
        let response = self
            .client
            .post(format!("{}config/providers", self.url))
            .send()
            .await?;

        let data: ProvidersResponse = response
            .json()
            .await
            .map_err(|e| CloudError::RcloneError(format!("Failed to parse providers: {}", e)))?;

        let provider = data
            .providers
            .into_iter()
            .find(|p| p.name == provider_type)
            .ok_or_else(|| {
                CloudError::RcloneError(format!("Provider '{}' not found", provider_type))
            })?;

        // Filter required and non-default options
        let filtered_options: Vec<serde_json::Value> = provider
            .options
            .into_iter()
            .filter(|opt| {
                !["token", "config_is_local", "config_login_port"].contains(&opt.name.as_str())
                    && opt.required
            })
            .map(|opt| {
                json!({
                    "Name": opt.name,
                    "Help": opt.help
                })
            })
            .collect();

        Ok(filtered_options)
    }

    /// Получение статуса файлов:
    ///
    /// # Arguments
    /// - `profile_name` - название хранилища
    /// - `paths` - относительные пути от корня хранилища до файлов
    async fn get_files_status(
        &self,
        profile_name: &str,
        paths: Vec<String>,
    ) -> Result<HashMap<String, String>> {
        let mut results = HashMap::new();
        let home = std::env::var("HOME").expect("HOME var should be init in every OS");

        let meta_base_path = std::path::Path::new(&home)
            .join(".cache/rclone/vfs")
            .join(profile_name);

        let core_stats_res = self
            .client
            .post(format!("{}core/stats", self.url))
            .send()
            .await;

        let active_transfers: Vec<String> = if let Ok(resp) = core_stats_res {
            let body: CoreStatsResponse = resp.json().await.unwrap_or_default();
            body.transferring.into_iter().map(|t| t.name).collect()
        } else {
            vec![]
        };

        for path in paths {
            let relative_path = path.trim_start_matches('/');

            if active_transfers.iter().any(|t| t.contains(relative_path)) {
                results.insert(path.clone(), "SYNCING".to_string());
                continue;
            }

            let meta_file = meta_base_path.join(relative_path);

            if meta_file.exists() {
                results.insert(path.clone(), "CACHED".to_string());
            } else {
                results.insert(path.clone(), "NOT_CACHED".to_string());
            }
        }

        Ok(results)
    }

    /// Создает профиль хранилища
    ///
    /// # Arguments
    /// - `profile_name` - название хранилища
    /// - `domain` - название типа хранилища (yandex, drive и тп)
    /// - `parameters` - дополнительные параметры авторизации
    async fn create_config(
        &self,
        profile_name: &str,
        domain: &str,
        parameters: &str,
    ) -> Result<String> {
        Self::cleanup_auth_port();
        self.delete_exists_profile(profile_name).await?;

        let args = self.setup_create_config_args(profile_name, domain, parameters)?;

        let mut child = Command::new("rclone")
            .args(&args)
            .kill_on_drop(true)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| CloudError::RcloneError(format!("Failed to spawn rclone: {}", e)))?;

        // Записали PID текущей попытки авторизации
        if let Some(pid) = child.id() {
            AUTH_PID.store(pid, std::sync::atomic::Ordering::Relaxed);
        }

        let result = match timeout(Duration::from_secs(600), child.wait()).await {
            Ok(Ok(status)) if status.success() => {
                Ok(format!("Profile '{}' created successfully", profile_name))
            }
            Ok(Ok(status)) => {
                println!("DEBUG: Rclone exited with error: {}", status);
                let _ = self.delete_profile(profile_name).await?;
                Err(CloudError::RcloneError(format!(
                    "Rclone failed with status: {}",
                    status
                )))
            }
            Ok(Err(e)) => Err(CloudError::RcloneError(format!("Wait error: {}", e))),
            Err(_) => {
                println!("DEBUG: Auth timeout reached for {}", profile_name);
                let _ = self.delete_profile(profile_name).await?;
                Err(CloudError::RcloneError("Authentication timed out".into()))
            }
        };

        if let Some(pid) = child.id() {
            let _ = AUTH_PID.compare_exchange(
                pid,
                0,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            );
        }

        result
    }

    /// Удаляет профиль хранилища по названию
    ///
    /// # Arguments
    /// - `profile_name` - название хранилища
    async fn delete_profile(&self, profile_name: &str) -> Result<String> {
        let body = HashMap::from([("name", profile_name)]);

        self.client
            .post(format!("{}config/delete", self.url))
            .json(&body)
            .send()
            .await?;

        Ok(format!("Success: Profile {} deleted", profile_name))
    }

    /// Монтирует удаленное хранилище в локальную файловую систему.
    ///
    /// # Arguments
    /// - `profile_name` - название хранилища
    /// - `path` - полный путь, где нужно примонтировать хранилище
    /// - `cache_max_size` - максимальный размер кэша на данный mount
    /// - `cache_max_age` - Время, которое файл хранится на диске после последнего
    ///   чтения/записи до вытеснения из кэша VFS.
    async fn mount(
        &self,
        profile_name: &str,
        file_path: &str,
        cache_max_size: &str,
        cache_max_age: &str,
    ) -> Result<String> {
        let mount_path = std::path::Path::new(file_path).join(profile_name);
        std::fs::create_dir_all(&mount_path)?;
        let mount_path_str = mount_path.to_string_lossy().to_string();

        let cache_max_size = format!(
            "{}G",
            cache_max_size
                .to_lowercase()
                .parse::<u32>()
                .map_err(|err| CloudError::ConvertError(format!("Convert error: {}", err)))?
        );

        let cache_max_age = format!(
            "{}h",
            cache_max_age
                .to_lowercase()
                .parse::<u32>()
                .map_err(|err| CloudError::ConvertError(format!("Convert error: {}", err)))?
        );

        let body = json!({
            "fs": format!("{}:", profile_name),
            "mountPoint": mount_path_str,
            "vfsOpt": {
                "CacheMode": "full",
                "CacheMaxAge": &cache_max_age,
                "CacheMaxSize": &cache_max_size,
                "DirCacheTime": "9999h",
                "NoChecksum": false,
                "NoModTime": false,
            }
        });

        println!("{body}");

        let response = self
            .client
            .post(format!("{}mount/mount", self.url))
            .json(&body)
            .send()
            .await?;

        if response.status().is_success() {
            setup_conf_dir::setup(profile_name, file_path)?;
            Ok(format!("Mounting {} started", profile_name))
        } else {
            Err(CloudError::RcloneError("Failed to mount".into()))
        }
    }

    /// Создает ссылку на просмотр на файл/директорию из хранилища
    ///
    /// # Arguments
    /// - `profile_name` - название хранилища
    /// - `path` - относительный путь к файлу/директории внутри хранилища.
    async fn link(&self, profile_name: &str, path: &str) -> Result<String> {
        let body = HashMap::from([
            ("fs", profile_name.to_string() + ":"),
            ("remote", path.to_string()),
        ]);

        let response = self
            .client
            .post(format!("{}operations/publiclink", self.url))
            .json(&body)
            .send()
            .await?;

        let res_json: serde_json::Value = response.json().await?;

        println!("Rclone link response: {:?}", res_json);

        match res_json["url"].as_str() {
            Some(url) => Ok(url.to_string()),
            None => Err(CloudError::RcloneError("No link generated".to_string())),
        }
    }

    /// Рекурсивно кэширует директорию с удаленного хранилища
    ///
    /// # Arguments
    /// - `path` - полный путь до директории в хранилище
    async fn cache_directory(&self, path: &str) -> Result<String> {
        let mut file_paths = Vec::new();

        // 1. Собираем все файлы рекурсивно
        get_all_files(Path::new(path), &mut file_paths);

        if file_paths.is_empty() {
            println!("Файлы не найдены.");
            return Ok("Empty dir".to_string());
        }

        let _ = Command::new("cat")
            .args(&file_paths)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?;

        Ok("Cached".to_string())
    }

    /// Обновляет данные с удаленного хранилища
    ///
    /// # Arguments
    /// - `profile_name` - название хранилища
    /// - `path` - относительный путь в хранилище
    async fn refresh(&self, profile_name: &str, path: &str) -> Result<String> {
        let body = json!({
            "fs": format!("{}:", profile_name),
            "dir": path,
            "_async": true,
            "recursive": true,
        });

        let response = self
            .client
            .post(format!("{}vfs/refresh", self.url))
            .json(&body)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(format!("Success: File {} cached", path))
        } else {
            Err(CloudError::RcloneError("Failed to cache file".into()))
        }
    }

    async fn delete_cache_file(&self, profile_name: &str, path: &str) -> Result<String> {
        let body = json!({
            "fs": format!("{}:", profile_name),
            "file": path,
        });

        let response = self
            .client
            .post(format!("{}vfs/forget", self.url))
            .json(&body)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(format!("Success: {} evicted from local cache", path))
        } else {
            Err(CloudError::RcloneError("Failed to evict from cache".into()))
        }
    }

    async fn delete_cache_directory(&self, profile_name: &str, path: &str) -> Result<String> {
        let body = json!({
            "fs": format!("{}:", profile_name),
            "dir": path,
        });

        let response = self
            .client
            .post(format!("{}vfs/forget", self.url))
            .json(&body)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(format!("Success: {} evicted from local cache", path))
        } else {
            Err(CloudError::RcloneError("Failed to evict from cache".into()))
        }
    }

    /// Получает информацию о доступном и занятом месте в хранилище.
    async fn about(&self, profile_name: &str) -> Result<AboutResponse> {
        let body = json!({
            "fs": format!("{}:", profile_name),
        });

        let response = self
            .client
            .post(format!("{}operations/about", self.url))
            .json(&body)
            .send()
            .await?;

        let data: AboutResponse = response.json().await?;

        Ok(data)
    }

    /// Возвращает список всех поддерживаемых провайдеров rclone
    async fn list_available_providers(&self) -> Result<Vec<String>> {
        let response = self
            .client
            .post(format!("{}config/providers", self.url))
            .send()
            .await?;

        let data: ProvidersResponse = response.json().await?;

        Ok(data.providers.into_iter().map(|p| p.name).collect())
    }
}
