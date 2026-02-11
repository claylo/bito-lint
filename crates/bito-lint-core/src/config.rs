//! Configuration loading and discovery.
//!
//! This module provides configuration file discovery by:
//! 1. Walking up from the current directory to find project config
//! 2. Loading user config from XDG config directory
//! 3. Merging with sensible defaults
//!
//! # Supported formats
//!
//! The following configuration file formats are supported:
//! - TOML (`.toml`)
//! - YAML (`.yaml`, `.yml`)
//! - JSON (`.json`)
//!
//! # Config file locations (in order of precedence, highest first):
//! - `.bito-lint.<ext>` in current directory or any parent
//! - `bito-lint.<ext>` in current directory or any parent
//! - `~/.config/bito-lint/config.<ext>` (user config)
//!
//! Where `<ext>` is one of: `toml`, `yaml`, `yml`, `json`
//!
//! # Example
//! ```no_run
//! use camino::Utf8PathBuf;
//! use bito_lint_core::config::{Config, ConfigLoader};
//!
//! let cwd = std::env::current_dir().unwrap();
//! let cwd = Utf8PathBuf::try_from(cwd).expect("current directory is not valid UTF-8");
//! let config = ConfigLoader::new()
//!     .with_project_search(&cwd)
//!     .load()
//!     .unwrap();
//! ```

use std::collections::HashMap;

use camino::{Utf8Path, Utf8PathBuf};
use figment::Figment;
use figment::providers::{Env, Format, Json, Serialized, Toml, Yaml};
use serde::{Deserialize, Serialize};

use crate::error::{ConfigError, ConfigResult};
use crate::tokens::Backend;

/// English dialect for spelling conventions.
///
/// When set, the consistency checker enforces the chosen dialect's spelling
/// (e.g., "color" vs "colour") in addition to detecting mixed usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum Dialect {
    /// American English (color, center, organize, defense).
    #[cfg_attr(feature = "clap", value(name = "en-us"))]
    EnUs,
    /// British English (colour, centre, organise, defence).
    #[cfg_attr(feature = "clap", value(name = "en-gb"))]
    EnGb,
    /// Canadian English (colour, centre, organize, defence — hybrid).
    #[cfg_attr(feature = "clap", value(name = "en-ca"))]
    EnCa,
    /// Australian English (colour, centre, organise, defence — follows GB).
    #[cfg_attr(feature = "clap", value(name = "en-au"))]
    EnAu,
}

impl Dialect {
    /// Returns the dialect as a BCP-47-style tag.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::EnUs => "en-us",
            Self::EnGb => "en-gb",
            Self::EnCa => "en-ca",
            Self::EnAu => "en-au",
        }
    }
}

impl std::fmt::Display for Dialect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The configuration for bito-lint.
///
/// Add your configuration fields here. This struct is deserialized from
/// config files found during discovery (TOML, YAML, or JSON).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Log level for the application (e.g., "debug", "info", "warn", "error").
    pub log_level: LogLevel,
    /// Directory for JSONL log files (falls back to platform defaults if unset).
    pub log_dir: Option<Utf8PathBuf>,
    /// Default token budget for the `tokens` command.
    pub token_budget: Option<usize>,
    /// Default maximum Flesch-Kincaid grade level for the `readability` command.
    pub max_grade: Option<f64>,
    /// Default maximum passive voice percentage for the `grammar` command.
    pub passive_max_percent: Option<f64>,
    /// Default minimum style score for the `analyze` command.
    pub style_min_score: Option<i32>,
    /// English dialect for spelling enforcement (en-us, en-gb, en-ca, en-au).
    pub dialect: Option<Dialect>,
    /// Maximum input size in bytes (default: 5 MiB).
    ///
    /// Prevents resource exhaustion from oversized inputs in both CLI and MCP server.
    /// Set to `null` / omit to disable the limit.
    pub max_input_bytes: Option<usize>,
    /// Tokenizer backend (claude or openai). Defaults to claude.
    pub tokenizer: Option<Backend>,
    /// Custom completeness templates (name → required section headings).
    ///
    /// These extend (not replace) the built-in templates (adr, handoff, design-doc).
    /// If a custom template name collides with a built-in, the custom one wins.
    pub templates: Option<HashMap<String, Vec<String>>>,
}

