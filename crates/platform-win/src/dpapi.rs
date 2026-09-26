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
    // ENTROPY 는 컴파일 시점에 고정된 짧은 상수라 u32 범위를 벗어날 일이 없다.
    CRYPT_INTEGER_BLOB {
        cbData: ENTROPY.len() as u32,
        pbData: ENTROPY.as_ptr() as *mut u8,
    }
}

/// `CRYPT_INTEGER_BLOB.cbData` 는 `u32` 라서 그보다 큰 입력은 `as u32` 로 조용히 잘려나간다.
/// 그런 축소가 저장 파일을 조용히 깨뜨리지 않도록, 넘치면 여기서 fail closed 한다.
fn checked_len(bytes: &[u8]) -> Result<u32, StoreError> {
    u32::try_from(bytes.len()).map_err(|_| StoreError::Protect("input too large".into()))
}

/// `take_blob` 이 넘겨받는 blob 이 평문(비밀)인지 암호문인지 표시한다. `Plaintext` 를
/// 넘기면 복사 후 `LocalFree` 전에 원본 buffer 를 0 으로 덮어써서, DPAPI 가 할당한 힙
/// 조각에 평문 사본이 해제된 뒤에도 남지 않게 한다. `Ciphertext` 는 비밀이 아니므로
/// 덮어쓰지 않는다. 호출부마다 명시적으로 골라야 하므로 새 호출을 추가하며 지우는 걸
/// 잊을 수 없다.
enum BlobKind {
    Plaintext,
    Ciphertext,
}

/// DPAPI 가 채운 출력 buffer 를 복사해 오고 `LocalFree` 로 해제한다. `kind` 가
/// `BlobKind::Plaintext` 면 복사가 끝난 원본 buffer 를 해제 전에 0 으로 덮어쓴다
/// (평문이 해제된 힙 메모리에 그대로 남는 것을 막는다).
unsafe fn take_blob(blob: &CRYPT_INTEGER_BLOB, kind: BlobKind) -> Vec<u8> {
    let out = if blob.cbData == 0 || blob.pbData.is_null() {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec() }
    };
    if !blob.pbData.is_null() {
        if matches!(kind, BlobKind::Plaintext) {
            unsafe {
                std::ptr::write_bytes(blob.pbData, 0, blob.cbData as usize);
            }
        }
        unsafe {
            let _ = LocalFree(Some(HLOCAL(blob.pbData as *mut _)));
        }
    }
    out
}

impl Protector for DpapiProtector {
    fn protect(&self, plain: &[u8]) -> Result<Vec<u8>, StoreError> {
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: checked_len(plain)?,
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
            // 결과는 암호문이므로 해제 전에 덮어쓸 필요가 없다.
            Ok(take_blob(&data_out, BlobKind::Ciphertext))
        }
    }

    fn unprotect(&self, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, StoreError> {
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: checked_len(blob)?,
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
            // 결과는 평문이므로 Zeroizing 으로 복사한 뒤 DPAPI 가 할당한 원본 buffer 를
            // LocalFree 전에 0 으로 덮어쓴다 (take_blob 이 강제한다).
            Ok(Zeroizing::new(take_blob(&data_out, BlobKind::Plaintext)))
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
