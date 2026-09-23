// Spike only (throwaway).
// dpapi_probe: DPAPI round-trip using CryptProtectData/CryptUnprotectData (no entropy, UI_FORBIDDEN).
// Usage: dpapi_probe protect <infile> <outfile>
//        dpapi_probe unprotect <infile>
// Prints current user name and session id first.

use std::fs;

use windows::core::{PWSTR, Result};
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::System::WindowsProgramming::GetUserNameW;

fn current_user() -> String {
    let mut buf = [0u16; 256];
    let mut len = buf.len() as u32;
    unsafe {
        match GetUserNameW(Some(PWSTR(buf.as_mut_ptr())), &mut len) {
            Ok(()) => {
                // len includes the terminating null.
                let n = (len as usize).saturating_sub(1);
                String::from_utf16_lossy(&buf[..n])
            }
            Err(e) => format!("<GetUserNameW failed: {e}>"),
        }
    }
}

fn current_session() -> u32 {
    let mut sid = 0u32;
    unsafe {
        let pid = GetCurrentProcessId();
        match ProcessIdToSessionId(pid, &mut sid) {
            Ok(()) => sid,
            Err(_) => u32::MAX,
        }
    }
}

/// Copy the DPAPI output blob into a Vec and free the DPAPI-allocated buffer.
unsafe fn take_blob(blob: &CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let out = if blob.cbData == 0 || blob.pbData.is_null() {
        Vec::new()
    } else {
        std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec()
    };
    if !blob.pbData.is_null() {
        let _ = LocalFree(Some(HLOCAL(blob.pbData as *mut _)));
    }
    out
}

fn protect(input: &[u8]) -> Result<Vec<u8>> {
    let data_in = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut data_out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(
            &data_in,
            None,                          // szdatadescr
            None,                          // poptionalentropy (no entropy)
            None,                          // pvreserved
            None,                          // ppromptstruct
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut data_out,
        )?;
        Ok(take_blob(&data_out))
    }
}

fn unprotect(input: &[u8]) -> Result<Vec<u8>> {
    let data_in = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut data_out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &data_in,
            None,                          // ppszdatadescr
            None,                          // poptionalentropy
            None,                          // pvreserved
            None,                          // ppromptstruct
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut data_out,
        )?;
        Ok(take_blob(&data_out))
    }
}

fn main() -> Result<()> {
    println!("user={} session={}", current_user(), current_session());

    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("protect") => {
            let infile = &args[2];
            let outfile = &args[3];
            let input = fs::read(infile).expect("read infile");
            let out = protect(&input)?;
            fs::write(outfile, &out).expect("write outfile");
            println!("protected {} bytes -> {} bytes", input.len(), out.len());
        }
        Some("unprotect") => {
            let infile = &args[2];
            let input = fs::read(infile).expect("read infile");
            let out = unprotect(&input)?;
            println!("unprotected {} bytes -> {} bytes", input.len(), out.len());
            // Print recovered plaintext if it is valid UTF-8.
            if let Ok(s) = std::str::from_utf8(&out) {
                println!("plaintext={s}");
            }
        }
        _ => {
            eprintln!("usage: dpapi_probe protect <infile> <outfile> | unprotect <infile>");
        }
    }
    Ok(())
}