/// Log level configuration.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Verbose output for debugging and development.
    Debug,
    /// Standard operational information (default).
    #[default]
    Info,
    /// Warnings about potential issues.
    Warn,
    /// Errors that indicate failures.
    Error,
}

impl LogLevel {
    /// Returns the log level as a lowercase string slice.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// Supported configuration file extensions (in order of preference).
const CONFIG_EXTENSIONS: &[&str] = &["toml", "yaml", "yml", "json"];

/// Application name for XDG directory lookup and config file names.
const APP_NAME: &str = "bito-lint";

/// Builder for loading configuration from multiple sources.
#[derive(Debug, Default)]
pub struct ConfigLoader {
    /// Starting directory for project config search.
    project_search_root: Option<Utf8PathBuf>,
    /// Whether to include user config from XDG directory.
    include_user_config: bool,
    /// Stop searching when we hit a directory containing this file/dir.
    boundary_marker: Option<String>,
    /// Explicit config files to load (for testing or programmatic use).
    explicit_files: Vec<Utf8PathBuf>,
}

impl ConfigLoader {
    /// Create a new config loader with default settings.
    pub fn new() -> Self {
        Self {
            project_search_root: None,
            include_user_config: true,
            boundary_marker: Some(".git".to_string()),
            explicit_files: Vec::new(),
        }
    }

