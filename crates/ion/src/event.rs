use crate::{geometry::Point, theme::ThemeMode};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Enter,
    Space,
    Escape,
    Tab,
    Backspace,
    Character(char),
    Other(u32),
}
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    PointerMove(Point),
    PointerDown(Point),
    PointerUp(Point),
    KeyDown(Key),
    KeyUp(Key),
    FocusChanged(Option<String>),
    Resized { width: u32, height: u32 },
    CloseRequested,
    ButtonPressed(String),
    InputChanged { id: String, value: String },
    SelectionChanged { id: String, selected: bool },
    ThemeChanged(ThemeMode),
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn events_carry_component_identity() {
        let e = Event::ButtonPressed("ok".into());
        assert_eq!(e, Event::ButtonPressed("ok".into()));
    }
}
