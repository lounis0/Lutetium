//! Backend-neutral retained frame commands. Component styling is resolved
//! here; platform renderers only translate primitives to native operations.
use crate::{
    component::{Component, ComponentKind, SemanticColor, Variant},
    geometry::Rect,
    theme::{Color, Theme},
};
#[derive(Clone, Debug, PartialEq)]
pub enum DrawCommand {
    FillRect {
        bounds: Rect,
        color: Color,
    },
    StrokeRect {
        bounds: Rect,
        color: Color,
        width: f32,
    },
    RoundRect {
        bounds: Rect,
        fill: Color,
        stroke: Color,
        radius: f32,
    },
    Line {
        from: (f32, f32),
        to: (f32, f32),
        color: Color,
        width: f32,
    },
    Text {
        bounds: Rect,
        origin: (f32, f32),
        text: String,
        color: Color,
        size: f32,
        bold: bool,
        centered: bool,
        font_family: String,
    },
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    commands: Vec<DrawCommand>,
}
impl Frame {
    pub fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }
}
pub trait Renderer {
    type Error;
    fn render(&mut self, frame: &Frame) -> Result<(), Self::Error>;
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Interaction<'a> {
    pub hover: Option<&'a str>,
    pub pressed: Option<&'a str>,
    pub focused: Option<&'a str>,
}
fn semantic(color: SemanticColor, theme: &Theme) -> (Color, Color) {
    match color {
        SemanticColor::Default => (theme.tokens.surface, theme.tokens.foreground),
        SemanticColor::Primary => (theme.tokens.primary, theme.tokens.on_primary),
        SemanticColor::Success => (theme.tokens.success, Color::rgb(255, 255, 255)),
        SemanticColor::Warning => (theme.tokens.warning, Color::rgb(255, 255, 255)),
        SemanticColor::Danger => (theme.tokens.danger, Color::rgb(255, 255, 255)),
    }
}
#[allow(clippy::too_many_arguments)]
fn text(
    frame: &mut Frame,
    bounds: Rect,
    origin: (f32, f32),
    value: &str,
    color: Color,
    size: f32,
    bold: bool,
    centered: bool,
    font_family: &str,
) {
    frame.commands.push(DrawCommand::Text {
        bounds,
        origin,
        text: value.into(),
        color,
        size,
        bold,
        centered,
        font_family: font_family.into(),
    });
}
fn walk(node: &Component, theme: &Theme, interaction: Interaction<'_>, frame: &mut Frame) {
    let b = node.bounds();
    let t = &theme.tokens;
    match node.kind() {
        ComponentKind::Stack { .. } => {}
        ComponentKind::Text {
            text: value,
            heading,
        } => {
            let font_size = if *heading { 22.0 } else { t.font_size };
            text(
                frame,
                b,
                (
                    b.origin.x,
                    b.origin.y + ((b.size.height - font_size) * 0.5).max(0.0),
                ),
                value,
                t.foreground,
                font_size,
                *heading,
                false,
                &t.font_family,
            );
        }
        ComponentKind::Button {
            label,
            variant,
            color,
            size,
            disabled,
        } => {
            let (base, on) = semantic(*color, theme);
            let over = interaction.hover == Some(node.id());
            let down = interaction.pressed == Some(node.id());
            let fill = if *disabled {
                t.background
            } else if down {
                t.border
            } else if over && *variant != Variant::Solid {
                base
            } else {
                match variant {
                    Variant::Solid => base,
                    Variant::Bordered | Variant::Ghost => t.surface,
                    Variant::Light | Variant::Flat => t.background,
                }
            };
            let fg = if *disabled {
                t.muted
            } else if *variant == Variant::Solid {
                on
            } else if *color == SemanticColor::Default {
                t.foreground
            } else {
                base
            };
            let stroke = if interaction.focused == Some(node.id()) {
                t.primary
            } else if *variant == Variant::Bordered && *color != SemanticColor::Default {
                base
            } else {
                t.border
            };
            frame.commands.push(DrawCommand::RoundRect {
                bounds: b,
                fill,
                stroke,
                radius: t.radius,
            });
            let fs = match size {
                crate::component::ButtonSize::Small => 12.0,
                crate::component::ButtonSize::Medium => t.font_size,
                crate::component::ButtonSize::Large => 16.0,
            };
            text(
                frame,
                b,
                (
                    b.origin.x + b.size.width * 0.5,
                    b.origin.y + ((b.size.height - fs) * 0.5).max(0.0),
                ),
                label,
                fg,
                fs,
                false,
                true,
                &t.font_family,
            );
        }
        ComponentKind::Input {
            label,
            value,
            placeholder,
            read_only,
            ..
        } => {
            text(
                frame,
                b,
                (b.origin.x, b.origin.y + 1.0),
                label,
                t.muted,
                12.0,
                false,
                false,
                &t.font_family,
            );
            let field = Rect::new(
                b.origin.x,
                b.origin.y + 18.0,
                b.size.width,
                (b.size.height - 18.0).max(0.0),
            );
            frame.commands.push(DrawCommand::FillRect {
                bounds: field,
                color: t.surface,
            });
            frame.commands.push(DrawCommand::StrokeRect {
                bounds: field,
                color: if !*read_only && interaction.focused == Some(node.id()) {
                    t.primary
                } else {
                    t.border
                },
                width: 1.0,
            });
            text(
                frame,
                field,
                (
                    field.origin.x + 10.0,
                    field.origin.y + ((field.size.height - t.font_size) * 0.5).max(0.0),
                ),
                if value.is_empty() { placeholder } else { value },
                if value.is_empty() {
                    t.muted
                } else {
                    t.foreground
                },
                t.font_size,
                false,
                false,
                &t.font_family,
            );
        }
        ComponentKind::Checkbox { label, checked } => {
            let r = Rect::new(
                b.origin.x,
                b.origin.y + ((b.size.height - 18.0) * 0.5).max(0.0),
                18.0,
                18.0,
            );
            frame.commands.push(DrawCommand::RoundRect {
                bounds: r,
                fill: if *checked { t.primary } else { t.surface },
                stroke: if interaction.focused == Some(node.id()) {
                    t.primary
                } else {
                    t.border
                },
                radius: 3.0,
            });
            if *checked {
                frame.commands.push(DrawCommand::Line {
                    from: (r.origin.x + 4.0, r.origin.y + 9.0),
                    to: (r.origin.x + 8.0, r.origin.y + 13.0),
                    color: t.on_primary,
                    width: 2.0,
                });
                frame.commands.push(DrawCommand::Line {
                    from: (r.origin.x + 8.0, r.origin.y + 13.0),
                    to: (r.origin.x + 15.0, r.origin.y + 5.0),
                    color: t.on_primary,
                    width: 2.0,
                });
            }
            text(
                frame,
                b,
                (
                    b.origin.x + 26.0,
                    b.origin.y + ((b.size.height - t.font_size) * 0.5).max(0.0),
                ),
                label,
                t.foreground,
                t.font_size,
                false,
                false,
                &t.font_family,
            );
        }
        ComponentKind::Switch { label, checked } => {
            let track = Rect::new(
                b.origin.x,
                b.origin.y + ((b.size.height - 20.0) * 0.5).max(0.0),
                36.0,
                20.0,
            );
            frame.commands.push(DrawCommand::RoundRect {
                bounds: track,
                fill: if *checked { t.primary } else { t.muted },
                stroke: if interaction.focused == Some(node.id()) {
                    t.primary
                } else {
                    t.border
                },
                radius: 10.0,
            });
            let thumb = Rect::new(
                b.origin.x + if *checked { 19.0 } else { 3.0 },
                track.origin.y + 3.0,
                14.0,
                14.0,
            );
            frame.commands.push(DrawCommand::RoundRect {
                bounds: thumb,
                fill: t.surface,
                stroke: t.surface,
                radius: 7.0,
            });
            text(
                frame,
                b,
                (
                    b.origin.x + 44.0,
                    b.origin.y + ((b.size.height - t.font_size) * 0.5).max(0.0),
                ),
                label,
                t.foreground,
                t.font_size,
                false,
                false,
                &t.font_family,
            );
        }
        ComponentKind::Card { title } => {
            frame.commands.push(DrawCommand::RoundRect {
                bounds: b,
                fill: t.surface,
                stroke: t.border,
                radius: t.radius,
            });
            text(
                frame,
                b,
                (b.origin.x + 16.0, b.origin.y + 12.0),
                title,
                t.foreground,
                16.0,
                true,
                false,
                &t.font_family,
            );
        }
        ComponentKind::Divider => frame.commands.push(DrawCommand::Line {
            from: (b.origin.x, b.origin.y),
            to: (b.origin.x + b.size.width, b.origin.y),
            color: t.border,
            width: 1.0,
        }),
    }
    for child in node.children() {
        walk(child, theme, interaction, frame);
    }
}
pub fn build_frame(root: &Component, theme: &Theme, interaction: Interaction<'_>) -> Frame {
    let mut frame = Frame::default();
    walk(root, theme, interaction, &mut frame);
    frame
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{layout, Component};
    #[test]
    fn renderer_resolves_theme_tokens_to_backend_neutral_draw_commands() {
        let mut root = Component::column("root").push(Component::button("ok", "Continue"));
        layout::compute(&mut root, Rect::new(0.0, 0.0, 200.0, 80.0)).unwrap();
        let f = build_frame(&root, &Theme::light(), Interaction::default());
        assert!(f.commands().iter().any(
            |c| matches!(c,DrawCommand::RoundRect{fill,..}if *fill==Theme::light().tokens.primary)
        ));
        assert!(f.commands().iter().any(|command| matches!(
            command,
            DrawCommand::Text { bounds, text, .. }
                if text == "Continue" && *bounds == root.find("ok").unwrap().bounds()
        )));
    }
    #[test]
    fn default_bordered_button_has_visible_text_and_border() {
        let mut root =
            Component::column("root").push(Component::bordered_button("default", "Default"));
        layout::compute(&mut root, Rect::new(0.0, 0.0, 200.0, 80.0)).unwrap();
        let frame = build_frame(&root, &Theme::light(), Interaction::default());
        assert!(frame.commands().iter().any(|command| matches!(
            command,
            DrawCommand::Text { text, color, .. }
                if text == "Default" && *color == Theme::light().tokens.foreground
        )));
        assert!(frame.commands().iter().any(|command| matches!(
            command,
            DrawCommand::RoundRect { stroke, .. }
                if *stroke == Theme::light().tokens.border
        )));
    }

    #[test]
    fn interaction_changes_resolved_frame() {
        let mut root = Component::column("root")
            .push(Component::button("ok", "Ok").variant(Variant::Bordered));
        layout::compute(&mut root, Rect::new(0.0, 0.0, 200.0, 80.0)).unwrap();
        let normal = build_frame(&root, &Theme::light(), Interaction::default());
        let hover = build_frame(
            &root,
            &Theme::light(),
            Interaction {
                hover: Some("ok"),
                ..Interaction::default()
            },
        );
        assert_ne!(normal, hover);
    }
}