    /// Set the starting directory for project config search.
    ///
    /// The loader will walk up from this directory looking for config files.
    pub fn with_project_search<P: AsRef<Utf8Path>>(mut self, path: P) -> Self {
        self.project_search_root = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set whether to include user config from `~/.config/bito-lint/`.
    pub const fn with_user_config(mut self, include: bool) -> Self {
        self.include_user_config = include;
        self
    }

    /// Set a boundary marker to stop directory traversal.
    ///
    /// When walking up directories, stop if we find a directory containing
    /// this file or directory name. Default is `.git`.
    pub fn with_boundary_marker<S: Into<String>>(mut self, marker: S) -> Self {
        self.boundary_marker = Some(marker.into());
        self
    }

    /// Disable boundary marker (search all the way to filesystem root).
    pub fn without_boundary_marker(mut self) -> Self {
        self.boundary_marker = None;
        self
    }

    /// Add an explicit config file to load.
    ///
    /// Files are loaded in order, with later files taking precedence.
    /// Explicit files are loaded after discovered files.
    pub fn with_file<P: AsRef<Utf8Path>>(mut self, path: P) -> Self {
        self.explicit_files.push(path.as_ref().to_path_buf());
        self
    }

    /// Load configuration, merging all discovered sources.
    ///
    /// Precedence (highest to lowest):
    /// 1. Explicit files (in order added via `with_file`)
    /// 2. Project config (closest to search root)
    /// 3. User config (`~/.config/bito-lint/config.<ext>`)
    /// 4. Default values
    #[tracing::instrument(skip(self), fields(search_root = ?self.project_search_root))]
    pub fn load(self) -> ConfigResult<Config> {
        tracing::debug!("loading configuration");
        let mut figment = Figment::new().merge(Serialized::defaults(Config::default()));

        // Start with user config (lowest precedence of file sources)
        if self.include_user_config
            && let Some(user_config) = self.find_user_config()
        {
            figment = Self::merge_file(figment, &user_config);
        }

        // Add project config
        if let Some(ref root) = self.project_search_root
            && let Some(project_config) = self.find_project_config(root)
        {
            figment = Self::merge_file(figment, &project_config);
        }

        // Add explicit files
        for file in &self.explicit_files {
            figment = Self::merge_file(figment, file);
        }

        // Environment variables (highest precedence)
        // BITO_LINT_DIALECT=en-gb, BITO_LINT_LOG_LEVEL=debug, etc.
        figment = figment.merge(Env::prefixed("BITO_LINT_").lowercase(true));

        let config: Config = figment
            .extract()
            .map_err(|e| ConfigError::Deserialize(Box::new(e)))?;
        tracing::info!(
            log_level = config.log_level.as_str(),
            "configuration loaded"
        );
        Ok(config)
    }

    /// Load configuration, returning an error if no config file is found.
    pub fn load_or_error(self) -> ConfigResult<Config> {
        let has_user = self.include_user_config && self.find_user_config().is_some();
        let has_project = self
            .project_search_root
            .as_ref()
            .and_then(|root| self.find_project_config(root))
            .is_some();
        let has_explicit = !self.explicit_files.is_empty();

        if !has_user && !has_project && !has_explicit {
            return Err(ConfigError::NotFound);
        }

        self.load()
    }

    /// Find project config by walking up from the given directory.
    fn find_project_config(&self, start: &Utf8Path) -> Option<Utf8PathBuf> {
        let mut current = Some(start.to_path_buf());

        while let Some(dir) = current {
            // Check for boundary marker
            if let Some(ref marker) = self.boundary_marker {
                let marker_path = dir.join(marker);
                if marker_path.exists() && dir != start {
                    // Found boundary in a parent dir, stop searching
                    break;
                }
            }

            // Check for config files in this directory (try each extension)
            for ext in CONFIG_EXTENSIONS {
                // Try dotfile first (.bito-lint.toml)
                let dotfile = dir.join(format!(".{APP_NAME}.{ext}"));
                if dotfile.is_file() {
                    return Some(dotfile);
                }

                // Then try regular name (bito-lint.toml)
                let regular = dir.join(format!("{APP_NAME}.{ext}"));
                if regular.is_file() {
                    return Some(regular);
                }
            }

            current = dir.parent().map(Utf8Path::to_path_buf);
        }

        None
    }

    /// Find user config in XDG config directory.
    fn find_user_config(&self) -> Option<Utf8PathBuf> {
        let proj_dirs = directories::ProjectDirs::from("", "", APP_NAME)?;
        let config_dir = proj_dirs.config_dir();

        // Try each supported extension
        for ext in CONFIG_EXTENSIONS {
            let config_path = config_dir.join(format!("config.{ext}"));
            if config_path.is_file() {
                return Utf8PathBuf::from_path_buf(config_path).ok();
            }
        }

        None
    }

    /// Merge a config file into the figment, detecting format from extension.
    fn merge_file(figment: Figment, path: &Utf8Path) -> Figment {
        match path.extension() {
            Some("toml") => figment.merge(Toml::file_exact(path.as_str())),
            Some("yaml" | "yml") => figment.merge(Yaml::file_exact(path.as_str())),
            Some("json") => figment.merge(Json::file_exact(path.as_str())),
            _ => figment.merge(Toml::file_exact(path.as_str())),
        }
    }
}

/// Find the project config file path without loading it.
///
/// Useful for commands that need to know where config is located.
pub fn find_project_config<P: AsRef<Utf8Path>>(start: P) -> Option<Utf8PathBuf> {
    ConfigLoader::new()
        .with_project_search(start.as_ref())
        .without_boundary_marker()
        .find_project_config(start.as_ref())
}

/// Get the project directories for XDG-compliant path resolution.
///
/// Returns `None` if the home directory cannot be determined.
fn project_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("", "", APP_NAME)
}

/// Get the user config directory path.
///
/// Returns `~/.config/bito-lint/` on Linux, `~/Library/Application Support/bito-lint/`
/// on macOS, and equivalent on other platforms.
pub fn user_config_dir() -> Option<Utf8PathBuf> {
    let proj_dirs = project_dirs()?;
    Utf8PathBuf::from_path_buf(proj_dirs.config_dir().to_path_buf()).ok()
}

/// Get the user cache directory path.
///
/// Returns `~/.cache/bito-lint/` on Linux, `~/Library/Caches/bito-lint/`
/// on macOS, and equivalent on other platforms.
pub fn user_cache_dir() -> Option<Utf8PathBuf> {
    let proj_dirs = project_dirs()?;
    Utf8PathBuf::from_path_buf(proj_dirs.cache_dir().to_path_buf()).ok()
}

/// Get the user data directory path.
///
/// Returns `~/.local/share/bito-lint/` on Linux, `~/Library/Application Support/bito-lint/`
/// on macOS, and equivalent on other platforms.
pub fn user_data_dir() -> Option<Utf8PathBuf> {
    let proj_dirs = project_dirs()?;
    Utf8PathBuf::from_path_buf(proj_dirs.data_dir().to_path_buf()).ok()
}

/// Get the local data directory path (machine-specific, not synced).
///
/// Returns `~/.local/share/bito-lint/` on Linux, `~/Library/Application Support/bito-lint/`
/// on macOS, and equivalent on other platforms.
pub fn user_data_local_dir() -> Option<Utf8PathBuf> {
    let proj_dirs = project_dirs()?;
    Utf8PathBuf::from_path_buf(proj_dirs.data_local_dir().to_path_buf()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// Serializes tests that mutate environment variables via `set_var`/`remove_var`.
    /// Prevents race conditions when nextest runs tests in the same binary concurrently.
    static TEST_ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.log_level, LogLevel::Info);
        assert!(config.log_dir.is_none());
    }

