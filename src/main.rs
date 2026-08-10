#![windows_subsystem = "windows"]

mod cli;
mod uwd2;

use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::mem;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};
use windows::Win32::Foundation::{
    GetLastError, BOOL, ERROR_ALREADY_EXISTS, HANDLE, HINSTANCE, HWND, LPARAM, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Threading::{
    CreateMutexW, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
};
use windows::Win32::UI::Shell::{
    SHAppBarMessage, ABM_GETSTATE, ABM_SETSTATE, ABS_AUTOHIDE, APPBARDATA,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, EnumChildWindows, EnumWindows,
    FindWindowExW, GetCursorPos, GetForegroundWindow, GetMessageW, GetWindowRect,
    GetWindowThreadProcessId, IsWindowVisible, MessageBoxW, PostQuitMessage, RegisterClassW,
    ShowWindow, TranslateMessage, EVENT_OBJECT_CLOAKED, EVENT_OBJECT_HIDE, EVENT_OBJECT_SHOW,
    EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_FOREGROUND, HWND_MESSAGE, MB_ICONERROR, MB_OK, MSG,
    SW_HIDE, SW_SHOW, WINEVENT_OUTOFCONTEXT, WM_HOTKEY, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};
use winit::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};

const HOVER_POLL_INTERVAL_MS: u64 = 50;
const HOVER_DELAY_MS: u64 = 300;
const START_MENU_POLL_INTERVAL_MS: u64 = 50;
const START_MENU_HIDE_DELAY_MS: u64 = 180;
const HOVER_EDGE_WIDTH: i32 = 4;
const HOVER_EDGE_TOLERANCE: i32 = 32;
const HOTKEY_ID: i32 = 0x5448;
const HOTKEY_VK_T: u32 = 0x54;

#[derive(Debug, Clone)]
enum IPCMessage {
    Show,
    Hide,
    Toggle,
    Quit,
    HoverShow,
    HoverHide,
    StartMenuSignal,
    StartMenuChanged(bool),
    PatchWatermark {
        manual: bool,
    },
    WatermarkResult {
        manual: bool,
        result: Result<(), String>,
    },
}

static GLOBAL_EVENT_PROXY: Mutex<Option<EventLoopProxy<IPCMessage>>> = Mutex::new(None);
static START_EVENT_SIGNAL_PENDING: AtomicBool = AtomicBool::new(false);

struct VisibilityState {
    should_hide: Arc<AtomicBool>,
    manual_override: Arc<AtomicBool>,
    hover_visible: Arc<AtomicBool>,
    start_menu_visible: Arc<AtomicBool>,
}

impl VisibilityState {
    fn wants_visible(&self) -> bool {
        self.manual_override.load(Ordering::SeqCst)
            || self.hover_visible.load(Ordering::SeqCst)
            || self.start_menu_visible.load(Ordering::SeqCst)
    }
}

fn runtime_log(message: &str) {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join("thide-hover-uwd2");
    if create_dir_all(&dir).is_err() {
        return;
    }

    let path = dir.join("runtime.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{}", message);
    }
}

fn attach_console() -> bool {
    unsafe {
        use windows::Win32::System::Console::{
            AttachConsole, GetConsoleMode, GetStdHandle, ATTACH_PARENT_PROCESS, CONSOLE_MODE,
            STD_OUTPUT_HANDLE,
        };

        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            return false;
        }

        let stdout = GetStdHandle(STD_OUTPUT_HANDLE);
        if let Ok(handle) = stdout {
            if !handle.is_invalid() {
                let mut mode = CONSOLE_MODE(0);
                GetConsoleMode(handle, &mut mode).is_ok()
            } else {
                false
            }
        } else {
            false
        }
    }
}

