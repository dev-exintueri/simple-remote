use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;
use zeroize::Zeroizing;

const MAGIC: &[u8; 4] = b"SRS1";

/// `StateFile` 읽기/쓰기 실패.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("corrupt state file")]
    Corrupt,
    #[error("protect failed: {0}")]
    Protect(String),
}

/// 저장 전 평문을 감싸고, 불러온 뒤 되돌리는 자리. 실제 암호화는 구현체가 맡는다.
pub trait Protector {
    fn protect(&self, plain: &[u8]) -> Result<Vec<u8>, StoreError>;
    fn unprotect(&self, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, StoreError>;
}

/// `b"SRS1" || protect(postcard(value))` 형식으로 값 하나를 담는 파일.
pub struct StateFile<P: Protector> {
    path: PathBuf,
    protector: P,
}

impl<P: Protector> StateFile<P> {
    pub fn new(path: PathBuf, protector: P) -> Self {
        Self { path, protector }
    }

    /// 파일이 없으면 `Ok(None)`. 머리가 다르거나, 풀리지 않거나, 풀린 뒤 해독이
    /// 안 되면 `Err(Corrupt)`. 이 함수는 파일을 지우거나 다시 쓰지 않는다.
    pub fn load<T: DeserializeOwned>(&self) -> Result<Option<T>, StoreError> {
        let bytes = match fs::read(&self.path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if bytes.len() < MAGIC.len() || &bytes[..MAGIC.len()] != MAGIC {
            return Err(StoreError::Corrupt);
        }
        let plain = self.protector.unprotect(&bytes[MAGIC.len()..])?;
        let (value, remainder) = postcard::take_from_bytes(&plain).map_err(|_| StoreError::Corrupt)?;
        if !remainder.is_empty() {
            return Err(StoreError::Corrupt);
        }
        Ok(Some(value))
    }

    /// 같은 폴더의 `<파일>.tmp` 에 쓰고 `sync_all` 뒤 `rename` 으로 교체한다.
    /// 중간에 실패하면 tmp 파일을 지우려 시도하고 기존 파일은 건드리지 않는다.
    pub fn save<T: Serialize>(&self, value: &T) -> Result<(), StoreError> {
        let plain: Zeroizing<Vec<u8>> = Zeroizing::new(
            postcard::to_stdvec(value).map_err(|e| StoreError::Protect(e.to_string()))?,
        );
        let protected = self.protector.protect(&plain)?;

        let mut buf = Vec::with_capacity(MAGIC.len() + protected.len());
        buf.extend_from_slice(MAGIC);
        buf.extend_from_slice(&protected);

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp_path = tmp_path_for(&self.path);
        let result = (|| -> Result<(), StoreError> {
            let mut file = fs::File::create(&tmp_path)?;
            file.write_all(&buf)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&tmp_path, &self.path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp_path);
        }
        result
    }
}

fn tmp_path_for(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// 암호화 없이 그대로 통과시키는 개발용 protector. 실제 배포 경로에서는 쓰지 않는다.
#[cfg(any(test, feature = "insecure-dev-store"))]
pub struct DevPlaintextProtector;

#[cfg(any(test, feature = "insecure-dev-store"))]
impl std::fmt::Debug for DevPlaintextProtector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DevPlaintextProtector(INSECURE)")
    }
}

#[cfg(any(test, feature = "insecure-dev-store"))]
impl Protector for DevPlaintextProtector {
    fn protect(&self, plain: &[u8]) -> Result<Vec<u8>, StoreError> {
        Ok(plain.to_vec())
    }

