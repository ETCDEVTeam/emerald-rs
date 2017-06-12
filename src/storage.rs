//! # Chain-related storage

use log::LogLevel;
use std::{env, fs};
use std::io::Error;
use std::path::PathBuf;

/// Base dir for internal data, all chain-related should be store in subdirectories
#[derive(Debug, Clone)]
pub struct Storages {
    /// base dir
    base_dir: PathBuf,
}

/// Default path (*nix)
#[cfg(all(unix, not(target_os = "macos"), not(target_os = "ios"), not(target_os = "android")))]
pub fn default_path() -> PathBuf {
    let mut config_dir = env::home_dir().expect("Expect path to home dir");
    config_dir.push(".emerald");
    config_dir
}

/// Default path (Mac OS X)
#[cfg(target_os = "macos")]
pub fn default_path() -> PathBuf {
    let mut config_dir = env::home_dir().expect("Expect path to home dir");
    config_dir.push("Library");
    config_dir.push("Emerald");
    config_dir
}

/// Default path (Windows OS)
#[cfg(target_os = "windows")]
pub fn default_path() -> PathBuf {
    let app_data_var = env::var("APPDATA").expect("Expect 'APPDATA' environment variable");
    let mut config_dir = PathBuf::from(app_data_var);
    config_dir.push(".emerald");
    config_dir
}

/// Default path for `Keystore` files
pub fn default_keystore_path(chain_id: &str) -> PathBuf {
    let mut path = default_path();
    path.push(chain_id);
    path.push("keystore");
    path
}

/// Default path for log files.
pub fn default_log_path(chain_id: &str) -> PathBuf {
    let mut path = default_path();
    path.push(chain_id);
    path.push("log");
    path
}

impl Storages {
    /// Create storage using user directory if specified, or default path in other case.
    pub fn new(path: PathBuf) -> Storages {
        Storages { base_dir: path }
    }

    /// Initialize new storage
    pub fn init(&self) -> Result<(), Error> {
        if !&self.base_dir.exists() {
            if log_enabled!(LogLevel::Info) {
                info!("Init new storage at {}", self.base_dir.display());
            }
            fs::create_dir(self.base_dir.as_path())?
        }
        Ok(())
    }
}

impl Default for Storages {
    fn default() -> Self {
        Storages { base_dir: default_path() }
    }
}

/// Subdir for a chain
#[derive(Debug, Clone)]
pub struct ChainStorage<'a> {
    /// subdir name
    pub id: String,
    /// storage
    base: &'a Storages,
}

impl<'a> ChainStorage<'a> {
    /// Crate a new chain
    pub fn new(base: &'a Storages, id: String) -> ChainStorage<'a> {
        ChainStorage { id: id, base: base }
    }

    /// Initialize a new chain
    pub fn init(&self) -> Result<(), Error> {
        let mut p: PathBuf = self.base.base_dir.to_path_buf();
        p.push(self.id.clone());
        if !p.exists() {
            if log_enabled!(LogLevel::Info) {
                info!("Init new chain at {}", p.display());
            }
            fs::create_dir(p)?
        }

        let ks_path = default_keystore_path(&self.id);
        if !ks_path.exists() {
            fs::create_dir(ks_path.as_path())?
        }

        let log_path = default_log_path(&self.id);
        if !log_path.exists() {
            fs::create_dir(log_path.as_path())?
        }

        Ok(())
    }

    /// Get chain path
    pub fn get_path(&self, id: String) -> Result<PathBuf, Error> {
        let mut p: PathBuf = self.base.base_dir.to_path_buf().clone();
        p.push(self.id.clone());
        p.push(id.clone());
        if !p.exists() {
            if log_enabled!(LogLevel::Debug) {
                debug!("Init new chain storage at {}", p.display());
            }
            fs::create_dir(&p)?
        }
        Ok(p)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn should_use_default_path() {
        let st = Storages::default();
        assert_eq!(st.base_dir, default_path());
    }

    #[test]
    fn should_use_user_path() {
        let user_path: &str = "/tmp/some";
        let st = Storages::new(PathBuf::from(user_path));

        assert_eq!(st.base_dir, PathBuf::from(user_path));
    }
}