    #[test]
    fn test_loader_builds_with_defaults() {
        let loader = ConfigLoader::new()
            .with_user_config(false)
            .without_boundary_marker();

        // Should succeed with defaults even if no files found
        let config = loader.load().unwrap();
        assert_eq!(config.log_level, LogLevel::Info);
    }

    #[test]
    fn test_single_file_overrides_default() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        fs::write(
            &config_path,
            r#"log_level = "debug"
log_dir = "/tmp/bito-lint"
"#,
        )
        .unwrap();

        // Convert to Utf8PathBuf for API call
        let config_path = Utf8PathBuf::try_from(config_path).unwrap();

        let config = ConfigLoader::new()
            .with_user_config(false)
            .with_file(&config_path)
            .load()
            .unwrap();

        assert_eq!(config.log_level, LogLevel::Debug);
        assert_eq!(
            config.log_dir.as_ref().map(|dir| dir.as_str()),
            Some("/tmp/bito-lint")
        );
    }

    #[test]
    fn test_later_file_overrides_earlier() {
        let tmp = TempDir::new().unwrap();

        let base_config = tmp.path().join("base.toml");
        fs::write(&base_config, r#"log_level = "warn""#).unwrap();

        let override_config = tmp.path().join("override.toml");
        fs::write(&override_config, r#"log_level = "error""#).unwrap();

        // Convert to Utf8PathBuf for API calls
        let base_config = Utf8PathBuf::try_from(base_config).unwrap();
        let override_config = Utf8PathBuf::try_from(override_config).unwrap();

        let config = ConfigLoader::new()
            .with_user_config(false)
            .with_file(&base_config)
            .with_file(&override_config)
            .load()
            .unwrap();

        // Later file wins
        assert_eq!(config.log_level, LogLevel::Error);
    }

    #[test]
    fn test_project_config_discovery() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("project");
        let sub_dir = project_dir.join("src").join("deep");
        fs::create_dir_all(&sub_dir).unwrap();

        // Create config in project root
        let config_path = project_dir.join(".bito-lint.toml");
        fs::write(&config_path, r#"log_level = "debug""#).unwrap();

        // Convert to Utf8PathBuf for API call
        let sub_dir = Utf8PathBuf::try_from(sub_dir).unwrap();

        // Search from deep subdirectory
        let config = ConfigLoader::new()
            .with_user_config(false)
            .without_boundary_marker()
            .with_project_search(&sub_dir)
            .load()
            .unwrap();

        assert_eq!(config.log_level, LogLevel::Debug);
    }

    #[test]
    fn test_boundary_marker_stops_search() {
        let tmp = TempDir::new().unwrap();

        // Create structure: /parent/config.toml, /parent/child/.git/, /parent/child/work/
        let parent = tmp.path().join("parent");
        let child = parent.join("child");
        let work = child.join("work");
        fs::create_dir_all(&work).unwrap();

        // Config in parent (should NOT be found due to .git boundary)
        fs::write(parent.join(".bito-lint.toml"), r#"log_level = "warn""#).unwrap();

        // .git marker in child
        fs::create_dir(child.join(".git")).unwrap();

        // Convert to Utf8PathBuf for API call
        let work = Utf8PathBuf::try_from(work).unwrap();

        // Search from work directory - should not find parent config
        let config = ConfigLoader::new()
            .with_user_config(false)
            .with_boundary_marker(".git")
            .with_project_search(&work)
            .load()
            .unwrap();

        // Should get default since config is beyond boundary
        assert_eq!(config.log_level, LogLevel::Info);
    }

    #[test]
    fn test_explicit_file_overrides_project_config() {
        let tmp = TempDir::new().unwrap();

        // Project config
        let project_config = tmp.path().join(".bito-lint.toml");
        fs::write(&project_config, r#"log_level = "warn""#).unwrap();

        // Explicit override
        let override_config = tmp.path().join("override.toml");
        fs::write(&override_config, r#"log_level = "error""#).unwrap();

        // Convert to Utf8PathBuf for API calls
        let tmp_path = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();
        let override_config = Utf8PathBuf::try_from(override_config).unwrap();

        let config = ConfigLoader::new()
            .with_user_config(false)
            .without_boundary_marker()
            .with_project_search(&tmp_path)
            .with_file(&override_config)
            .load()
            .unwrap();

        // Explicit file wins over project config
        assert_eq!(config.log_level, LogLevel::Error);
    }

    #[test]
    fn test_load_or_error_fails_when_no_config() {
        let result = ConfigLoader::new()
            .with_user_config(false)
            .without_boundary_marker()
            .load_or_error();

        assert!(matches!(result, Err(ConfigError::NotFound)));
    }

    #[test]
    fn test_load_or_error_succeeds_with_explicit_file() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        fs::write(&config_path, r#"log_level = "debug""#).unwrap();

        // Convert to Utf8PathBuf for API call
        let config_path = Utf8PathBuf::try_from(config_path).unwrap();

        let config = ConfigLoader::new()
            .with_user_config(false)
            .with_file(&config_path)
            .load_or_error()
            .unwrap();

        assert_eq!(config.log_level, LogLevel::Debug);
    }

    #[test]
    fn test_user_config_dir() {
        // Should return Some on most systems
        let dir = user_config_dir();
        if let Some(path) = dir {
            assert!(path.as_str().contains("bito-lint"));
        }
    }

    #[test]
    fn test_dialect_deserialization_toml() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        fs::write(&config_path, "dialect = \"en-gb\"\n").unwrap();

        let config_path = Utf8PathBuf::try_from(config_path).unwrap();

        let config = ConfigLoader::new()
            .with_user_config(false)
            .with_file(&config_path)
            .load()
            .unwrap();

        assert_eq!(config.dialect, Some(Dialect::EnGb));
    }

    #[test]
    fn test_dialect_deserialization_all_variants() {
        for (input, expected) in [
            ("en-us", Dialect::EnUs),
            ("en-gb", Dialect::EnGb),
            ("en-ca", Dialect::EnCa),
            ("en-au", Dialect::EnAu),
        ] {
            let tmp = TempDir::new().unwrap();
            let config_path = tmp.path().join("config.toml");
            fs::write(&config_path, format!("dialect = \"{input}\"\n")).unwrap();

            let config_path = Utf8PathBuf::try_from(config_path).unwrap();

            let config = ConfigLoader::new()
                .with_user_config(false)
                .with_file(&config_path)
                .load()
                .unwrap();

            assert_eq!(config.dialect, Some(expected), "failed for {input}");
        }
    }

    #[test]
    fn test_dialect_default_is_none() {
        let config = Config::default();
        assert!(config.dialect.is_none());
    }

    #[test]
    fn test_dialect_as_str() {
        assert_eq!(Dialect::EnUs.as_str(), "en-us");
        assert_eq!(Dialect::EnGb.as_str(), "en-gb");
        assert_eq!(Dialect::EnCa.as_str(), "en-ca");
        assert_eq!(Dialect::EnAu.as_str(), "en-au");
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_env_var_override_dialect() {
        let _lock = TEST_ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        // SAFETY: Test environment — mutex serializes env access across tests.
        unsafe {
            std::env::set_var("BITO_LINT_DIALECT", "en-ca");
        }

        let config = ConfigLoader::new()
            .with_user_config(false)
            .without_boundary_marker()
            .load()
            .unwrap();

        assert_eq!(config.dialect, Some(Dialect::EnCa));

        // SAFETY: Cleanup after test.
        unsafe {
            std::env::remove_var("BITO_LINT_DIALECT");
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_env_var_overrides_file_config() {
        let _lock = TEST_ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        fs::write(&config_path, "dialect = \"en-us\"\n").unwrap();

        let config_path = Utf8PathBuf::try_from(config_path).unwrap();

        // SAFETY: Test environment — mutex serializes env access across tests.
        unsafe {
            std::env::set_var("BITO_LINT_DIALECT", "en-au");
        }

        let config = ConfigLoader::new()
            .with_user_config(false)
            .with_file(&config_path)
            .load()
            .unwrap();

        assert_eq!(config.dialect, Some(Dialect::EnAu));

        // SAFETY: Cleanup after test.
        unsafe {
            std::env::remove_var("BITO_LINT_DIALECT");
        }
    }
}
