//! End-to-end Windows GUI smoke test: compile and launch a Lutetium
//! window/button, trigger the real Win32 button control's click action,
//! dispatch it to a Lutetium callback, and verify clean shutdown.

use lu_compiler::compile_ir;
use lu_native::build_gui_exe;
use std::ops::{Deref, DerefMut};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

#[link(name = "user32")]
unsafe extern "system" {
    fn FindWindowW(class_name: *const u16, window_name: *const u16) -> isize;
    fn FindWindowExW(
        parent: isize,
        child_after: isize,
        class_name: *const u16,
        window_name: *const u16,
    ) -> isize;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn SendMessageW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> isize;
}

struct ChildGuard(Child);

impl Deref for ChildGuard {
    type Target = Child;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ChildGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

const WINDOW_TITLE: &str = "Lutetium ION native GUI smoke test";
const BM_CLICK: u32 = 0x00F5;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wait_for_window(child: &mut Child, title: &[u16]) -> isize {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let window = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
        if window != 0 {
            return window;
        }
        if let Some(status) = child.try_wait().expect("poll GUI process") {
            panic!("GUI process exited before creating its window: {status}");
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the GUI window"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn wait_for_visible(child: &mut Child, window: isize) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if unsafe { IsWindowVisible(window) } != 0 {
            return;
        }
        if let Some(status) = child.try_wait().expect("poll GUI process") {
            panic!("GUI process exited before showing its window: {status}");
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the GUI window to show"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_exit(child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().expect("poll GUI process exit") {
            assert!(
                status.success(),
                "GUI process exited unsuccessfully: {status}"
            );
            return;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Lutetium callback did not close the GUI process after the click");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn native_button_click_calls_lutetium_handler_and_shuts_down() {
    let source = format!(
        "fn ignore_click(window: window) {{}}\n\nfn on_click(window: window) {{\n    gui_close(window)\n}}\n\nfn main() {{\n    let window = gui_window(\"{}\", 480, 320)\n    gui_button(window, 101, \"Ignore\", 24, 24, 140, 36, ignore_click)\n    gui_button(window, 101, \"Click me\", 24, 72, 140, 36, on_click)\n    gui_run(window)\n}}\n",
        WINDOW_TITLE
    );
    let ir = compile_ir("gui_smoke.lu", &source).expect("GUI source should type-check and lower");
    let dir = std::env::temp_dir().join("lutetium_native_gui_tests");
    std::fs::create_dir_all(&dir).expect("create GUI test directory");
    let exe = dir.join("button_callback.exe");
    assert!(lu_native::build_exe(&ir, &exe)
        .expect_err("console target must reject GUI IR")
        .contains("--target gui"));
    build_gui_exe(&ir, &exe).expect("GUI executable should link");

    let pe = std::fs::read(&exe).expect("read generated GUI executable");
    assert!(
        pe.len() >= 0x40 && &pe[..2] == b"MZ",
        "missing PE DOS header"
    );
    let pe_offset = u32::from_le_bytes(pe[0x3c..0x40].try_into().unwrap()) as usize;
    let subsystem_offset = pe_offset + 4 + 20 + 68;
    assert_eq!(pe.get(pe_offset..pe_offset + 4), Some(b"PE\0\0".as_slice()));
    assert_eq!(
        u16::from_le_bytes(
            pe.get(subsystem_offset..subsystem_offset + 2)
                .expect("PE optional header has a subsystem field")
                .try_into()
                .unwrap()
        ),
        2,
        "GUI executable must use IMAGE_SUBSYSTEM_WINDOWS_GUI"
    );
    let mut child = ChildGuard(Command::new(&exe).spawn().expect("launch GUI executable"));
    let title = wide(WINDOW_TITLE);
    let window = wait_for_window(&mut child, &title);
    wait_for_visible(&mut child, window);
    let button_class = wide("BUTTON");
    let first_button = unsafe { FindWindowExW(window, 0, button_class.as_ptr(), std::ptr::null()) };
    let second_button = unsafe {
        FindWindowExW(
            window,
            first_button,
            button_class.as_ptr(),
            std::ptr::null(),
        )
    };
    if first_button == 0 || second_button == 0 {
        let _ = child.kill();
        let _ = child.wait();
        panic!("expected two native BUTTON child controls sharing an ID");
    }

    // Both controls intentionally share ID 101 but have different callbacks.
    // The first must not close the window; the second must. This verifies that
    // dispatch identifies the notifying child HWND, not just the 16-bit ID.
    // BM_CLICK deterministically exercises BN_CLICKED -> WM_COMMAND without
    // moving the user's desktop cursor.
    unsafe { SendMessageW(first_button, BM_CLICK, 0, 0) };
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        child
            .try_wait()
            .expect("poll after ignore button")
            .is_none(),
        "duplicate button ID dispatched the wrong callback"
    );
    unsafe { SendMessageW(second_button, BM_CLICK, 0, 0) };
    wait_for_exit(&mut child);
    std::fs::remove_file(&exe).ok();
    std::fs::remove_file(exe.with_extension("obj")).ok();
    std::fs::remove_file(exe.with_extension("runtime.a")).ok();
    std::fs::remove_dir(&dir).ok();
}