fn get_process_name(hwnd: HWND) -> Option<String> {
    unsafe {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let h_process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer: [u16; 512] = [0; 512];
        let mut len: u32 = buffer.len() as u32;
        QueryFullProcessImageNameW(
            h_process,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut len,
        )
        .ok()?;
        let _ = windows::Win32::Foundation::CloseHandle(h_process);

        let full_path = String::from_utf16_lossy(&buffer[..len as usize]);
        std::path::Path::new(&full_path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }
}

fn is_start_menu_process(process_name: &str) -> bool {
    process_name.eq_ignore_ascii_case("StartMenuExperienceHost.exe")
        || process_name.eq_ignore_ascii_case("ShellExperienceHost.exe")
        || process_name.eq_ignore_ascii_case("SearchHost.exe")
}

unsafe extern "system" fn enum_start_menu_windows(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect).is_err() || rect.right <= rect.left || rect.bottom <= rect.top
    {
        return BOOL(1);
    }

    let process_name = get_process_name(hwnd);
    if process_name.as_deref().is_some_and(is_start_menu_process) {
        let found = &mut *(lparam.0 as *mut bool);
        *found = true;
        return BOOL(0);
    }

    // Recent Windows builds can host the Start surface in a XAML child
    // window owned by ShellExperienceHost rather than exposing it as a
    // separate top-level HWND. Inspect that host's descendants as a fallback.
    if process_name
        .as_deref()
        .is_some_and(|name| name.eq_ignore_ascii_case("ShellExperienceHost.exe"))
    {
        let _ = EnumChildWindows(hwnd, Some(enum_start_menu_descendant), lparam);
        let found = &mut *(lparam.0 as *mut bool);
        if *found {
            return BOOL(0);
        }
    }

    BOOL(1)
}

unsafe extern "system" fn enum_start_menu_descendant(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect).is_err() || rect.right <= rect.left || rect.bottom <= rect.top
    {
        return BOOL(1);
    }

    if get_process_name(hwnd).is_some_and(|name| is_start_menu_process(&name)) {
        let found = &mut *(lparam.0 as *mut bool);
        *found = true;
        return BOOL(0);
    }

    BOOL(1)
}

fn is_start_menu_visible() -> bool {
    // On newer Windows 11 builds, Start can be rendered by a XAML surface
    // without a visible top-level rectangle. In that case EnumWindows below
    // does not expose the Start surface, but the foreground HWND still belongs
    // to the Start menu host.
    unsafe {
        let foreground = GetForegroundWindow();
        if !foreground.0.is_null()
            && get_process_name(foreground)
                .as_deref()
                .is_some_and(is_start_menu_process)
        {
            return true;
        }
    }

    let mut found = false;
    unsafe {
        let _ = EnumWindows(
            Some(enum_start_menu_windows),
            LPARAM((&mut found as *mut bool) as isize),
        );
    }
    found
}

unsafe extern "system" fn start_menu_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    if !matches!(
        event,
        EVENT_SYSTEM_FOREGROUND
            | EVENT_OBJECT_SHOW
            | EVENT_OBJECT_HIDE
            | EVENT_OBJECT_CLOAKED
            | EVENT_OBJECT_UNCLOAKED
    ) {
        return;
    }

    if START_EVENT_SIGNAL_PENDING.swap(true, Ordering::SeqCst) {
        return;
    }

    let sent = GLOBAL_EVENT_PROXY.lock().ok().and_then(|guard| {
        guard
            .as_ref()
            .map(|proxy| proxy.send_event(IPCMessage::StartMenuSignal))
    });

    if sent.is_none_or(|result| result.is_err()) {
        START_EVENT_SIGNAL_PENDING.store(false, Ordering::SeqCst);
    }
}

