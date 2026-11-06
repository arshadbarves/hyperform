//! The pet's growth, `<data dir>/pet.json`. The webview owns the rules
//! (`ui/src/lib/domain/pet/growth.ts`); this only keeps the JSON between
//! launches, written through a temp file so a crash cannot truncate it.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::ipc::{IpcResult, fail};

pub(crate) const FILE_NAME: &str = "pet.json";
/// Growth is a few counters and the last counted ids; anything larger is refused.
const MAX_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct PetStore {
    file: PathBuf,
}

impl PetStore {
    pub(crate) fn new(data_dir: &Path) -> Self {
        Self {
            file: data_dir.join(FILE_NAME),
        }
    }

    /// The saved growth, or null when there is none or it cannot be read.
    pub(crate) fn load(&self) -> Value {
        std::fs::read_to_string(&self.file)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .filter(Value::is_object)
            .unwrap_or(Value::Null)
    }

    pub(crate) fn save(&self, growth: &Value) -> IpcResult<()> {
        if !growth.is_object() {
            return Err("the pet's growth must be an object".to_string());
        }
        let text = serde_json::to_string_pretty(growth).map_err(fail)?;
        if text.len() > MAX_BYTES {
            return Err(format!("the pet's growth is over {MAX_BYTES} bytes"));
        }
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let tmp = self.file.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(fail)?;
        std::fs::rename(&tmp, &self.file).map_err(fail)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn growth_round_trips_and_bad_input_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = PetStore::new(dir.path());
        assert_eq!(store.load(), Value::Null);

        let growth = json!({ "version": 1, "xp": 60, "counted": ["turn:t1"] });
        store.save(&growth).unwrap();
        assert_eq!(store.load(), growth);
        assert!(!dir.path().join("pet.json.tmp").exists());

        assert!(store.save(&json!([1, 2])).is_err());
        let huge = json!({ "counted": vec!["x".repeat(100); 1000] });
        assert!(store.save(&huge).is_err());
        assert_eq!(store.load(), growth, "a refused save leaves the file alone");

        std::fs::write(dir.path().join(FILE_NAME), "not json").unwrap();
        assert_eq!(store.load(), Value::Null);
    }
}
