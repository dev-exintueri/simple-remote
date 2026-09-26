use auth::{Protector, StoreError};
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use zeroize::Zeroizing;

/// DPAPI 호출에 함께 넣는 고정 entropy. 다른 프로그램이 같은 사용자 계정으로 만든
/// DPAPI blob 을 풀지 못하게 막는다. 비밀은 아니므로 코드에 그대로 둔다.
const ENTROPY: &[u8] = b"simple-remote store v1";

/// Windows DPAPI(`CryptProtectData`/`CryptUnprotectData`) 로 로그인 계정 단위 보호를 거는
/// `auth::Protector` 구현체. `CRYPTPROTECT_LOCAL_MACHINE` 은 쓰지 않으므로 다른 계정으로
/// 로그인하면 이전에 저장한 값을 풀 수 없다.
#[derive(Debug, Clone, Copy, Default)]
pub struct DpapiProtector;

impl DpapiProtector {
    pub fn new() -> Self {
        Self
    }
}

fn entropy_blob() -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: ENTROPY.len() as u32,
        pbData: ENTROPY.as_ptr() as *mut u8,
    }
}

/// DPAPI 가 채운 출력 buffer 를 복사해 오고 `LocalFree` 로 해제한다.
unsafe fn take_blob(blob: &CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let out = if blob.cbData == 0 || blob.pbData.is_null() {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec() }
    };
    if !blob.pbData.is_null() {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(blob.pbData as *mut _)));
        }
    }
    out
}

impl Protector for DpapiProtector {
    fn protect(&self, plain: &[u8]) -> Result<Vec<u8>, StoreError> {
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: plain.len() as u32,
            pbData: plain.as_ptr() as *mut u8,
        };
        let entropy = entropy_blob();
        let mut data_out = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptProtectData(
                &data_in,
                None,
                Some(&entropy),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut data_out,
            )
            .map_err(|e| StoreError::Protect(e.to_string()))?;
            Ok(take_blob(&data_out))
        }
    }

    fn unprotect(&self, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, StoreError> {
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: blob.len() as u32,
            pbData: blob.as_ptr() as *mut u8,
        };
        let entropy = entropy_blob();
        let mut data_out = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptUnprotectData(
                &data_in,
                None,
                Some(&entropy),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut data_out,
            )
            .map_err(|e| StoreError::Protect(e.to_string()))?;
            Ok(Zeroizing::new(take_blob(&data_out)))
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use auth::{HostBook, StateFile};

    fn unique_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("platform-win-dpapi-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn roundtrip() {
        let protector = DpapiProtector::new();
        let plain = b"simple-remote dpapi roundtrip test".to_vec();

        let blob = protector.protect(&plain).unwrap();
        assert!(
            !blob.windows(plain.len()).any(|w| w == plain.as_slice()),
            "protected blob must not contain the plaintext"
        );

        let recovered = protector.unprotect(&blob).unwrap();
        assert_eq!(recovered.as_slice(), plain.as_slice());
    }

    #[test]
    fn tampered_blob_fails() {
        let protector = DpapiProtector::new();
        let blob = protector.protect(b"tamper me").unwrap();

        let mut tampered = blob.clone();
        let mid = tampered.len() / 2;
        tampered[mid] ^= 0xFF;

        let result = protector.unprotect(&tampered);
        assert!(matches!(result, Err(StoreError::Protect(_))));
    }

    #[test]
    fn empty_roundtrip() {
        let protector = DpapiProtector::new();
        let blob = protector.protect(&[]).unwrap();
        let recovered = protector.unprotect(&blob).unwrap();
        assert!(recovered.is_empty());
    }

    #[test]
    fn state_file_with_dpapi() {
        let dir = unique_dir();
        let path = dir.join("hostbook.bin");
        let sf = StateFile::new(path, DpapiProtector::new());

        let book = HostBook::default();
        sf.save(&book).unwrap();
        let loaded: HostBook = sf.load().unwrap().unwrap();
        assert_eq!(loaded, book);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
