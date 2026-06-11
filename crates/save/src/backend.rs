//! Save backends: filesystem now, LocalStorage at P7 (doc 03 §9).

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::model::SaveError;

pub trait SaveBackend {
    fn write(&mut self, name: &str, contents: &str) -> Result<(), SaveError>;
    fn read(&self, name: &str) -> Result<Option<String>, SaveError>;
}

/// Filesystem backend rooted at the platform save dir
/// (`ProjectDirs("com", "turboot", "undersong")/saves`, doc 03 §4).
pub struct FsBackend {
    dir: PathBuf,
}

impl FsBackend {
    /// Platform-default location. `None` only on exotic systems with no
    /// home directory at all.
    pub fn platform_default() -> Option<Self> {
        directories::ProjectDirs::from("com", "turboot", "undersong")
            .map(|dirs| Self::at(dirs.data_dir().join("saves")))
    }

    /// Explicit root — tests and the future replay harness use this.
    pub fn at(dir: PathBuf) -> Self {
        Self { dir }
    }
}

impl SaveBackend for FsBackend {
    fn write(&mut self, name: &str, contents: &str) -> Result<(), SaveError> {
        std::fs::create_dir_all(&self.dir).map_err(|e| SaveError::Io(e.to_string()))?;
        // Write-then-rename so a crash mid-write can't truncate the only
        // copy of a player's save.
        let tmp = self.dir.join(format!("{name}.tmp"));
        let target = self.dir.join(name);
        {
            use std::io::Write;
            let mut file = std::fs::File::create(&tmp).map_err(|e| SaveError::Io(e.to_string()))?;
            file.write_all(contents.as_bytes())
                .map_err(|e| SaveError::Io(e.to_string()))?;
            // Flush to disk before the rename so a crash can't leave the
            // slot pointing at a half-written file.
            file.sync_all().map_err(|e| SaveError::Io(e.to_string()))?;
        }
        std::fs::rename(&tmp, &target).map_err(|e| SaveError::Io(e.to_string()))
    }

    fn read(&self, name: &str) -> Result<Option<String>, SaveError> {
        match std::fs::read_to_string(self.dir.join(name)) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(SaveError::Io(e.to_string())),
        }
    }
}

/// In-memory backend for tests (and the shape of the web backend later).
#[derive(Debug, Default)]
pub struct MemBackend {
    files: BTreeMap<String, String>,
}

impl SaveBackend for MemBackend {
    fn write(&mut self, name: &str, contents: &str) -> Result<(), SaveError> {
        self.files.insert(name.to_string(), contents.to_string());
        Ok(())
    }

    fn read(&self, name: &str) -> Result<Option<String>, SaveError> {
        Ok(self.files.get(name).cloned())
    }
}

/// Browser backend: one `localStorage` key per save file, namespaced
/// `undersong.<name>` (doc 03 §4; P7 WASM target). Compiled only for
/// wasm32 — the native build never links web-sys.
#[cfg(target_arch = "wasm32")]
pub struct LocalStorageBackend;

#[cfg(target_arch = "wasm32")]
impl SaveBackend for LocalStorageBackend {
    fn write(&mut self, name: &str, contents: &str) -> Result<(), SaveError> {
        let storage = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .ok_or_else(|| SaveError::Io("localStorage unavailable".into()))?;
        storage
            .set_item(&format!("undersong.{name}"), contents)
            .map_err(|_| SaveError::Io("localStorage write failed (quota?)".into()))
    }

    fn read(&self, name: &str) -> Result<Option<String>, SaveError> {
        let storage = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .ok_or_else(|| SaveError::Io("localStorage unavailable".into()))?;
        storage
            .get_item(&format!("undersong.{name}"))
            .map_err(|_| SaveError::Io("localStorage read failed".into()))
    }
}
