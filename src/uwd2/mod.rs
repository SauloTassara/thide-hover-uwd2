//! UWD2 integration.
//!
//! UWD2 is AGPL-3.0 licensed and is kept as a clearly separated module. It
//! resolves the current shell32 watermark function through Microsoft symbols
//! and patches the running explorer.exe process in memory. The caller must
//! invoke this from a worker thread because symbol download and PDB parsing
//! can take time.

use std::panic::{catch_unwind, AssertUnwindSafe};

mod cache_pdb;
mod constants;
mod explorer_modinfo;
mod fetch_pdb;
mod inject;
mod parse_pdb;

/// Apply the UWD2 watermark patch without allowing an upstream panic to take
/// down the tray application.
pub fn patch_watermark() -> Result<(), String> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let guid = unsafe { explorer_modinfo::get_guid() };
        let rva = cache_pdb::get_rva(guid);
        unsafe {
            inject::inject(rva);
            inject::refresh();
        }
    }));

    match result {
        Ok(()) => Ok(()),
        Err(payload) => {
            let message = if let Some(value) = payload.downcast_ref::<&str>() {
                (*value).to_string()
            } else if let Some(value) = payload.downcast_ref::<String>() {
                value.clone()
            } else {
                "UWD2 terminó con un error no especificado".to_string()
            };
            Err(message)
        }
    }
}
