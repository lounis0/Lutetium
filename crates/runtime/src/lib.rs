//! Lutetium native runtime library.
//!
//! Compiled once by `crates/native/build.rs` (rustc, crate-type staticlib)
//! and embedded inside the `lutetium` compiler binary. At native-build time
//! the linker links it into generated executables together with the
//! Cranelift-generated object, so generated programs need neither the
//! compiler nor this source tree at run time.
//!
//! ABI: `extern "system"` (Windows x64 fastcall), matching the Cranelift
//! imports declared by the native backend.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Mutex, OnceLock};

const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
const STD_ERROR_HANDLE: u32 = -12i32 as u32;

#[allow(non_snake_case)]
mod win {
    #[link(name = "kernel32")]
    extern "system" {
        pub fn GetStdHandle(nStdHandle: u32) -> isize;
        pub fn WriteFile(
            hFile: isize,
            lpBuffer: *const u8,
            nNumberOfBytesToWrite: u32,
            lpNumberOfBytesWritten: *mut u32,
            lpOverlapped: *mut u8,
        ) -> i32;
        pub fn GetProcessHeap() -> isize;
        pub fn HeapAlloc(hHeap: isize, dwFlags: u32, dwBytes: usize) -> *mut u8;
        pub fn ExitProcess(uExitCode: u32) -> !;
        pub fn GetModuleHandleW(name: *const u16) -> isize;
        pub fn GetLastError() -> u32;
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn RegisterClassExW(class: *const WndClassExW) -> u16;
        pub fn CreateWindowExW(
            ex_style: u32,
            class_name: *const u16,
            title: *const u16,
            style: u32,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: isize,
            menu: isize,
            instance: isize,
            param: *mut u8,
        ) -> isize;
        pub fn DefWindowProcW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
        pub fn GetMessageW(msg: *mut MsgW, hwnd: isize, min: u32, max: u32) -> i32;
        pub fn TranslateMessage(msg: *const MsgW) -> i32;
        pub fn DispatchMessageW(msg: *const MsgW) -> isize;
        pub fn PostQuitMessage(code: i32);
        pub fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
        pub fn UpdateWindow(hwnd: isize) -> i32;
        pub fn SetWindowTextW(hwnd: isize, text: *const u16) -> i32;
        pub fn LoadCursorW(instance: isize, name: *const u16) -> isize;
        pub fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
        pub fn DestroyWindow(hwnd: isize) -> i32;
    }

    pub const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
    pub const WS_VISIBLE: u32 = 0x10000000;
    pub const WS_CHILD: u32 = 0x40000000;
    pub const WS_TABSTOP: u32 = 0x00010000;
    pub const BS_PUSHBUTTON: u32 = 0;
    pub const SW_SHOW: i32 = 5;
    pub const WM_DESTROY: u32 = 0x0002;
    pub const WM_COMMAND: u32 = 0x0111;
    pub const IDC_ARROW: usize = 32512;
    pub const COLOR_WINDOW: isize = 6;
    pub const SS_LEFT: u32 = 0;
    pub const BN_CLICKED: usize = 0;
    pub const GWLP_USERDATA: i32 = -21;
    pub const SM_CXSCREEN: i32 = 0;
    pub const SM_CYSCREEN: i32 = 1;

    #[repr(C)]
    pub struct WndClassExW {
        pub size: u32,
        pub style: u32,
        pub wnd_proc: Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
        pub cls_extra: i32,
        pub wnd_extra: i32,
        pub instance: isize,
        pub icon: isize,
        pub cursor: isize,
        pub background: isize,
        pub menu_name: *const u16,
        pub class_name: *const u16,
        pub icon_sm: isize,
    }
    #[repr(C)]
    pub struct MsgW {
        pub hwnd: isize,
        pub message: u32,
        pub wparam: usize,
        pub lparam: isize,
        pub time: u32,
        pub pt_x: i32,
        pub pt_y: i32,
    }
    #[repr(C)]
    pub struct RectW {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }
    #[link(name = "user32")]
    extern "system" {
        pub fn AdjustWindowRect(rect: *mut RectW, style: u32, menu: i32) -> i32;
        pub fn GetSystemMetrics(index: i32) -> i32;
    }
}

const GUI_CLASS: &[u16] = &[
    76, 117, 116, 101, 116, 105, 117, 109, 73, 111, 110, 87, 105, 110, 100, 111, 119, 0,
];
type ClickHandler = extern "system" fn(isize);
static GUI_HANDLERS: OnceLock<Mutex<HashMap<(isize, isize), usize>>> = OnceLock::new();

fn gui_handlers() -> &'static Mutex<HashMap<(isize, isize), usize>> {
    GUI_HANDLERS.get_or_init(|| Mutex::new(HashMap::new()))
}

