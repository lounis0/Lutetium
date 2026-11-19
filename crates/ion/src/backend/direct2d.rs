use crate::{
    renderer::{DrawCommand, Frame},
    theme::Color,
};
use std::collections::HashMap;
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct2D::Common::{
                D2D1_ALPHA_MODE_UNKNOWN, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
            },
            Direct2D::{
                D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
                D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_DRAW_TEXT_OPTIONS_CLIP,
                D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
                D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
                D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
                D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
                D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE,
            },
            DirectWrite::{
                DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
                DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_WEIGHT_NORMAL, DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_WORD_WRAPPING_WRAP,
            },
        },
    },
};

type TextStyle = (String, u32, bool);

/// Direct2D uses the default render-target type (hardware where available,
/// with the platform's supported software fallback) and DirectWrite layouts.
pub(super) struct Renderer {
    text_factory: IDWriteFactory,
    target: ID2D1HwndRenderTarget,
    brushes: HashMap<(u8, u8, u8), ID2D1SolidColorBrush>,
    formats: HashMap<TextStyle, IDWriteTextFormat>,
}

impl Renderer {
    pub(super) fn new(hwnd: isize, width: u32, height: u32, dpi: f32) -> Result<Self, String> {
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
                .map_err(|error| format!("Direct2D factory: {error}"))?;
            let text_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
                .map_err(|error| format!("DirectWrite factory: {error}"))?;
            let target_properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN,
                    alphaMode: D2D1_ALPHA_MODE_UNKNOWN,
                },
                dpiX: dpi,
                dpiY: dpi,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let hwnd_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd: HWND(hwnd as *mut _),
                pixelSize: D2D_SIZE_U { width, height },
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };
            let target = factory
                .CreateHwndRenderTarget(&target_properties, &hwnd_properties)
                .map_err(|error| format!("Direct2D window render target: {error}"))?;
            target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);
            Ok(Self {
                text_factory,
                target,
                brushes: HashMap::new(),
                formats: HashMap::new(),
            })
        }
    }

    pub(super) fn resize(&mut self, width: u32, height: u32, dpi: f32) -> Result<(), String> {
        let size = D2D_SIZE_U { width, height };
        unsafe {
            self.target.SetDpi(dpi, dpi);
            self.target.Resize(&size)
        }
        .map_err(|error| format!("Direct2D resize: {error}"))
    }

    fn brush(&mut self, color: Color) -> Result<ID2D1SolidColorBrush, String> {
        let key = (color.r, color.g, color.b);
        if let Some(brush) = self.brushes.get(&key) {
            return Ok(brush.clone());
        }
        let brush = unsafe {
            self.target
                .CreateSolidColorBrush(&d2d_color(color), None)
                .map_err(|error| format!("Direct2D brush: {error}"))?
        };
        self.brushes.insert(key, brush.clone());
        Ok(brush)
    }

    fn text_format(
        &mut self,
        family: &str,
        size: f32,
        bold: bool,
    ) -> Result<IDWriteTextFormat, String> {
        let size = if size.is_finite() {
            size.clamp(1.0, 256.0)
        } else {
            14.0
        };
        let key = (family.to_owned(), size.to_bits(), bold);
        if let Some(format) = self.formats.get(&key) {
            return Ok(format.clone());
        }
        let wide_family: Vec<u16> = family.encode_utf16().chain(Some(0)).collect();
        let format = unsafe {
            self.text_factory
                .CreateTextFormat(
                    PCWSTR(wide_family.as_ptr()),
                    None,
                    if bold {
                        DWRITE_FONT_WEIGHT_BOLD
                    } else {
                        DWRITE_FONT_WEIGHT_NORMAL
                    },
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    PCWSTR::null(),
                )
                .map_err(|error| format!("DirectWrite text format: {error}"))?
        };
        unsafe {
            format
                .SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)
                .map_err(|error| format!("DirectWrite word wrapping: {error}"))?;
            format
                .SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)
                .map_err(|error| format!("DirectWrite text alignment: {error}"))?;
            format
                .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)
                .map_err(|error| format!("DirectWrite paragraph alignment: {error}"))?;
        }
        self.formats.insert(key, format.clone());
        Ok(format)
    }

    pub(super) fn render(&mut self, frame: &Frame, background: Color) -> Result<(), String> {
        unsafe {
            self.target.BeginDraw();
            self.target
                .SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            self.target.Clear(Some(&d2d_color(background)));

            let drawing = (|| {
                for command in frame.commands() {
                    match command {
                        DrawCommand::FillRect { bounds, color } => {
                            let rect = rect(
                                bounds.origin.x,
                                bounds.origin.y,
                                bounds.size.width,
                                bounds.size.height,
                            );
                            let brush = self.brush(*color)?;
                            self.target.FillRectangle(&rect, &brush);
                        }
                        DrawCommand::StrokeRect {
                            bounds,
                            color,
                            width,
                        } => {
                            let rect = rect(
                                bounds.origin.x,
                                bounds.origin.y,
                                bounds.size.width,
                                bounds.size.height,
                            );
                            let brush = self.brush(*color)?;
                            self.target
                                .DrawRectangle(&rect, &brush, safe_width(*width), None);
                        }
                        DrawCommand::RoundRect {
                            bounds,
                            fill,
                            stroke,
                            radius,
                        } => {
                            let bounds = rect(
                                bounds.origin.x,
                                bounds.origin.y,
                                bounds.size.width,
                                bounds.size.height,
                            );
                            let radius = finite(*radius).max(0.0).min(
                                (bounds.right - bounds.left).min(bounds.bottom - bounds.top) * 0.5,
                            );
                            let rounded = D2D1_ROUNDED_RECT {
                                rect: bounds,
                                radiusX: radius,
                                radiusY: radius,
                            };
                            let fill_brush = self.brush(*fill)?;
                            self.target.FillRoundedRectangle(&rounded, &fill_brush);
                            let stroke_brush = self.brush(*stroke)?;
                            // Keep the outline inside the fill bounds. Centering
                            // a stroke on the outer edge clips half its coverage
                            // and can make the corner pixels look uneven.
                            let stroke_width = 1.0;
                            let inset = stroke_width * 0.5;
                            let stroke_rect = D2D_RECT_F {
                                left: bounds.left + inset,
                                top: bounds.top + inset,
                                right: (bounds.right - inset).max(bounds.left + inset),
                                bottom: (bounds.bottom - inset).max(bounds.top + inset),
                            };
                            let stroke_radius = (radius - inset).max(0.0);
                            let stroke_shape = D2D1_ROUNDED_RECT {
                                rect: stroke_rect,
                                radiusX: stroke_radius,
                                radiusY: stroke_radius,
                            };
                            self.target.DrawRoundedRectangle(
                                &stroke_shape,
                                &stroke_brush,
                                stroke_width,
                                None,
                            );
                        }
                        DrawCommand::Line {
                            from,
                            to,
                            color,
                            width,
                        } => {
                            let start = windows_numerics::Vector2 {
                                X: finite(from.0),
                                Y: finite(from.1),
                            };
                            let end = windows_numerics::Vector2 {
                                X: finite(to.0),
                                Y: finite(to.1),
                            };
                            let brush = self.brush(*color)?;
                            self.target
                                .DrawLine(start, end, &brush, safe_width(*width), None);
                        }
                        DrawCommand::Text {
                            bounds,
                            origin,
                            text,
                            color,
                            size: font_size,
                            bold,
                            centered,
                            font_family,
                        } => {
                            let format = self.text_format(font_family, *font_size, *bold)?;
                            let value: Vec<u16> = text.encode_utf16().collect();
                            let text_width = if *centered {
                                finite(bounds.size.width).max(1.0)
                            } else {
                                (finite(bounds.origin.x) + finite(bounds.size.width)
                                    - finite(origin.0))
                                .max(1.0)
                            };
                            let text_height = (finite(bounds.origin.y)
                                + finite(bounds.size.height)
                                - finite(origin.1))
                            .max(1.0);
                            let layout = self
                                .text_factory
                                .CreateTextLayout(&value, &format, text_width, text_height)
                                .map_err(|error| format!("DirectWrite text layout: {error}"))?;
                            let mut metrics =
                                windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_METRICS::default(
                                );
                            layout
                                .GetMetrics(&mut metrics)
                                .map_err(|error| format!("DirectWrite text metrics: {error}"))?;
                            let x = if *centered {
                                finite(origin.0) - metrics.width * 0.5
                            } else {
                                finite(origin.0)
                            };
                            let clip = rect(
                                bounds.origin.x,
                                bounds.origin.y,
                                bounds.size.width,
                                bounds.size.height,
                            );
                            let brush = self.brush(*color)?;
                            self.target.PushAxisAlignedClip(
                                &clip,
                                windows::Win32::Graphics::Direct2D::D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                            );
                            self.target.DrawTextLayout(
                                windows_numerics::Vector2 {
                                    X: x,
                                    Y: finite(origin.1),
                                },
                                &layout,
                                &brush,
                                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            );
                            self.target.PopAxisAlignedClip();
                        }
                    }
                }
                Ok::<(), String>(())
            })();
            let drawing_end = self
                .target
                .EndDraw(None, None)
                .map_err(|error| format!("Direct2D EndDraw: {error}"));
            drawing?;
            drawing_end?;
        }
        Ok(())
    }
}

fn finite(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn safe_width(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.5, 64.0)
    } else {
        1.0
    }
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> D2D_RECT_F {
    let left = finite(x);
    let top = finite(y);
    D2D_RECT_F {
        left,
        top,
        right: left + finite(width).max(0.0),
        bottom: top + finite(height).max(0.0),
    }
}

fn d2d_color(color: Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: f32::from(color.r) / 255.0,
        g: f32::from(color.g) / 255.0,
        b: f32::from(color.b) / 255.0,
        a: 1.0,
    }
}
