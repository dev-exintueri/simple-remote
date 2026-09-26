#[cfg(windows)]
mod dpapi;

#[cfg(windows)]
pub use dpapi::DpapiProtector;
