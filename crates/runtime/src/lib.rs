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

use std::fmt::Write as _;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

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
        pub fn CreateWindowExW(ex_style: u32, class_name: *const u16, title: *const u16, style: u32, x: i32, y: i32, width: i32, height: i32, parent: isize, menu: isize, instance: isize, param: *mut u8) -> isize;
        pub fn DefWindowProcW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
        pub fn GetMessageW(msg: *mut MsgW, hwnd: isize, min: u32, max: u32) -> i32;
        pub fn TranslateMessage(msg: *const MsgW) -> i32;
        pub fn DispatchMessageW(msg: *const MsgW) -> isize;
        pub fn PostQuitMessage(code: i32);
        pub fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
        pub fn UpdateWindow(hwnd: isize) -> i32;
        pub fn SetWindowTextW(hwnd: isize, text: *const u16) -> i32;
        pub fn GetWindowTextLengthW(hwnd: isize) -> i32;
        pub fn GetWindowTextW(hwnd: isize, text: *mut u16, max: i32) -> i32;
        pub fn SendMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
        pub fn LoadCursorW(instance: isize, name: *const u16) -> isize;
        pub fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }

    pub const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
    pub const WS_VISIBLE: u32 = 0x10000000;
    pub const WS_CHILD: u32 = 0x40000000;
    pub const WS_TABSTOP: u32 = 0x00010000;
    pub const ES_AUTOHSCROLL: u32 = 0x0080;
    pub const BS_PUSHBUTTON: u32 = 0;
    pub const SW_SHOW: i32 = 5;
    pub const WM_DESTROY: u32 = 0x0002;
    pub const WM_COMMAND: u32 = 0x0111;
    pub const WM_GETTEXT: u32 = 0x000D;
    pub const WM_GETTEXTLENGTH: u32 = 0x000E;
    pub const WM_SETTEXT: u32 = 0x000C;
    pub const IDC_ARROW: usize = 32512;
    pub const COLOR_WINDOW: isize = 6;
    pub const WS_BORDER: u32 = 0x00800000;
    pub const WS_VSCROLL: u32 = 0x00200000;
    pub const ES_LEFT: u32 = 0;
    pub const SS_LEFT: u32 = 0;
    pub const WM_NCCREATE: u32 = 0x0081;
    pub const WM_NCDESTROY: u32 = 0x0082;

    #[repr(C)]
    pub struct WndClassExW {
        pub size: u32, pub style: u32, pub wnd_proc: Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
        pub cls_extra: i32, pub wnd_extra: i32, pub instance: isize, pub icon: isize, pub cursor: isize,
        pub background: isize, pub menu_name: *const u16, pub class_name: *const u16, pub icon_sm: isize,
    }
    #[repr(C)]
    pub struct MsgW { pub hwnd: isize, pub message: u32, pub wparam: usize, pub lparam: isize, pub time: u32, pub pt_x: i32, pub pt_y: i32 }
    #[repr(C)]
    pub struct CreateStructW { pub create_params: *mut u8, pub instance: isize, pub menu: isize, pub parent: isize, pub cy: i32, pub cx: i32, pub y: i32, pub x: i32, pub style: i32, pub name: *const u16, pub class: *const u16, pub ex_style: u32 }
}

fn write_to(handle: u32, bytes: &[u8]) {
    let h = unsafe { win::GetStdHandle(handle) };
    if h == 0 || h == -1 {
        return;
    }
    let mut written: u32 = 0;
    unsafe {
        win::WriteFile(h, bytes.as_ptr(), bytes.len() as u32, &mut written, std::ptr::null_mut());
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
