#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeMode {
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tokens {
    pub background: Color,
    pub surface: Color,
    pub foreground: Color,
    pub muted: Color,
    pub border: Color,
    pub primary: Color,
    pub on_primary: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub radius: f32,
    pub spacing: f32,
    pub font_size: f32,
    pub font_family: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub mode: ThemeMode,
    pub tokens: Tokens,
}
impl Theme {
    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            tokens: Tokens {
                background: Color::rgb(248, 249, 251),
                surface: Color::rgb(255, 255, 255),
                foreground: Color::rgb(24, 28, 36),
                muted: Color::rgb(104, 112, 124),
                border: Color::rgb(220, 224, 231),
                primary: Color::rgb(43, 91, 215),
                on_primary: Color::rgb(255, 255, 255),
                success: Color::rgb(30, 128, 87),
                warning: Color::rgb(171, 105, 12),
                danger: Color::rgb(190, 54, 62),
                radius: 7.0,
                spacing: 8.0,
                font_size: 14.0,
                font_family: "Segoe UI".into(),
            },
        }
    }
    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            tokens: Tokens {
                background: Color::rgb(22, 24, 29),
                surface: Color::rgb(31, 34, 41),
                foreground: Color::rgb(239, 241, 245),
                muted: Color::rgb(161, 168, 180),
                border: Color::rgb(59, 64, 75),
                primary: Color::rgb(104, 143, 255),
                on_primary: Color::rgb(15, 22, 40),
                success: Color::rgb(82, 190, 142),
                warning: Color::rgb(235, 179, 91),
                danger: Color::rgb(244, 113, 122),
                radius: 7.0,
                spacing: 8.0,
                font_size: 14.0,
                font_family: "Segoe UI".into(),
            },
        }
    }
}
impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn light_and_dark_themes_keep_semantic_contrast() {
        let light = Theme::light();
        let dark = Theme::dark();
        assert_ne!(light.tokens.background, dark.tokens.background);
        assert_ne!(light.tokens.primary, dark.tokens.primary);
        assert_eq!(light.tokens.radius, dark.tokens.radius);
        assert_eq!(light.tokens.spacing, dark.tokens.spacing);
    }
    #[test]
    fn custom_tokens_are_owned_and_mutable() {
        let mut theme = Theme::light();
        theme.tokens.radius = 12.0;
        assert_eq!(theme.tokens.radius, 12.0);
    }
}
