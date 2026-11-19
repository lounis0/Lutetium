use super::BackendError;
use crate::{
    component::{Component, ComponentKind},
    event::{Event, Key},
    geometry::{Point, Rect},
    layout::compute,
    renderer::{build_frame, DrawCommand, Interaction},
    theme::{Color, Theme},
};
use std::{ffi::c_void, mem, ptr};

use super::input::{input_char, key};

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassExW(class: *const WindowClass) -> u16;
    fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: isize,
        menu: isize,
        instance: isize,
        param: *mut c_void,
    ) -> isize;
    fn DefWindowProcW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: isize, index: i32, value: isize) -> isize;
    fn GetMessageW(msg: *mut Message, hwnd: isize, min: u32, max: u32) -> i32;
    fn TranslateMessage(msg: *const Message) -> i32;
    fn DispatchMessageW(msg: *const Message) -> isize;
    fn PostQuitMessage(code: i32);
    fn ShowWindow(hwnd: isize, command: i32) -> i32;
    fn UpdateWindow(hwnd: isize) -> i32;
    fn GetClientRect(hwnd: isize, rect: *mut NativeRect) -> i32;
    fn InvalidateRect(hwnd: isize, rect: *const NativeRect, erase: i32) -> i32;
    fn SetCapture(hwnd: isize) -> isize;
    fn ReleaseCapture() -> i32;
    fn TrackMouseEvent(event: *mut TrackMouseEventStruct) -> i32;
    fn SetFocus(hwnd: isize) -> isize;
    fn GetKeyState(key: i32) -> i16;
    fn LoadCursorW(instance: isize, name: *const u16) -> isize;
    fn SetCursor(cursor: isize) -> isize;
    fn SetWindowPos(
        hwnd: isize,
        insert_after: isize,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    fn DestroyWindow(hwnd: isize) -> i32;
    fn AdjustWindowRect(rect: *mut NativeRect, style: u32, menu: i32) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
    fn GetDpiForSystem() -> u32;
    fn GetDpiForWindow(hwnd: isize) -> u32;
    fn SetProcessDpiAwarenessContext(context: isize) -> i32;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn BeginPaint(hwnd: isize, paint: *mut PaintStruct) -> isize;
    fn EndPaint(hwnd: isize, paint: *const PaintStruct) -> i32;
    fn CreateSolidBrush(color: u32) -> isize;
    fn DeleteObject(object: isize) -> i32;
    fn GetStockObject(index: i32) -> isize;
    fn FillRect(dc: isize, rect: *const NativeRect, brush: isize) -> i32;
    fn SetBkMode(dc: isize, mode: i32) -> i32;
    fn SetTextColor(dc: isize, color: u32) -> u32;
    fn CreateFontW(
        height: i32,
        width: i32,
        escapement: i32,
        orientation: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strike_out: u32,
        char_set: u32,
        out_precision: u32,
        clip_precision: u32,
        quality: u32,
        pitch_and_family: u32,
        face: *const u16,
    ) -> isize;
    fn TextOutW(dc: isize, x: i32, y: i32, text: *const u16, len: i32) -> i32;
    fn GetTextExtentPoint32W(dc: isize, text: *const u16, len: i32, size: *mut TextExtent) -> i32;
    fn SaveDC(dc: isize) -> i32;
    fn RestoreDC(dc: isize, saved: i32) -> i32;
    fn IntersectClipRect(dc: isize, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    fn CreatePen(style: i32, width: i32, color: u32) -> isize;
    fn SelectObject(dc: isize, obj: isize) -> isize;
    fn Rectangle(dc: isize, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    fn RoundRect(
        dc: isize,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        ellipse_w: i32,
        ellipse_h: i32,
    ) -> i32;
    fn MoveToEx(dc: isize, x: i32, y: i32, old: *mut NativePoint) -> i32;
    fn LineTo(dc: isize, x: i32, y: i32) -> i32;
    fn SetMapMode(dc: isize, mode: i32) -> i32;
    fn SetWindowExtEx(dc: isize, x: i32, y: i32, old: *mut NativePoint) -> i32;
    fn SetViewportExtEx(dc: isize, x: i32, y: i32, old: *mut NativePoint) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> isize;
    fn GetLastError() -> u32;
}

const CLASS: &[u16] = &[76, 117, 116, 101, 116, 105, 117, 109, 73, 79, 78, 48, 53, 0];
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;
const IDC_ARROW: *const u16 = 32512usize as *const u16;
const GWLP_USERDATA: i32 = -21;
const WM_NCCREATE: u32 = 0x0081;
const WM_SETCURSOR: u32 = 0x20;
const WM_DESTROY: u32 = 2;
const WM_SIZE: u32 = 5;
const WM_DPICHANGED: u32 = 0x02e0;
const WM_PAINT: u32 = 15;
const WM_CLOSE: u32 = 16;
const WM_ERASEBKGND: u32 = 0x14;
const WM_MOUSEMOVE: u32 = 0x200;
const WM_MOUSELEAVE: u32 = 0x02a3;
const TME_LEAVE: u32 = 0x00000002;
const WM_LBUTTONDOWN: u32 = 0x201;
const WM_LBUTTONUP: u32 = 0x202;
const WM_KEYDOWN: u32 = 0x100;
const WM_KEYUP: u32 = 0x101;
const WM_CHAR: u32 = 0x102;
const VK_SHIFT: i32 = 0x10;
const SW_SHOW: i32 = 5;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const TRANSPARENT: i32 = 1;
const PS_SOLID: i32 = 0;
const MM_ANISOTROPIC: i32 = 8;
const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: isize = -4;
const NULL_BRUSH: i32 = 5;
#[repr(C)]
struct NativePoint {
    x: i32,
    y: i32,
}
#[repr(C)]
struct TextExtent {
    x: i32,
    y: i32,
}
#[repr(C)]
struct NativeRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct Message {
    hwnd: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    x: i32,
    y: i32,
}
#[repr(C)]
struct TrackMouseEventStruct {
    size: u32,
    flags: u32,
    hwnd: isize,
    hover_time: u32,
}
#[repr(C)]
struct PaintStruct {
    dc: isize,
    erase: i32,
    paint: NativeRect,
    restore: i32,
    inc_update: i32,
    reserved: [u8; 32],
}
#[repr(C)]
struct WindowClass {
    size: u32,
    style: u32,
    proc: Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
    cls_extra: i32,
    wnd_extra: i32,
    instance: isize,
    icon: isize,
    cursor: isize,
    background: isize,
    menu: *const u16,
    class: *const u16,
    icon_sm: isize,
}
#[repr(C)]
struct CreateStruct {
    create_params: *mut c_void,
    instance: isize,
    menu: isize,
    parent: isize,
    cy: i32,
    cx: i32,
    y: i32,
    x: i32,
    style: i32,
    name: *const u16,
    class: *const u16,
    ex_style: u32,
}

struct AppState {
    hwnd: isize,
    root: Component,
    theme: Theme,
    handler: crate::EventHandler,
    width: u32,
    height: u32,
    dpi_scale: f32,
    hover: Option<String>,
    pressed: Option<String>,
    focused: Option<String>,
    pointer: Option<Point>,
    tracking_mouse_leave: bool,
    direct2d: Option<super::direct2d::Renderer>,
    direct2d_disabled: bool,
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
fn rgb(c: Color) -> u32 {
    u32::from(c.r) | (u32::from(c.g) << 8) | (u32::from(c.b) << 16)
}
fn native_rect(r: Rect) -> NativeRect {
    NativeRect {
        left: r.origin.x.round() as i32,
        top: r.origin.y.round() as i32,
        right: (r.origin.x + r.size.width).round() as i32,
        bottom: (r.origin.y + r.size.height).round() as i32,
    }
}
fn collect(node: &Component, out: &mut Vec<(String, Rect, bool)>) {
    let interactive = matches!(
        node.kind(),
        ComponentKind::Button {
            disabled: false,
            ..
        } | ComponentKind::Input {
            read_only: false,
            ..
        } | ComponentKind::Checkbox { .. }
            | ComponentKind::Switch { .. }
    );
    out.push((node.id().into(), node.bounds(), interactive));
    for child in node.children() {
        collect(child, out)
    }
}
fn hit(root: &Component, p: Point) -> Option<String> {
    let mut nodes = Vec::new();
    collect(root, &mut nodes);
    nodes.into_iter().rev().find_map(|(id, b, interactive)| {
        if interactive && b.contains(p) {
            Some(id)
        } else {
            None
        }
    })
}
fn focusables(root: &Component) -> Vec<String> {
    let mut nodes = Vec::new();
    collect(root, &mut nodes);
    nodes
        .into_iter()
        .filter_map(|(id, _, interactive)| interactive.then_some(id))
        .collect()
}
fn dispatch(st: &mut AppState, event: Event) -> bool {
    // Do not redraw for no-op key and pointer callbacks. These events are
    // high-frequency; only invalidate when a callback actually changes UI state.
    let previous_root = st.root.clone();
    let previous_theme = st.theme.clone();
    (st.handler)(event, &mut st.root, &mut st.theme);
    let root_changed = previous_root != st.root;
    let theme_changed = previous_theme != st.theme;

    if root_changed {
        let bounds = Rect::new(
            0.0,
            0.0,
            st.width as f32 / st.dpi_scale,
            st.height as f32 / st.dpi_scale,
        );
        let _ = compute(&mut st.root, bounds);
    }
    if (root_changed || theme_changed) && st.hwnd != 0 {
        unsafe { InvalidateRect(st.hwnd, ptr::null(), 0) };
    }
    root_changed || theme_changed
}

#[allow(clippy::too_many_arguments)]
fn draw_styled_text(
    dc: isize,
    text: &str,
    x: i32,
    y: i32,
    color: Color,
    size: f32,
    bold: bool,
    centered: bool,
    font_family: &str,
) {
    let text = wide(text);
    let face = wide(font_family);
    unsafe {
        let font = CreateFontW(
            -(size.round() as i32),
            0,
            0,
            0,
            if bold { 600 } else { 400 },
            0,
            0,
            0,
            1,
            0,
            0,
            5,
            0,
            face.as_ptr(),
        );
        let old = SelectObject(dc, font);
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, rgb(color));
        let width = if centered {
            let mut ext = TextExtent { x: 0, y: 0 };
            GetTextExtentPoint32W(dc, text.as_ptr(), (text.len() - 1) as i32, &mut ext);
            x - ext.x / 2
        } else {
            x
        };
        TextOutW(dc, width, y, text.as_ptr(), (text.len() - 1) as i32);
        SelectObject(dc, old);
        DeleteObject(font);
    }
}
fn paint_frame(dc: isize, frame: &crate::renderer::Frame) {
    for command in frame.commands() {
        match command {
            DrawCommand::FillRect { bounds, color } => unsafe {
                let rect = native_rect(*bounds);
                let brush = CreateSolidBrush(rgb(*color));
                FillRect(dc, &rect, brush);
                DeleteObject(brush);
            },
            DrawCommand::StrokeRect {
                bounds,
                color,
                width,
            } => unsafe {
                let rect = native_rect(*bounds);
                let pen = CreatePen(PS_SOLID, width.round().max(1.0) as i32, rgb(*color));
                let old = SelectObject(dc, pen);
                let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
                Rectangle(dc, rect.left, rect.top, rect.right, rect.bottom);
                SelectObject(dc, old_brush);
                SelectObject(dc, old);
                DeleteObject(pen);
            },
            DrawCommand::RoundRect {
                bounds,
                fill,
                stroke,
                radius,
            } => unsafe {
                let rect = native_rect(*bounds);
                let brush = CreateSolidBrush(rgb(*fill));
                let pen = CreatePen(PS_SOLID, 1, rgb(*stroke));
                let old_pen = SelectObject(dc, pen);
                let old_brush = SelectObject(dc, brush);
                RoundRect(
                    dc,
                    rect.left,
                    rect.top,
                    rect.right,
                    rect.bottom,
                    (*radius * 2.0) as i32,
                    (*radius * 2.0) as i32,
                );
                SelectObject(dc, old_brush);
                SelectObject(dc, old_pen);
                DeleteObject(brush);
                DeleteObject(pen);
            },
            DrawCommand::Line {
                from,
                to,
                color,
                width,
            } => unsafe {
                let pen = CreatePen(PS_SOLID, width.round().max(1.0) as i32, rgb(*color));
                let old = SelectObject(dc, pen);
                let mut old_point = NativePoint { x: 0, y: 0 };
                MoveToEx(
                    dc,
                    from.0.round() as i32,
                    from.1.round() as i32,
                    &mut old_point,
                );
                LineTo(dc, to.0.round() as i32, to.1.round() as i32);
                SelectObject(dc, old);
                DeleteObject(pen);
            },
            DrawCommand::Text {
                bounds,
                origin,
                text,
                color,
                size,
                bold,
                centered,
                font_family,
            } => unsafe {
                let clip = native_rect(*bounds);
                let saved_dc = SaveDC(dc);
                if saved_dc != 0 {
                    IntersectClipRect(dc, clip.left, clip.top, clip.right, clip.bottom);
                    draw_styled_text(
                        dc,
                        text,
                        origin.0.round() as i32,
                        origin.1.round() as i32,
                        *color,
                        *size,
                        *bold,
                        *centered,
                        font_family,
                    );
                    RestoreDC(dc, saved_dc);
                }
            },
        }
    }
}
unsafe fn state<'a>(hwnd: isize) -> Option<&'a mut AppState> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
    if ptr.is_null() {
        None
    } else {
        Some(&mut *ptr)
    }
}
unsafe extern "system" fn wnd_proc(hwnd: isize, msg: u32, w: usize, l: isize) -> isize {
    if msg == WM_NCCREATE {
        let create = &*(l as *const CreateStruct);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.create_params as isize);
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let Some(st) = state(hwnd) else {
        return DefWindowProcW(hwnd, msg, w, l);
    };
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PaintStruct = mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            let background = NativeRect {
                left: 0,
                top: 0,
                right: (st.width as f32 / st.dpi_scale).round() as i32,
                bottom: (st.height as f32 / st.dpi_scale).round() as i32,
            };
            let frame = build_frame(
                &st.root,
                &st.theme,
                Interaction {
                    hover: st.hover.as_deref(),
                    pressed: st.pressed.as_deref(),
                    focused: st.focused.as_deref(),
                },
            );
            let rendered = if st.direct2d_disabled || st.width == 0 || st.height == 0 {
                false
            } else if let Some(renderer) = st.direct2d.as_mut() {
                if renderer.render(&frame, st.theme.tokens.background).is_ok() {
                    true
                } else {
                    // Device loss is recoverable: discard the old target, paint
                    // this frame via GDI, and recreate Direct2D on a later paint.
                    st.direct2d = None;
                    st.direct2d_disabled = true;
                    false
                }
            } else {
                match super::direct2d::Renderer::new(hwnd, st.width, st.height, st.dpi_scale * 96.0)
                {
                    Ok(mut renderer) => {
                        if renderer.render(&frame, st.theme.tokens.background).is_ok() {
                            st.direct2d = Some(renderer);
                            true
                        } else {
                            st.direct2d_disabled = true;
                            false
                        }
                    }
                    Err(_) => {
                        st.direct2d_disabled = true;
                        false
                    }
                }
            };
            if !rendered {
                let dpi = (st.dpi_scale * 96.0).round() as i32;
                let mut old = NativePoint { x: 0, y: 0 };
                SetMapMode(dc, MM_ANISOTROPIC);
                SetWindowExtEx(dc, 96, 96, &mut old);
                SetViewportExtEx(dc, dpi, dpi, &mut old);
                let brush = CreateSolidBrush(rgb(st.theme.tokens.background));
                FillRect(dc, &background, brush);
                DeleteObject(brush);
                paint_frame(dc, &frame);
            }
            EndPaint(hwnd, &ps);
            0
        }
        WM_DPICHANGED => {
            let dpi = (w as u16).max(1) as f32;
            st.dpi_scale = dpi / 96.0;
            if !st.direct2d_disabled && st.width > 0 && st.height > 0 {
                if let Some(renderer) = st.direct2d.as_mut() {
                    if renderer.resize(st.width, st.height, dpi).is_err() {
                        st.direct2d = None;
                    }
                }
            }
            let suggested = &*(l as *const NativeRect);
            SetWindowPos(
                hwnd,
                0,
                suggested.left,
                suggested.top,
                suggested.right - suggested.left,
                suggested.bottom - suggested.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            InvalidateRect(hwnd, ptr::null(), 0);
            0
        }
        WM_SIZE => {
            let mut r = mem::zeroed();
            GetClientRect(hwnd, &mut r);
            st.width = (r.right - r.left).max(0) as u32;
            st.height = (r.bottom - r.top).max(0) as u32;
            st.dpi_scale = GetDpiForWindow(hwnd).max(1) as f32 / 96.0;
            if st.width > 0 && st.height > 0 {
                if std::env::var("LUTETIUM_ION_RENDERER")
                    .is_ok_and(|renderer| renderer.eq_ignore_ascii_case("gdi"))
                {
                    st.direct2d_disabled = true;
                } else {
                    st.direct2d_disabled = false;
                    if let Some(renderer) = st.direct2d.as_mut() {
                        if renderer
                            .resize(st.width, st.height, st.dpi_scale * 96.0)
                            .is_err()
                        {
                            st.direct2d = None;
                        }
                    }
                }
            }
            let w = st.width;
            let h = st.height;
            let logical_width = w as f32 / st.dpi_scale;
            let logical_height = h as f32 / st.dpi_scale;
            let _ = compute(
                &mut st.root,
                Rect::new(0.0, 0.0, logical_width, logical_height),
            );
            dispatch(
                st,
                Event::Resized {
                    width: (w as f32 / st.dpi_scale).round() as u32,
                    height: (h as f32 / st.dpi_scale).round() as u32,
                },
            );
            InvalidateRect(hwnd, ptr::null(), 0);
            0
        }
        WM_SETCURSOR => {
            if ((l as usize >> 16) & 0xffff) == 1 {
                SetCursor(LoadCursorW(0, IDC_ARROW));
                1
            } else {
                DefWindowProcW(hwnd, msg, w, l)
            }
        }
        WM_MOUSEMOVE => {
            if !st.tracking_mouse_leave {
                let mut event = TrackMouseEventStruct {
                    size: mem::size_of::<TrackMouseEventStruct>() as u32,
                    flags: TME_LEAVE,
                    hwnd,
                    hover_time: 0,
                };
                st.tracking_mouse_leave = TrackMouseEvent(&mut event) != 0;
            }
            let p = Point::new(
                l as u16 as f32 / st.dpi_scale,
                (l as usize >> 16) as u16 as f32 / st.dpi_scale,
            );
            let next = hit(&st.root, p);
            if next != st.hover {
                st.hover = next;
                InvalidateRect(hwnd, ptr::null(), 0);
            }
            if st.pressed.is_some() {
                SetCursor(LoadCursorW(0, IDC_ARROW));
            }
            if st.pointer != Some(p) {
                st.pointer = Some(p);
                dispatch(st, Event::PointerMove(p));
            }
            0
        }
        WM_MOUSELEAVE => {
            st.tracking_mouse_leave = false;
            st.pointer = None;
            if st.hover.take().is_some() {
                InvalidateRect(hwnd, ptr::null(), 0);
            }
            0
        }
        WM_LBUTTONDOWN => {
            let p = Point::new(
                l as u16 as f32 / st.dpi_scale,
                (l as usize >> 16) as u16 as f32 / st.dpi_scale,
            );
            st.pressed = hit(&st.root, p);
            st.focused = st.pressed.clone();
            SetCapture(hwnd);
            SetFocus(hwnd);
            dispatch(st, Event::PointerDown(p));
            dispatch(st, Event::FocusChanged(st.focused.clone()));
            InvalidateRect(hwnd, ptr::null(), 0);
            0
        }
        WM_LBUTTONUP => {
            let p = Point::new(
                l as u16 as f32 / st.dpi_scale,
                (l as usize >> 16) as u16 as f32 / st.dpi_scale,
            );
            let target = hit(&st.root, p);
            let pressed = st.pressed.take();
            ReleaseCapture();
            dispatch(st, Event::PointerUp(p));
            if pressed.is_some() && pressed == target {
                if let Some(id) = target {
                    match st.root.find(&id).map(|c| c.kind().clone()) {
                        Some(ComponentKind::Button {
                            disabled: false, ..
                        }) => {
                            dispatch(st, Event::ButtonPressed(id));
                        }
                        Some(ComponentKind::Checkbox { checked, .. })
                        | Some(ComponentKind::Switch { checked, .. }) => {
                            if let Some(c) = st.root.find_mut(&id) {
                                c.set_checked(!checked);
                            }
                            dispatch(
                                st,
                                Event::SelectionChanged {
                                    id,
                                    selected: !checked,
                                },
                            );
                        }
                        Some(ComponentKind::Input {
                            read_only: false, ..
                        }) => {
                            st.focused = Some(id);
                            SetFocus(hwnd);
                        }
                        _ => {}
                    }
                }
            }
            InvalidateRect(hwnd, ptr::null(), 0);
            0
        }
        WM_KEYDOWN => {
            let key = key(w as u32);
            if key == Key::Tab {
                let ids = focusables(&st.root);
                if !ids.is_empty() {
                    let current = st
                        .focused
                        .as_ref()
                        .and_then(|id| ids.iter().position(|x| x == id));
                    let backwards = GetKeyState(VK_SHIFT) < 0;
                    let next = match (current, backwards) {
                        (None, false) => 0,
                        (None, true) => ids.len() - 1,
                        (Some(index), false) => (index + 1) % ids.len(),
                        (Some(index), true) => (index + ids.len() - 1) % ids.len(),
                    };
                    st.focused = Some(ids[next].clone());
                    if !dispatch(st, Event::FocusChanged(st.focused.clone())) {
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                }
            } else if matches!(key, Key::Enter | Key::Space) && (l as usize & (1 << 30)) == 0 {
                if let Some(id) = st.focused.clone() {
                    match st.root.find(&id).map(|c| c.kind().clone()) {
                        Some(ComponentKind::Button {
                            disabled: false, ..
                        }) => {
                            dispatch(st, Event::ButtonPressed(id));
                        }
                        Some(ComponentKind::Checkbox { checked, .. })
                        | Some(ComponentKind::Switch { checked, .. }) => {
                            if let Some(c) = st.root.find_mut(&id) {
                                c.set_checked(!checked);
                            }
                            if !dispatch(
                                st,
                                Event::SelectionChanged {
                                    id,
                                    selected: !checked,
                                },
                            ) {
                                InvalidateRect(hwnd, ptr::null(), 0);
                            }
                        }
                        _ => {}
                    }
                }
            }
            dispatch(st, Event::KeyDown(key));
            0
        }
        WM_KEYUP => {
            dispatch(st, Event::KeyUp(key(w as u32)));
            0
        }
        WM_CHAR => {
            let mut changed = None;
            if let Some(id) = st.focused.clone() {
                if let Some(c) = st.root.find_mut(&id) {
                    if let ComponentKind::Input {
                        value,
                        numeric,
                        read_only,
                        ..
                    } = &mut c.kind
                    {
                        if *read_only {
                            return 0;
                        }
                        if input_char(value, w as u32, *numeric) {
                            changed = Some((id, value.clone()));
                        }
                    }
                }
            }
            if let Some((id, value)) = changed {
                if !dispatch(st, Event::InputChanged { id, value }) {
                    InvalidateRect(hwnd, ptr::null(), 0);
                }
            }
            0
        }
        WM_CLOSE => {
            dispatch(st, Event::CloseRequested);
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

pub fn run(app: crate::Application) -> Result<(), BackendError> {
    let (title, size, theme, mut root, handler) = app.into_parts();
    let width = size.width as u32;
    let height = size.height as u32;
    compute(&mut root, Rect::new(0.0, 0.0, size.width, size.height))
        .map_err(|e| BackendError(e.to_string()))?;
    let mut app_state = Box::new(AppState {
        hwnd: 0,
        root,
        theme,
        handler,
        width,
        height,
        hover: None,
        pressed: None,
        focused: None,
        pointer: None,
        tracking_mouse_leave: false,
        dpi_scale: 1.0,
        direct2d: None,
        direct2d_disabled: std::env::var("LUTETIUM_ION_RENDERER")
            .is_ok_and(|renderer| renderer.eq_ignore_ascii_case("gdi")),
    });
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let initial_dpi = GetDpiForSystem().max(1) as f32;
        app_state.dpi_scale = initial_dpi / 96.0;
        let pixel_width = (width as f32 * app_state.dpi_scale).round() as u32;
        let pixel_height = (height as f32 * app_state.dpi_scale).round() as u32;
        let instance = GetModuleHandleW(ptr::null());
        let class = WindowClass {
            size: mem::size_of::<WindowClass>() as u32,
            style: 0,
            proc: Some(wnd_proc),
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: 0,
            cursor: LoadCursorW(0, IDC_ARROW),
            background: 0,
            menu: ptr::null(),
            class: CLASS.as_ptr(),
            icon_sm: 0,
        };
        if RegisterClassExW(&class) == 0 && GetLastError() != 1410 {
            return Err(BackendError(format!(
                "RegisterClassExW failed ({})",
                GetLastError()
            )));
        }
        let title = wide(&title);
        let mut rect = NativeRect {
            left: 0,
            top: 0,
            right: pixel_width as i32,
            bottom: pixel_height as i32,
        };
        AdjustWindowRect(&mut rect, WS_OVERLAPPEDWINDOW, 0);
        let x = (GetSystemMetrics(SM_CXSCREEN) - (rect.right - rect.left)) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - (rect.bottom - rect.top)) / 2;
        let hwnd = CreateWindowExW(
            0,
            CLASS.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            x,
            y,
            rect.right - rect.left,
            rect.bottom - rect.top,
            0,
            0,
            instance,
            &mut *app_state as *mut AppState as *mut c_void,
        );
        if hwnd == 0 {
            return Err(BackendError(format!(
                "CreateWindowExW failed ({})",
                GetLastError()
            )));
        }
        app_state.hwnd = hwnd;
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        let mut msg: Message = mem::zeroed();
        loop {
            let result = GetMessageW(&mut msg, 0, 0, 0);
            if result < 0 {
                return Err(BackendError("GetMessageW failed".into()));
            }
            if result == 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
