use crate::domain::errors::KVStoreError;
use crate::ports::outbound::{BatchOperation, KeyValueStore, ScanResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// File-backed key-value store for production without RocksDB.
///
/// Persists data to a binary file on disk, providing durability without
/// requiring RocksDB compilation. Suitable for development and light production.
pub struct FileBackedKVStore {
    data: HashMap<Vec<u8>, Vec<u8>>,
    path: PathBuf,
}

impl FileBackedKVStore {
    /// Create a new file-backed store at the given path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref().to_path_buf();

        // Check if file exists and its size
        #[allow(unused_variables)]
        if let Ok(metadata) = std::fs::metadata(&path) {
            #[cfg(feature = "tracing-log")]
            tracing::info!(
                "[qc-02] 💾 Found existing storage file: {} ({} bytes)",
                path.display(),
                metadata.len()
            );
        } else {
            #[cfg(feature = "tracing-log")]
            tracing::info!("[qc-02] 📁 No existing storage file at {}", path.display());
        }

        let data = Self::load_from_file(&path).unwrap_or_default();

        if !data.is_empty() {
            #[cfg(feature = "tracing-log")]
            tracing::info!(
                "[qc-02] 💾 Loaded {} keys from {}",
                data.len(),
                path.display()
            );
        } else {
            #[cfg(feature = "tracing-log")]
            tracing::info!("[qc-02] 📁 Storage file empty or not found");
        }

        Self { data, path }
    }

    fn load_from_file(path: &Path) -> Option<HashMap<Vec<u8>, Vec<u8>>> {
        use std::io::Read;

        let mut file = std::fs::File::open(path).ok()?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).ok()?;

        // Simple binary format: [key_len:u32][key][value_len:u32][value]...
        let mut data = HashMap::new();
        let mut cursor = 0;

        while cursor + 4 <= bytes.len() {
            // Read key length
            let key_len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().ok()?) as usize;
            cursor += 4;

            if cursor + key_len > bytes.len() {
                break;
            }
            let key = bytes[cursor..cursor + key_len].to_vec();
            cursor += key_len;

            if cursor + 4 > bytes.len() {
                break;
            }
            // Read value length
            let value_len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().ok()?) as usize;
            cursor += 4;

            if cursor + value_len > bytes.len() {
                break;
            }
            let value = bytes[cursor..cursor + value_len].to_vec();
            cursor += value_len;

            data.insert(key, value);
        }

        Some(data)
    }

    fn save_to_file(&self) -> Result<(), KVStoreError> {
        use std::io::Write;

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| KVStoreError::IOError {
                message: e.to_string(),
            })?;
        }

        let mut bytes = Vec::new();

        for (key, value) in &self.data {
            bytes.extend_from_slice(&(key.len() as u32).to_le_bytes());
            bytes.extend_from_slice(key);
            bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
            bytes.extend_from_slice(value);
        }

        // Write atomically via temp file
        let temp_path = self.path.with_extension("tmp");
        let mut file = std::fs::File::create(&temp_path).map_err(|e| KVStoreError::IOError {
            message: e.to_string(),
        })?;
        file.write_all(&bytes).map_err(|e| KVStoreError::IOError {
            message: e.to_string(),
        })?;
        file.sync_all().map_err(|e| KVStoreError::IOError {
            message: e.to_string(),
        })?;

        std::fs::rename(&temp_path, &self.path).map_err(|e| KVStoreError::IOError {
            message: e.to_string(),
        })?;

        Ok(())
    }
}

impl KeyValueStore for FileBackedKVStore {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KVStoreError> {
        Ok(self.data.get(key).cloned())
    }

    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), KVStoreError> {
        self.data.insert(key.to_vec(), value.to_vec());
        self.save_to_file()
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), KVStoreError> {
        self.data.remove(key);
        self.save_to_file()
    }

    fn atomic_batch_write(&mut self, operations: Vec<BatchOperation>) -> Result<(), KVStoreError> {
        for op in operations {
            match op {
                BatchOperation::Put { key, value } => {
                    self.data.insert(key, value);
                }
                BatchOperation::Delete { key } => {
                    self.data.remove(&key);
                }
            }
        }
        self.save_to_file()
    }

    fn exists(&self, key: &[u8]) -> Result<bool, KVStoreError> {
        Ok(self.data.contains_key(key))
    }

    fn prefix_scan(&self, prefix: &[u8]) -> Result<ScanResult, KVStoreError> {
        let results: Vec<_> = self
            .data
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Ok(results)
    }
}
