#![cfg(windows)]

use lutetium_ion::{Application, Component, Event, Theme};
use std::{
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

#[link(name = "user32")]
unsafe extern "system" {
    fn FindWindowW(class: *const u16, title: *const u16) -> isize;
    fn SendMessageW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> isize;
}

const WM_CLOSE: u32 = 0x0010;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_CHAR: u32 = 0x0102;

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[test]
fn native_window_child() {
    let Some(marker) = std::env::var_os("ION_NATIVE_TEST_MARKER") else {
        return;
    };
    let title = std::env::var("ION_NATIVE_TEST_TITLE").expect("test title");
    let root = Component::column("root")
        .padding(10.0)
        .push(Component::button("ok", "Click"))
        .push(Component::input("name", "Name").preferred_size(240.0, 52.0))
        .push(Component::checkbox("updates", "Updates").preferred_size(120.0, 30.0));
    let marker = std::path::PathBuf::from(marker);
    let result = Application::new(title)
        .size(320, 180)
        .theme(Theme::light())
        .content(root)
        .on_event(move |event, _, _| {
            let line = match event {
                Event::ButtonPressed(id) => Some(format!("button:{id}\n")),
                Event::InputChanged { id, value } => Some(format!("input:{id}:{value}\n")),
                Event::SelectionChanged { id, selected } => {
                    Some(format!("selection:{id}:{selected}\n"))
                }
                _ => None,
            };
            if let Some(line) = line {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&marker)
                    .expect("open callback marker");
                file.write_all(line.as_bytes())
                    .expect("write callback marker");
            }
        })
        .run();
    if let Err(error) = result {
        panic!("native window failed: {error}")
    }
}

#[test]
fn native_window_renders_dispatches_input_and_shuts_down() {
    let id = std::process::id();
    let title = format!("ION 0.5 integration {id}");
    let marker = std::env::temp_dir().join(format!("ion-native-{id}.marker"));
    let _ = std::fs::remove_file(&marker);
    let current = std::env::current_exe().expect("test executable");
    let child = Command::new(current)
        .arg("--exact")
        .arg("native_window_child")
        .arg("--nocapture")
        .env("ION_NATIVE_TEST_MARKER", &marker)
        .env("ION_NATIVE_TEST_TITLE", &title)
        .spawn()
        .expect("launch child window");
    let mut child = ChildGuard(child);
    let title_w = wide(&title);
    let deadline = Instant::now() + Duration::from_secs(15);
    let hwnd = loop {
        let found = unsafe { FindWindowW(std::ptr::null(), title_w.as_ptr()) };
        if found != 0 {
            break found;
        }
        if let Some(status) = child.0.try_wait().expect("poll child") {
            panic!("child exited before creating a window: {status}")
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for ION window"
        );
        thread::sleep(Duration::from_millis(40));
    };
    // The root has 10px padding and the button has a 64x32 minimum. Send native
    // window messages to its client point; this is event-dispatch coverage, not
    // physical mouse automation.
    let click = |x: u32, y: u32| {
        let point = ((y << 16) | x) as isize;
        unsafe {
            SendMessageW(hwnd, WM_LBUTTONDOWN, 1, point);
            SendMessageW(hwnd, WM_LBUTTONUP, 0, point);
        }
    };
    click(42, 26);
    click(20, 70);
    unsafe {
        SendMessageW(hwnd, WM_CHAR, '7' as usize, 0);
    }
    click(20, 110);
    let callback_deadline = Instant::now() + Duration::from_secs(5);
    let expected = "button:ok\ninput:name:7\nselection:updates:true\n";
    loop {
        if std::fs::read_to_string(&marker).is_ok_and(|actual| actual == expected) {
            break;
        }
        assert!(
            Instant::now() < callback_deadline,
            "button, input, and selection events were not all dispatched"
        );
        thread::sleep(Duration::from_millis(20));
    }
    unsafe {
        SendMessageW(hwnd, WM_CLOSE, 0, 0);
    }
    let exit_deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.0.try_wait().expect("poll child exit") {
            assert!(
                status.success(),
                "child window exited unsuccessfully: {status}"
            );
            break;
        }
        assert!(Instant::now() < exit_deadline, "window did not shut down");
        thread::sleep(Duration::from_millis(30));
    }
    let _ = std::fs::remove_file(marker);
}