    fn unprotect(&self, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, StoreError> {
        Ok(Zeroizing::new(blob.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::HostBook;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_dir() -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir()
            .join(format!("auth-store-test-{}-{}-{}", std::process::id(), n, nanos));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    struct FailingProtector;

    impl Protector for FailingProtector {
        fn protect(&self, _plain: &[u8]) -> Result<Vec<u8>, StoreError> {
            Err(StoreError::Protect("boom".to_string()))
        }

        fn unprotect(&self, _blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, StoreError> {
            Err(StoreError::Protect("boom".to_string()))
        }
    }

    #[test]
    fn missing_file_is_none() {
        let dir = unique_dir();
        let path = dir.join("host.bin");
        let sf = StateFile::new(path, DevPlaintextProtector);

        let result: Option<HostBook> = sf.load().unwrap();
        assert!(result.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = unique_dir();
        let path = dir.join("state.bin");
        let sf = StateFile::new(path, DevPlaintextProtector);

        let book = HostBook::default();
        sf.save(&book).unwrap();
        let loaded: HostBook = sf.load().unwrap().unwrap();
        assert_eq!(loaded, book);

        let text = "hello world".to_string();
        sf.save(&text).unwrap();
        let loaded_text: String = sf.load().unwrap().unwrap();
        assert_eq!(loaded_text, text);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_replaces_atomically() {
        let dir = unique_dir();
        let path = dir.join("state.bin");
        let sf = StateFile::new(path.clone(), DevPlaintextProtector);

        sf.save(&"first".to_string()).unwrap();
        sf.save(&"second".to_string()).unwrap();

        let loaded: String = sf.load().unwrap().unwrap();
        assert_eq!(loaded, "second");
        assert!(!tmp_path_for(&path).exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_file_is_error_and_left_untouched() {
        let dir = unique_dir();
        let path = dir.join("state.bin");
        let sf = StateFile::new(path.clone(), DevPlaintextProtector);
        sf.save(&"value".to_string()).unwrap();
        let original = fs::read(&path).unwrap();

        let mut bad_header = original.clone();
        bad_header[0] = b'X';
        fs::write(&path, &bad_header).unwrap();
        let result: Result<Option<String>, StoreError> = sf.load();
        assert!(matches!(result, Err(StoreError::Corrupt)));
        assert_eq!(fs::read(&path).unwrap(), bad_header);

        let mut truncated = original.clone();
        truncated.pop();
        fs::write(&path, &truncated).unwrap();
        let result: Result<Option<String>, StoreError> = sf.load();
        assert!(matches!(result, Err(StoreError::Corrupt)));
        assert_eq!(fs::read(&path).unwrap(), truncated);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn trailing_bytes_are_corrupt() {
        let dir = unique_dir();
        let path = dir.join("state.bin");
        let sf = StateFile::new(path.clone(), DevPlaintextProtector);
        sf.save(&"value".to_string()).unwrap();
        let original = fs::read(&path).unwrap();

        let mut with_garbage = original.clone();
        with_garbage.push(0xAB);
        fs::write(&path, &with_garbage).unwrap();

        let result: Result<Option<String>, StoreError> = sf.load();
        assert!(matches!(result, Err(StoreError::Corrupt)));
        assert_eq!(fs::read(&path).unwrap(), with_garbage);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_creates_missing_directory() {
        let dir = unique_dir();
        let path = dir.join("nested").join("deeper").join("state.bin");
        let sf = StateFile::new(path.clone(), DevPlaintextProtector);

        sf.save(&"value".to_string()).unwrap();
        let loaded: String = sf.load().unwrap().unwrap();
        assert_eq!(loaded, "value");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_protect_keeps_existing_file() {
        let dir = unique_dir();
        let path = dir.join("state.bin");

        let working = StateFile::new(path.clone(), DevPlaintextProtector);
        working.save(&"value".to_string()).unwrap();
        let original = fs::read(&path).unwrap();

        let failing = StateFile::new(path.clone(), FailingProtector);
        let result = failing.save(&"other".to_string());
        assert!(matches!(result, Err(StoreError::Protect(_))));
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(!tmp_path_for(&path).exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn protector_failure_is_error() {
        let dir = unique_dir();
        let path = dir.join("state.bin");
        let sf = StateFile::new(path.clone(), FailingProtector);

        let result = sf.save(&"value".to_string());
        assert!(matches!(result, Err(StoreError::Protect(_))));
        assert!(!path.exists());
        assert!(!tmp_path_for(&path).exists());

        let _ = fs::remove_dir_all(&dir);
    }
}
