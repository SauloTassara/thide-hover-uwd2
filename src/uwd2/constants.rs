use std::path::PathBuf;

use directories::ProjectDirs;

pub fn data_dir() -> PathBuf {
    ProjectDirs::from("net", "reticivis", "UWD2")
        .map(|dirs| dirs.data_dir().to_owned())
        .unwrap_or_else(|| std::env::temp_dir().join("thide-hover-uwd2"))
}

pub const SHELL32_PATH: &str = r"C:\Windows\System32\shell32.dll";

#[cfg(target_arch = "x86_64")]
pub const RET: [u8; 1] = [0xC3];

#[cfg(target_arch = "aarch64")]
pub const RET: [u8; 4] = [0xc0, 0x03, 0x1f, 0xd6];
