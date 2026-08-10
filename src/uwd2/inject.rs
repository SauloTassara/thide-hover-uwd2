use std::ffi::c_void;

use windows_054::core::imp::CloseHandle;
use windows_054::core::s;
use windows_054::Win32::Foundation::{LPARAM, WPARAM};
use windows_054::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows_054::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
use windows_054::Win32::UI::WindowsAndMessaging::{
    FindWindowA, GetWindow, GetWindowInfo, SendMessageA, GW_CHILD, WINDOWINFO, WM_COMMAND,
    WS_VISIBLE,
};

use super::constants::RET;
use super::explorer_modinfo::{get_explorer_handle, get_shell32_offset};

pub unsafe fn inject(rva: u32) {
    let offset = get_shell32_offset();
    let explorerhandle = get_explorer_handle();

    WriteProcessMemory(
        explorerhandle,
        (offset + rva as u64) as *const c_void,
        &RET as *const u8 as *const c_void,
        RET.len(),
        None,
    )
    .unwrap();
    CloseHandle(explorerhandle.0);
}

pub unsafe fn refresh() {
    let h_wnd = GetWindow(FindWindowA(s!("Progman"), s!("Program Manager")), GW_CHILD);

    let h_wnd2 = GetWindow(h_wnd, GW_CHILD);
    let mut window_info = WINDOWINFO {
        cbSize: std::mem::size_of::<WINDOWINFO>() as u32,
        ..Default::default()
    };
    GetWindowInfo(h_wnd2, &mut window_info as *mut _).unwrap();
    let visible = window_info.dwStyle & WS_VISIBLE == WS_VISIBLE;

    if visible {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    } else {
        SendMessageA(h_wnd, WM_COMMAND, WPARAM(0x7402), LPARAM::default());
        SendMessageA(h_wnd, WM_COMMAND, WPARAM(0x7402), LPARAM::default());
    }
}