unsafe extern "system" fn ion_wnd_proc(
    hwnd: isize,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    if msg == win::WM_COMMAND && (wparam >> 16) & 0xffff == win::BN_CLICKED && lparam != 0 {
        // WM_COMMAND's lParam is the notifying child HWND. Use it with the
        // parent rather than the 16-bit control ID, which can collide after
        // clamping or when callers reuse an ID.
        let key = (hwnd, lparam);
        let callback = gui_handlers()
            .lock()
            .ok()
            .and_then(|handlers| handlers.get(&key).copied());
        if let Some(callback) = callback {
            // SAFETY: `callback` is emitted by Cranelift as the address of a
            // Lutetium function with one i64 window parameter and no return
            // value. On x86_64 Windows, Rust's `extern "system"` ABI matches
            // Cranelift's WindowsFastcall convention used for that function.
            let callback: ClickHandler = std::mem::transmute(callback);
            // Pass the parent window as the callback context. Release the
            // handler-map lock first: callbacks may destroy this window.
            callback(hwnd);
            return 0;
        }
    }
    if msg == win::WM_DESTROY {
        if let Ok(mut handlers) = gui_handlers().lock() {
            handlers.retain(|(window, _), _| *window != hwnd);
        }
        win::PostQuitMessage(0);
        return 0;
    }
    win::DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn write_to(handle: u32, bytes: &[u8]) {
    let h = unsafe { win::GetStdHandle(handle) };
    if h == 0 || h == -1 {
        return;
    }
    let mut written: u32 = 0;
    unsafe {
        win::WriteFile(
            h,
            bytes.as_ptr(),
            bytes.len() as u32,
            &mut written,
            std::ptr::null_mut(),
        );
    }
}

fn print_line(s: &str) {
    write_to(STD_OUTPUT_HANDLE, s.as_bytes());
    write_to(STD_OUTPUT_HANDLE, b"\n");
}

/// Native float formatting. `{:?}` (Rust Debug) yields the shortest
/// round-trip decimal with a mandatory decimal point (`2.0`, `3.5`), which
/// matches the bytecode VM's format for ordinary values.
fn format_f64(v: f64) -> String {
    format!("{:?}", v)
}

fn gui_error() -> i64 {
    // GUI-subsystem executables do not have a console for eprintln!. Fail
    // visibly and stop instead of returning a null HWND that could leave the
    // subsequent message loop blocked forever.
    let code = unsafe { win::GetLastError() };
    let message: Vec<u16> = format!("Lutetium ION: Win32 operation failed (error {code})")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let caption: Vec<u16> = "Lutetium ION error\0".encode_utf16().collect();
    unsafe {
        win::MessageBoxW(0, message.as_ptr(), caption.as_ptr(), 0x10);
        win::ExitProcess(1);
    }
}

unsafe fn string_arg(ptr: i64) -> Vec<u16> {
    let (bytes, len) = str_parts(ptr as *const u8);
    let text = String::from_utf8_lossy(std::slice::from_raw_parts(bytes, len));
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[no_mangle]
pub extern "system" fn lut_rt_gui_window(title: i64, width: i64, height: i64) -> i64 {
    unsafe {
        let instance = win::GetModuleHandleW(std::ptr::null());
        let cursor = win::LoadCursorW(0, win::IDC_ARROW as *const u16);
        let class = win::WndClassExW {
            size: std::mem::size_of::<win::WndClassExW>() as u32,
            style: 0,
            wnd_proc: Some(ion_wnd_proc),
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: 0,
            cursor,
            background: (win::COLOR_WINDOW + 1) as isize,
            menu_name: std::ptr::null(),
            class_name: GUI_CLASS.as_ptr(),
            icon_sm: 0,
        };
        if win::RegisterClassExW(&class) == 0 && win::GetLastError() != 1410 {
            return gui_error();
        }
        let title = string_arg(title);
        let mut rect = win::RectW {
            left: 0,
            top: 0,
            right: width.clamp(160, 4096) as i32,
            bottom: height.clamp(120, 4096) as i32,
        };
        if win::AdjustWindowRect(&mut rect, win::WS_OVERLAPPEDWINDOW, 0) == 0 {
            return gui_error();
        }
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        let x = (win::GetSystemMetrics(win::SM_CXSCREEN) - w) / 2;
        let y = (win::GetSystemMetrics(win::SM_CYSCREEN) - h) / 2;
        let hwnd = win::CreateWindowExW(
            0,
            GUI_CLASS.as_ptr(),
            title.as_ptr(),
            win::WS_OVERLAPPEDWINDOW,
            x,
            y,
            w,
            h,
            0,
            0,
            instance,
            std::ptr::null_mut(),
        );
        if hwnd == 0 {
            return gui_error();
        }
        hwnd as i64
    }
}

#[no_mangle]
pub extern "system" fn lut_rt_gui_label(
    window: i64,
    id: i64,
    text: i64,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
) -> i64 {
    unsafe {
        let text = string_arg(text);
        let hwnd = win::CreateWindowExW(
            0,
            [83u16, 84, 65, 84, 73, 67, 0].as_ptr(),
            text.as_ptr(),
            win::WS_CHILD | win::WS_VISIBLE | win::SS_LEFT,
            x.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            y.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            width.clamp(1, 8192) as i32,
            height.clamp(1, 8192) as i32,
            window as isize,
            id as usize as isize,
            win::GetModuleHandleW(std::ptr::null()),
            std::ptr::null_mut(),
        );
        if hwnd == 0 {
            gui_error()
        } else {
            hwnd as i64
        }
    }
}

#[no_mangle]
pub extern "system" fn lut_rt_gui_button(
    window: i64,
    id: i64,
    text: i64,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    callback: i64,
) -> i64 {
    unsafe {
        let text = string_arg(text);
        let id = id.clamp(1, u16::MAX as i64) as u16;
        let hwnd = win::CreateWindowExW(
            0,
            [66u16, 85, 84, 84, 79, 78, 0].as_ptr(),
            text.as_ptr(),
            win::WS_CHILD | win::WS_VISIBLE | win::WS_TABSTOP | win::BS_PUSHBUTTON,
            x.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            y.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            width.clamp(1, 8192) as i32,
            height.clamp(1, 8192) as i32,
            window as isize,
            id as usize as isize,
            win::GetModuleHandleW(std::ptr::null()),
            std::ptr::null_mut(),
        );
        if hwnd == 0 {
            return gui_error();
        }
        if let Ok(mut handlers) = gui_handlers().lock() {
            handlers.insert((window as isize, hwnd), callback as usize);
        }
        hwnd as i64
    }
}

#[no_mangle]
pub extern "system" fn lut_rt_gui_set_text(widget: i64, text: i64) {
    unsafe {
        let text = string_arg(text);
        win::SetWindowTextW(widget as isize, text.as_ptr());
    }
}

#[no_mangle]
pub extern "system" fn lut_rt_gui_run(window: i64) {
    unsafe {
        win::ShowWindow(window as isize, win::SW_SHOW);
        win::UpdateWindow(window as isize);
        let mut msg = std::mem::zeroed::<win::MsgW>();
        loop {
            let result = win::GetMessageW(&mut msg, 0, 0, 0);
            if result <= 0 {
                break;
            }
            win::TranslateMessage(&msg);
            win::DispatchMessageW(&msg);
        }
    }
}

#[no_mangle]
pub extern "system" fn lut_rt_gui_close(window: i64) {
    unsafe {
        win::DestroyWindow(window as isize);
    }
}

/// A string value is a pointer to a header: `[len: u64][bytes..]`.
unsafe fn str_parts(p: *const u8) -> (*const u8, usize) {
    let len = *(p as *const u64) as usize;
    (p.add(8), len)
}

#[no_mangle]
pub extern "system" fn lut_rt_print_i64(v: i64) {
    print_line(&v.to_string());
}

#[no_mangle]
pub extern "system" fn lut_rt_print_f64(v: f64) {
    print_line(&format_f64(v));
}

#[no_mangle]
pub extern "system" fn lut_rt_print_bool(v: i32) {
    print_line(if v != 0 { "true" } else { "false" });
}

#[no_mangle]
pub unsafe extern "system" fn lut_rt_print_str(p: *const u8) {
    let (bytes, len) = str_parts(p);
    let slice = std::slice::from_raw_parts(bytes, len);
    match std::str::from_utf8(slice) {
        Ok(s) => print_line(s),
        Err(_) => write_to(STD_OUTPUT_HANDLE, slice),
    }
}

#[no_mangle]
pub unsafe extern "system" fn lut_rt_concat(a: *const u8, b: *const u8) -> *mut u8 {
    let (abytes, alen) = str_parts(a);
    let (bbytes, blen) = str_parts(b);
    let total = 8 + alen + blen;
    let heap = win::GetProcessHeap();
    let out = win::HeapAlloc(heap, 0, total);
    if out.is_null() {
        rt_fail("runtime error: out of memory");
    }
    *(out as *mut u64) = (alen + blen) as u64;
    std::ptr::copy_nonoverlapping(abytes, out.add(8), alen);
    std::ptr::copy_nonoverlapping(bbytes, out.add(8 + alen), blen);
    out
}

#[no_mangle]
pub unsafe extern "system" fn lut_rt_str_eq(a: *const u8, b: *const u8) -> i32 {
    let (abytes, alen) = str_parts(a);
    let (bbytes, blen) = str_parts(b);
    if alen != blen {
        return 0;
    }
    let eq = std::slice::from_raw_parts(abytes, alen) == std::slice::from_raw_parts(bbytes, blen);
    eq as i32
}

fn rt_fail(msg: &str) -> ! {
    let mut line = String::new();
    let _ = writeln!(line, "{}", msg);
    write_to(STD_ERROR_HANDLE, line.as_bytes());
    unsafe { win::ExitProcess(1) }
}

#[no_mangle]
pub extern "system" fn lut_rt_panic_div0() {
    rt_fail("runtime error: integer division by zero");
}

#[no_mangle]
pub extern "system" fn lut_rt_panic_overflow(op: i32) {
    let name = match op {
        0 => "+",
        1 => "-",
        2 => "*",
        _ => "arith",
    };
    let mut msg = String::new();
    let _ = writeln!(msg, "runtime error: integer {} overflow", name);
    write_to(STD_ERROR_HANDLE, msg.as_bytes());
    unsafe { win::ExitProcess(1) }
}
