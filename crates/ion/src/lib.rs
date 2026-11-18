//! ION 0.5: a small, backend-independent component and theme model with a
//! native Windows/GDI application backend. The core crate has no OS bindings.
pub mod component;
pub mod events {
    //! Stable application event types.
    pub use crate::event::{Event, Key};
}
mod event;
pub mod input {
    //! Keyboard and pointer input event types shared across backends.
    pub use crate::event::Key;
}
mod geometry;
pub mod layout;
pub mod renderer;
pub mod theme;
pub mod state {
    //! Component-owned mutable values exposed through `Component` methods.
    pub use crate::component::Component;
}
pub mod accessibility {
    //! Components expose stable identifiers. Native accessibility APIs are
    //! not yet implemented and are not implied by this module.
    pub use crate::Component;
}
pub mod typography {
    //! System-font typography tokens are part of the shared theme.
    pub use crate::theme::Tokens;
}
pub mod window {
    //! Window/application configuration is currently provided by `Application`.
    pub use crate::Application;
}
pub mod components {
    //! Reusable native controls and layout nodes.
    pub use crate::{ButtonSize, Component, ComponentKind, Direction, SemanticColor, Variant};
}
pub mod layout_api {
    //! Backend-independent geometry and stack layout.
    pub use crate::{layout, Constraints, LayoutError, Point, Rect, Size};
}
pub mod renderer_api {
    //! Backend-independent draw commands and renderer extension contract.
    pub use crate::renderer::{build_frame, DrawCommand, Frame, Interaction, Renderer};
}
pub mod theme_api {
    //! Shared semantic tokens and theme presets.
    pub use crate::{Color, Theme, ThemeMode, Tokens};
}
pub mod style {
    //! Component variants and semantic colors.
    pub use crate::{SemanticColor, Variant};
}
pub mod application {
    pub use crate::Application;
}
pub mod backend;

pub use component::{ButtonSize, Component, ComponentKind, Direction, SemanticColor, Variant};
pub use event::{Event, Key};
pub use geometry::{Constraints, Point, Rect, Size};
pub use layout::{compute as layout, LayoutError};
pub use theme::{Color, Theme, ThemeMode, Tokens};

/// A native ION application. The component tree and event contract are
/// platform-independent; `run` selects the native backend for this target.
pub struct Application {
    title: String,
    size: Size,
    theme: Theme,
    root: Component,
    on_event: Box<dyn FnMut(Event, &mut Component, &mut Theme)>,
}

impl Application {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            size: Size::new(800.0, 600.0),
            theme: Theme::light(),
            root: Component::column("root").padding(24.0).gap(16.0),
            on_event: Box::new(|_, _, _| {}),
        }
    }

    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.size = Size::new(width.max(240) as f32, height.max(180) as f32);
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    pub fn content(mut self, root: Component) -> Self {
        self.root = root;
        self
    }

    pub fn on_event(
        mut self,
        handler: impl FnMut(Event, &mut Component, &mut Theme) + 'static,
    ) -> Self {
        self.on_event = Box::new(handler);
        self
    }

    /// Run the native window and event loop. The current native backend is
    /// Windows-only; other targets return a clear unsupported-platform error.
    pub fn run(self) -> Result<(), backend::BackendError> {
        backend::run(self)
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        String,
        Size,
        Theme,
        Component,
        Box<dyn FnMut(Event, &mut Component, &mut Theme)>,
    ) {
        (self.title, self.size, self.theme, self.root, self.on_event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_defaults_and_builder_are_stable() {
        let app = Application::new("test").size(640, 480).theme(Theme::dark());
        let (title, size, theme, root, _) = app.into_parts();
        assert_eq!(title, "test");
        assert_eq!(size, Size::new(640.0, 480.0));
        assert_eq!(theme.mode, ThemeMode::Dark);
        assert_eq!(root.id(), "root");
    }
}