fn register_start_menu_hooks() -> Vec<HWINEVENTHOOK> {
    let ranges = [
        (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
        (EVENT_OBJECT_SHOW, EVENT_OBJECT_HIDE),
        (EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
    ];

    let mut hooks = Vec::new();
    for (event_min, event_max) in ranges {
        let hook = unsafe {
            SetWinEventHook(
                event_min,
                event_max,
                HINSTANCE::default(),
                Some(start_menu_event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if hook.is_invalid() {
            runtime_log(&format!(
                "Could not register Start menu WinEvent hook {event_min:#X}-{event_max:#X}"
            ));
        } else {
            hooks.push(hook);
        }
    }

    hooks
}

fn start_menu_poll_monitor(proxy: EventLoopProxy<IPCMessage>) {
    thread::spawn(move || {
        let mut published = false;
        let mut pending: Option<(bool, Instant)> = None;

        loop {
            let observed = is_start_menu_visible();

            if observed == published {
                pending = None;
            } else {
                let target = pending.get_or_insert_with(|| (observed, Instant::now()));
                if target.0 != observed {
                    *target = (observed, Instant::now());
                }

                let delay = if observed {
                    Duration::from_millis(0)
                } else {
                    Duration::from_millis(START_MENU_HIDE_DELAY_MS)
                };

                if target.1.elapsed() >= delay {
                    published = observed;
                    pending = None;
                    runtime_log(&format!("Start menu state changed: visible={published}"));
                    let _ = proxy.send_event(IPCMessage::StartMenuChanged(published));
                }
            }

            thread::sleep(Duration::from_millis(START_MENU_POLL_INTERVAL_MS));
        }
    });
}

fn load_icon() -> tray_icon::Icon {
    const ICON_DATA: &[u8] = include_bytes!("../assets/icon.ico");
    load_icon_file(ICON_DATA).expect("Failed to load icon")
}

fn load_icon_file(data: &[u8]) -> Result<tray_icon::Icon, Box<dyn std::error::Error>> {
    let icon_dir = ico::IconDir::read(std::io::Cursor::new(data))?;
    let entry = icon_dir
        .entries()
        .iter()
        .max_by_key(|entry| entry.width() * entry.height())
        .ok_or("No icon entries found")?;
    let image = entry.decode()?;
    Ok(tray_icon::Icon::from_rgba(
        image.rgba_data().to_vec(),
        image.width(),
        image.height(),
    )?)
}

fn find_all_explorer_taskbars() -> Vec<HWND> {
    unsafe {
        let mut taskbars = Vec::new();
        let class_names = ["Shell_TrayWnd\0", "Shell_SecondaryTrayWnd\0"];

        for class_name in &class_names {
            let class_wide: Vec<u16> = class_name.encode_utf16().collect();
            let mut hwnd = HWND(std::ptr::null_mut());

            while let Ok(found_hwnd) = FindWindowExW(
                HWND(std::ptr::null_mut()),
                hwnd,
                windows::core::PCWSTR(class_wide.as_ptr()),
                windows::core::PCWSTR::null(),
            ) {
                hwnd = found_hwnd;
                if hwnd.0.is_null() {
                    break;
                }

                if let Some(process_name) = get_process_name(hwnd) {
                    if process_name.eq_ignore_ascii_case("explorer.exe") {
                        taskbars.push(hwnd);
                        if *class_name == "Shell_TrayWnd\0" {
                            break;
                        }
                    }
                }
            }
        }

        taskbars
    }
}

fn current_explorer_pid() -> Option<u32> {
    let hwnd = find_all_explorer_taskbars().into_iter().next()?;
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        (pid != 0).then_some(pid)
    }
}

fn set_taskbar_state(show: bool) -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let show_cmd = if show { SW_SHOW } else { SW_HIDE };
        for hwnd in find_all_explorer_taskbars() {
            let _ = ShowWindow(hwnd, show_cmd);
        }
        Ok(())
    }
}

fn read_taskbar_appbar_state() -> u32 {
    unsafe {
        let mut appbar_data: APPBARDATA = mem::zeroed();
        appbar_data.cbSize = mem::size_of::<APPBARDATA>() as u32;
        SHAppBarMessage(ABM_GETSTATE, &mut appbar_data) as u32
    }
}

fn write_taskbar_appbar_state(state: u32) {
    unsafe {
        let mut appbar_data: APPBARDATA = mem::zeroed();
        appbar_data.cbSize = mem::size_of::<APPBARDATA>() as u32;
        appbar_data.lParam = LPARAM(state as isize);
        let _ = SHAppBarMessage(ABM_SETSTATE, &mut appbar_data);
    }
}

struct TaskbarStateManager {
    original_state: u32,
    enforced_state: u32,
}

impl TaskbarStateManager {
    fn new() -> Self {
        let original_state = read_taskbar_appbar_state();
        let enforced_state = original_state | ABS_AUTOHIDE;
        if enforced_state != original_state {
            write_taskbar_appbar_state(enforced_state);
        }
        Self {
            original_state,
            enforced_state,
        }
    }

    fn enforce(&self) {
        write_taskbar_appbar_state(self.enforced_state);
    }

    fn restore(&self) {
        write_taskbar_appbar_state(self.original_state);
    }
}

impl Drop for TaskbarStateManager {
    fn drop(&mut self) {
        self.restore();
    }
}

fn taskbar_show(should_hide: &AtomicBool, manager: &TaskbarStateManager) {
    should_hide.store(false, Ordering::SeqCst);
    manager.restore();
    let _ = set_taskbar_state(true);
}

fn taskbar_hide(should_hide: &AtomicBool, manager: &TaskbarStateManager) {
    should_hide.store(true, Ordering::SeqCst);
    manager.enforce();
    let _ = set_taskbar_state(false);
}

fn reconcile_taskbar(state: &VisibilityState, manager: &TaskbarStateManager) {
    let wants_visible = state.wants_visible();
    let is_hidden = state.should_hide.load(Ordering::SeqCst);

    if wants_visible && is_hidden {
        taskbar_show(&state.should_hide, manager);
    } else if !wants_visible && !is_hidden {
        taskbar_hide(&state.should_hide, manager);
    }
}

fn taskbar_toggle(state: &VisibilityState, manager: &TaskbarStateManager) {
    if state.should_hide.load(Ordering::SeqCst) {
        state.manual_override.store(true, Ordering::SeqCst);
        state.hover_visible.store(false, Ordering::SeqCst);
    } else {
        state.manual_override.store(false, Ordering::SeqCst);
        state.hover_visible.store(false, Ordering::SeqCst);
    }

    reconcile_taskbar(state, manager);
}

fn cursor_is_in_visible_taskbar(point: POINT) -> bool {
    unsafe {
        find_all_explorer_taskbars().into_iter().any(|hwnd| {
            if !IsWindowVisible(hwnd).as_bool() {
                return false;
            }
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return false;
            }
            point.x >= rect.left
                && point.x < rect.right
                && point.y >= rect.top
                && point.y < rect.bottom
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskbarEdge {
    Left,
    Top,
    Right,
    Bottom,
}

fn taskbar_edge_for_rect(taskbar_rect: RECT, monitor_rect: RECT) -> Option<TaskbarEdge> {
    let width = taskbar_rect.right - taskbar_rect.left;
    let height = taskbar_rect.bottom - taskbar_rect.top;
    if width <= 0 || height <= 0 {
        return None;
    }

    // Windows taskbars are wider than tall when docked at the top/bottom and
    // taller than wide when docked at the left/right. This also avoids the
    // top-left/top-right corner tie when a taskbar touches two edges.
    if width >= height {
        if (taskbar_rect.top - monitor_rect.top).abs() <= HOVER_EDGE_TOLERANCE {
            Some(TaskbarEdge::Top)
        } else if (monitor_rect.bottom - taskbar_rect.bottom).abs() <= HOVER_EDGE_TOLERANCE {
            Some(TaskbarEdge::Bottom)
        } else {
            None
        }
    } else if (taskbar_rect.left - monitor_rect.left).abs() <= HOVER_EDGE_TOLERANCE {
        Some(TaskbarEdge::Left)
    } else if (monitor_rect.right - taskbar_rect.right).abs() <= HOVER_EDGE_TOLERANCE {
        Some(TaskbarEdge::Right)
    } else {
        None
    }
}

fn cursor_is_on_taskbar_edge(point: POINT, monitor_rect: RECT, edge: TaskbarEdge) -> bool {
    if point.x < monitor_rect.left
        || point.x >= monitor_rect.right
        || point.y < monitor_rect.top
        || point.y >= monitor_rect.bottom
    {
        return false;
    }

    match edge {
        TaskbarEdge::Left => point.x < monitor_rect.left + HOVER_EDGE_WIDTH,
        TaskbarEdge::Top => point.y < monitor_rect.top + HOVER_EDGE_WIDTH,
        TaskbarEdge::Right => point.x >= monitor_rect.right - HOVER_EDGE_WIDTH,
        TaskbarEdge::Bottom => point.y >= monitor_rect.bottom - HOVER_EDGE_WIDTH,
    }
}

fn cursor_is_at_taskbar_edge(point: POINT) -> bool {
    unsafe {
        find_all_explorer_taskbars().into_iter().any(|hwnd| {
            let mut taskbar_rect = RECT::default();
            if GetWindowRect(hwnd, &mut taskbar_rect).is_err() {
                return false;
            }

            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            if monitor.0.is_null() {
                return false;
            }

            let mut info = MONITORINFO {
                cbSize: mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(monitor, &mut info).as_bool() {
                return false;
            }

            taskbar_edge_for_rect(taskbar_rect, info.rcMonitor)
                .is_some_and(|edge| cursor_is_on_taskbar_edge(point, info.rcMonitor, edge))
        })
    }
}

fn cursor_is_in_reveal_zone(point: POINT) -> bool {
    cursor_is_at_taskbar_edge(point) || cursor_is_in_visible_taskbar(point)
}

fn start_hover_monitor(
    proxy: EventLoopProxy<IPCMessage>,
    auto_hover: Arc<AtomicBool>,
    should_hide: Arc<AtomicBool>,
    manual_override: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let mut reveal_started: Option<Instant> = None;
        let mut conceal_started: Option<Instant> = None;

        loop {
            if !auto_hover.load(Ordering::SeqCst) || manual_override.load(Ordering::SeqCst) {
                reveal_started = None;
                conceal_started = None;
                thread::sleep(Duration::from_millis(HOVER_POLL_INTERVAL_MS));
                continue;
            }

            let mut point = POINT::default();
            let cursor_available = unsafe { GetCursorPos(&mut point).is_ok() };
            let in_reveal_zone = cursor_available && cursor_is_in_reveal_zone(point);
            let hidden = should_hide.load(Ordering::SeqCst);

            if hidden {
                conceal_started = None;
                if in_reveal_zone {
                    let started = reveal_started.get_or_insert_with(Instant::now);
                    if started.elapsed() >= Duration::from_millis(HOVER_DELAY_MS) {
                        let _ = proxy.send_event(IPCMessage::HoverShow);
                        reveal_started = None;
                    }
                } else {
                    reveal_started = None;
                }
            } else {
                reveal_started = None;
                if in_reveal_zone {
                    conceal_started = None;
                } else {
                    let started = conceal_started.get_or_insert_with(Instant::now);
                    if started.elapsed() >= Duration::from_millis(HOVER_DELAY_MS) {
                        let _ = proxy.send_event(IPCMessage::HoverHide);
                        conceal_started = None;
                    }
                }
            }

            thread::sleep(Duration::from_millis(HOVER_POLL_INTERVAL_MS));
        }
    });
}

fn start_explorer_monitor(proxy: EventLoopProxy<IPCMessage>) {
    thread::spawn(move || {
        let mut last_pid = current_explorer_pid();
        loop {
            thread::sleep(Duration::from_secs(1));
            let current_pid = current_explorer_pid();
            if current_pid.is_some() && current_pid != last_pid {
                let _ = proxy.send_event(IPCMessage::PatchWatermark { manual: false });
            }
            last_pid = current_pid;
        }
    });
}

fn spawn_watermark_patch(
    proxy: EventLoopProxy<IPCMessage>,
    manual: bool,
    in_progress: Arc<AtomicBool>,
) {
    if in_progress.swap(true, Ordering::SeqCst) {
        return;
    }

    thread::spawn(move || {
        let result = uwd2::patch_watermark();
        let log_message = match &result {
            Ok(()) => "UWD2 watermark patch completed".to_string(),
            Err(error) => format!("UWD2 watermark patch failed: {error}"),
        };
        runtime_log(&log_message);
        let _ = proxy.send_event(IPCMessage::WatermarkResult { manual, result });
        in_progress.store(false, Ordering::SeqCst);
    });
}

fn show_watermark_error(error: &str) {
    let title: Vec<u16> = "THide + UWD2\0".encode_utf16().collect();
    let message: Vec<u16> = format!("No se pudo volver a aplicar el parche Insider.\n\n{error}\0")
        .encode_utf16()
        .collect();
    unsafe {
        let _ = MessageBoxW(
            HWND(std::ptr::null_mut()),
            windows::core::PCWSTR(message.as_ptr()),
            windows::core::PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn check_single_instance() -> Option<HANDLE> {
    unsafe {
        let mutex_name: Vec<u16> = "Local\\TaskbarHideHoverUwd2_SingleInstance\0"
            .encode_utf16()
            .collect();
        let mutex_handle =
            CreateMutexW(None, true, windows::core::PCWSTR(mutex_name.as_ptr())).ok()?;

        if GetLastError() == ERROR_ALREADY_EXISTS {
            let title: Vec<u16> = "THide Hover + UWD2\0".encode_utf16().collect();
            let message: Vec<u16> = "La aplicación ya está en ejecución.\0"
                .encode_utf16()
                .collect();
            let _ = MessageBoxW(
                HWND(std::ptr::null_mut()),
                windows::core::PCWSTR(message.as_ptr()),
                windows::core::PCWSTR(title.as_ptr()),
                MB_OK,
            );
            return None;
        }

        Some(mutex_handle)
    }
}

fn create_ipc_window(event_loop_proxy: EventLoopProxy<IPCMessage>) {
    thread::spawn(move || unsafe {
        let class_name: Vec<u16> = format!("{}\0", cli::get_ipc_window_class())
            .encode_utf16()
            .collect();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(ipc_window_proc),
            lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);

        if let Ok(mut proxy) = GLOBAL_EVENT_PROXY.lock() {
            proxy.replace(event_loop_proxy);
        }

        let hwnd = match CreateWindowExW(
            Default::default(),
            windows::core::PCWSTR(class_name.as_ptr()),
            windows::core::PCWSTR::null(),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            None,
            None,
        ) {
            Ok(hwnd) => hwnd,
            Err(_) => {
                runtime_log("Could not create THide Hover + UWD2 IPC window");
                return;
            }
        };

        if RegisterHotKey(
            hwnd,
            HOTKEY_ID,
            MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
            HOTKEY_VK_T,
        )
        .is_err()
        {
            runtime_log("Could not register Ctrl+Alt+T global hotkey");
        }

        let start_menu_hooks = register_start_menu_hooks();

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = UnregisterHotKey(hwnd, HOTKEY_ID);
        for hook in start_menu_hooks {
            let _ = UnhookWinEvent(hook);
        }
    });
}

unsafe extern "system" fn ipc_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    let (msg_show, msg_hide, msg_toggle, msg_quit) = cli::get_message_ids();
    let msg_patch = cli::get_patch_message_id();

    let ipc_message = if msg == msg_show {
        Some(IPCMessage::Show)
    } else if msg == msg_hide {
        Some(IPCMessage::Hide)
    } else if msg == msg_toggle {
        Some(IPCMessage::Toggle)
    } else if msg == msg_patch {
        Some(IPCMessage::PatchWatermark { manual: true })
    } else if msg == msg_quit {
        PostQuitMessage(0);
        Some(IPCMessage::Quit)
    } else if msg == WM_HOTKEY && wparam.0 == HOTKEY_ID as usize {
        Some(IPCMessage::Toggle)
    } else {
        None
    };

    if let Some(ipc_msg) = ipc_message {
        if let Ok(guard) = GLOBAL_EVENT_PROXY.lock() {
            if let Some(proxy) = guard.as_ref() {
                let _ = proxy.send_event(ipc_msg);
            }
        }
        return windows::Win32::Foundation::LRESULT(0);
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        let _ = attach_console();
        return cli::handle_cli_command(&args);
    }

    let _mutex = check_single_instance().ok_or("Another instance is already running")?;
    let event_loop = EventLoopBuilder::<IPCMessage>::with_user_event().build()?;
    let event_loop_proxy = event_loop.create_proxy();

    let tray_menu = Menu::new();
    let show_item = MenuItem::new("Show Taskbar", true, None);
    let hide_item = MenuItem::new("Hide Taskbar", true, None);
    let toggle_item = MenuItem::new("Toggle Taskbar", true, None);
    let hover_item = CheckMenuItem::new("Auto-hide on hover", true, true, None);
    let patch_item = MenuItem::new("Re-patch Insider watermark", true, None);
    let quit_item = MenuItem::new("Quit", true, None);
    tray_menu.append(&show_item)?;
    tray_menu.append(&hide_item)?;
    tray_menu.append(&toggle_item)?;
    tray_menu.append(&hover_item)?;
    tray_menu.append(&patch_item)?;
    tray_menu.append(&quit_item)?;

    let tray_icon: TrayIcon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("THide Hover + UWD2 | Ctrl+Alt+T")
        .with_icon(load_icon())
        .build()?;

    let taskbar_manager = Arc::new(TaskbarStateManager::new());
    let visibility_state = Arc::new(VisibilityState {
        should_hide: Arc::new(AtomicBool::new(true)),
        manual_override: Arc::new(AtomicBool::new(false)),
        hover_visible: Arc::new(AtomicBool::new(false)),
        start_menu_visible: Arc::new(AtomicBool::new(is_start_menu_visible())),
    });
    let auto_hover = Arc::new(AtomicBool::new(true));
    let watermark_in_progress = Arc::new(AtomicBool::new(false));

    taskbar_manager.enforce();
    set_taskbar_state(false)?;

    create_ipc_window(event_loop_proxy.clone());
    start_menu_poll_monitor(event_loop_proxy.clone());
    start_hover_monitor(
        event_loop_proxy.clone(),
        Arc::clone(&auto_hover),
        Arc::clone(&visibility_state.should_hide),
        Arc::clone(&visibility_state.manual_override),
    );
    start_explorer_monitor(event_loop_proxy.clone());
    spawn_watermark_patch(
        event_loop_proxy.clone(),
        false,
        Arc::clone(&watermark_in_progress),
    );

    let menu_channel = MenuEvent::receiver();
    let taskbar_manager_for_loop = Arc::clone(&taskbar_manager);
    let visibility_state_for_loop = Arc::clone(&visibility_state);
    let auto_hover_for_loop = Arc::clone(&auto_hover);
    let watermark_in_progress_for_loop = Arc::clone(&watermark_in_progress);
    let event_proxy_for_loop = event_loop_proxy.clone();

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);

        if let winit::event::Event::UserEvent(ipc_msg) = event {
            match ipc_msg {
                IPCMessage::Show => {
                    visibility_state_for_loop
                        .manual_override
                        .store(true, Ordering::SeqCst);
                    visibility_state_for_loop
                        .hover_visible
                        .store(false, Ordering::SeqCst);
                    reconcile_taskbar(&visibility_state_for_loop, &taskbar_manager_for_loop);
                }
                IPCMessage::Hide => {
                    visibility_state_for_loop
                        .manual_override
                        .store(false, Ordering::SeqCst);
                    visibility_state_for_loop
                        .hover_visible
                        .store(false, Ordering::SeqCst);
                    reconcile_taskbar(&visibility_state_for_loop, &taskbar_manager_for_loop);
                }
                IPCMessage::Toggle => {
                    taskbar_toggle(&visibility_state_for_loop, &taskbar_manager_for_loop)
                }
                IPCMessage::HoverShow => {
                    if auto_hover_for_loop.load(Ordering::SeqCst)
                        && !visibility_state_for_loop
                            .manual_override
                            .load(Ordering::SeqCst)
                    {
                        visibility_state_for_loop
                            .hover_visible
                            .store(true, Ordering::SeqCst);
                        reconcile_taskbar(&visibility_state_for_loop, &taskbar_manager_for_loop);
                    }
                }
                IPCMessage::HoverHide => {
                    if auto_hover_for_loop.load(Ordering::SeqCst)
                        && !visibility_state_for_loop
                            .manual_override
                            .load(Ordering::SeqCst)
                    {
                        visibility_state_for_loop
                            .hover_visible
                            .store(false, Ordering::SeqCst);
                        reconcile_taskbar(&visibility_state_for_loop, &taskbar_manager_for_loop);
                    }
                }
                IPCMessage::StartMenuSignal => {
                    START_EVENT_SIGNAL_PENDING.store(false, Ordering::SeqCst);
                    if is_start_menu_visible() {
                        visibility_state_for_loop
                            .start_menu_visible
                            .store(true, Ordering::SeqCst);
                        reconcile_taskbar(&visibility_state_for_loop, &taskbar_manager_for_loop);
                    }
                }
                IPCMessage::StartMenuChanged(visible) => {
                    visibility_state_for_loop
                        .start_menu_visible
                        .store(visible, Ordering::SeqCst);
                    reconcile_taskbar(&visibility_state_for_loop, &taskbar_manager_for_loop);
                }
                IPCMessage::PatchWatermark { manual } => spawn_watermark_patch(
                    event_proxy_for_loop.clone(),
                    manual,
                    Arc::clone(&watermark_in_progress_for_loop),
                ),
                IPCMessage::WatermarkResult { manual, result } => match result {
                    Ok(()) => {
                        let _ = tray_icon
                            .set_tooltip(Some("THide Hover + UWD2 | Insider patched | Ctrl+Alt+T"));
                        if manual {
                            runtime_log("Manual UWD2 watermark re-patch completed");
                        }
                    }
                    Err(error) => {
                        let _ = tray_icon.set_tooltip(Some(
                            "THide Hover + UWD2 | UWD2 patch failed | Ctrl+Alt+T",
                        ));
                        if manual {
                            show_watermark_error(&error);
                        }
                    }
                },
                IPCMessage::Quit => {
                    taskbar_show(
                        &visibility_state_for_loop.should_hide,
                        &taskbar_manager_for_loop,
                    );
                    elwt.exit();
                }
            }
        }

        if let Ok(menu_event) = menu_channel.try_recv() {
            let event_id = menu_event.id;

            if event_id == show_item.id() {
                visibility_state_for_loop
                    .manual_override
                    .store(true, Ordering::SeqCst);
                visibility_state_for_loop
                    .hover_visible
                    .store(false, Ordering::SeqCst);
                reconcile_taskbar(&visibility_state_for_loop, &taskbar_manager_for_loop);
            } else if event_id == hide_item.id() {
                visibility_state_for_loop
                    .manual_override
                    .store(false, Ordering::SeqCst);
                visibility_state_for_loop
                    .hover_visible
                    .store(false, Ordering::SeqCst);
                reconcile_taskbar(&visibility_state_for_loop, &taskbar_manager_for_loop);
            } else if event_id == toggle_item.id() {
                taskbar_toggle(&visibility_state_for_loop, &taskbar_manager_for_loop);
            } else if event_id == hover_item.id() {
                let enabled = !auto_hover_for_loop.load(Ordering::SeqCst);
                auto_hover_for_loop.store(enabled, Ordering::SeqCst);
                hover_item.set_checked(enabled);
                if enabled {
                    visibility_state_for_loop
                        .manual_override
                        .store(false, Ordering::SeqCst);
                    visibility_state_for_loop
                        .hover_visible
                        .store(false, Ordering::SeqCst);
                } else {
                    visibility_state_for_loop
                        .manual_override
                        .store(true, Ordering::SeqCst);
                    visibility_state_for_loop
                        .hover_visible
                        .store(false, Ordering::SeqCst);
                }
                reconcile_taskbar(&visibility_state_for_loop, &taskbar_manager_for_loop);
            } else if event_id == patch_item.id() {
                spawn_watermark_patch(
                    event_proxy_for_loop.clone(),
                    true,
                    Arc::clone(&watermark_in_progress_for_loop),
                );
            } else if event_id == quit_item.id() {
                taskbar_show(
                    &visibility_state_for_loop.should_hide,
                    &taskbar_manager_for_loop,
                );
                elwt.exit();
            }
        }
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{taskbar_edge_for_rect, TaskbarEdge};
    use windows::Win32::Foundation::RECT;

    const MONITOR: RECT = RECT {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };

    #[test]
    fn detects_horizontal_taskbars() {
        assert_eq!(
            taskbar_edge_for_rect(
                RECT {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 48,
                },
                MONITOR,
            ),
            Some(TaskbarEdge::Top)
        );
        assert_eq!(
            taskbar_edge_for_rect(
                RECT {
                    left: 0,
                    top: 1032,
                    right: 1920,
                    bottom: 1080,
                },
                MONITOR,
            ),
            Some(TaskbarEdge::Bottom)
        );
    }

    #[test]
    fn detects_vertical_taskbars() {
        assert_eq!(
            taskbar_edge_for_rect(
                RECT {
                    left: 0,
                    top: 0,
                    right: 64,
                    bottom: 1080,
                },
                MONITOR,
            ),
            Some(TaskbarEdge::Left)
        );
        assert_eq!(
            taskbar_edge_for_rect(
                RECT {
                    left: 1856,
                    top: 0,
                    right: 1920,
                    bottom: 1080,
                },
                MONITOR,
            ),
            Some(TaskbarEdge::Right)
        );
    }

    #[test]
    fn rejects_rectangles_not_docked_to_an_edge() {
        assert_eq!(
            taskbar_edge_for_rect(
                RECT {
                    left: 100,
                    top: 100,
                    right: 1820,
                    bottom: 148,
                },
                MONITOR,
            ),
            None
        );
    }
}
