use std::mem::size_of;
use std::path::Path;

use windows_054::core::imp::CloseHandle;
use windows_054::core::PCSTR;
use windows_054::Win32::Foundation::{GetLastError, FALSE, HANDLE, HMODULE};
use windows_054::Win32::System::Diagnostics::Debug::{
    SymGetModuleInfo64, SymInitialize, SymLoadModuleEx, SymSetOptions, IMAGEHLP_MODULE64,
    SYMOPT_UNDNAME, SYM_LOAD_FLAGS,
};
use windows_054::Win32::System::LibraryLoader::GetModuleHandleExA;
use windows_054::Win32::System::Threading::{OpenProcess, PROCESS_ALL_ACCESS};

use super::constants::SHELL32_PATH;

pub unsafe fn get_guid() -> String {
    let modinfo = get_shell32_modinfo();
    let sig = modinfo.PdbSig70.to_u128();
    let age = modinfo.PdbAge;
    format!("{sig:032X}{age:X}")
}

pub unsafe fn get_shell32_offset() -> u64 {
    let modinfo = get_shell32_modinfo();
    modinfo.BaseOfImage
}

pub unsafe fn get_explorer_handle() -> HANDLE {
    let explorerid = sysinfo::System::new_with_specifics(
        sysinfo::RefreshKind::new().with_processes(sysinfo::ProcessRefreshKind::everything()),
    )
    .processes()
    .values()
    .find(|proc| {
        if let Some(path) = proc.exe() {
            path == Path::new(r"C:\Windows\explorer.exe")
        } else {
            false
        }
    })
    .unwrap()
    .pid()
    .as_u32();

    OpenProcess(PROCESS_ALL_ACCESS, FALSE, explorerid).unwrap()
}

pub unsafe fn get_shell32_modinfo() -> IMAGEHLP_MODULE64 {
    let explorerhandle = get_explorer_handle();

    SymInitialize(explorerhandle, PCSTR::null(), true).expect("initializing symbols failed");
    SymSetOptions(SYMOPT_UNDNAME);
    let nullterminatedpath = format!("{SHELL32_PATH}\0");
    let name = PCSTR::from_raw(nullterminatedpath.as_ptr());
    let mut module = HMODULE::default();
    GetModuleHandleExA(0, name, &mut module as *mut HMODULE).unwrap();

    let r = SymLoadModuleEx(
        explorerhandle,
        HANDLE::default(),
        name,
        PCSTR::null(),
        module.0 as u64,
        0,
        None,
        SYM_LOAD_FLAGS::default(),
    );
    if r == 0 {
        GetLastError();
    }

    let mut modinfo = IMAGEHLP_MODULE64 {
        SizeOfStruct: size_of::<IMAGEHLP_MODULE64>() as u32,
        ..Default::default()
    };
    SymGetModuleInfo64(
        explorerhandle,
        module.0 as u64,
        &mut modinfo as *mut IMAGEHLP_MODULE64,
    )
    .unwrap();
    CloseHandle(explorerhandle.0);
    modinfo
}
