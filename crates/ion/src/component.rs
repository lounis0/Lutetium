use crate::geometry::{Rect, Size};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Horizontal,
    Vertical,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Variant {
    Solid,
    Bordered,
    Light,
    Flat,
    Ghost,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticColor {
    Default,
    Primary,
    Success,
    Warning,
    Danger,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonSize {
    Small,
    Medium,
    Large,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ComponentKind {
    Stack {
        direction: Direction,
        gap: f32,
        padding: f32,
    },
    Text {
        text: String,
        heading: bool,
    },
    Button {
        label: String,
        variant: Variant,
        color: SemanticColor,
        size: ButtonSize,
        disabled: bool,
    },
    Input {
        label: String,
        value: String,
        placeholder: String,
        numeric: bool,
        read_only: bool,
    },
    Checkbox {
        label: String,
        checked: bool,
    },
    Switch {
        label: String,
        checked: bool,
    },
    Card {
        title: String,
    },
    Divider,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    id: String,
    pub(crate) kind: ComponentKind,
    pub(crate) children: Vec<Component>,
    pub(crate) bounds: Rect,
    pub(crate) minimum: Size,
    pub(crate) preferred: Option<Size>,
    pub(crate) flex: f32,
}

impl Component {
    fn new(id: impl Into<String>, kind: ComponentKind) -> Self {
        Self {
            id: id.into(),
            kind,
            children: Vec::new(),
            bounds: Rect::default(),
            minimum: Size::new(0.0, 0.0),
            preferred: None,
            flex: 0.0,
        }
    }
    pub fn column(id: impl Into<String>) -> Self {
        Self::new(
            id,
            ComponentKind::Stack {
                direction: Direction::Vertical,
                gap: 0.0,
                padding: 0.0,
            },
        )
    }
    pub fn row(id: impl Into<String>) -> Self {
        let mut component = Self::new(
            id,
            ComponentKind::Stack {
                direction: Direction::Horizontal,
                gap: 0.0,
                padding: 0.0,
            },
        );
        component.minimum = Size::new(0.0, 32.0);
        component.preferred = Some(Size::new(0.0, 40.0));
        component
    }
    pub fn text(id: impl Into<String>, text: impl Into<String>) -> Self {
        let mut c = Self::new(
            id,
            ComponentKind::Text {
                text: text.into(),
                heading: false,
            },
        );
        c.preferred = Some(Size::new(240.0, 24.0));
        c
    }
    pub fn heading(id: impl Into<String>, text: impl Into<String>) -> Self {
        let mut c = Self::new(
            id,
            ComponentKind::Text {
                text: text.into(),
                heading: true,
            },
        );
        c.preferred = Some(Size::new(280.0, 34.0));
        c
    }
    pub fn button(id: impl Into<String>, label: impl Into<String>) -> Self {
        let mut c = Self::new(
            id,
            ComponentKind::Button {
                label: label.into(),
                variant: Variant::Solid,
                color: SemanticColor::Primary,
                size: ButtonSize::Medium,
                disabled: false,
            },
        );
        c.preferred = Some(Size::new(100.0, 36.0));
        c.minimum = Size::new(64.0, 32.0);
        c
    }
    pub fn input(id: impl Into<String>, label: impl Into<String>) -> Self {
        let mut c = Self::new(
            id,
            ComponentKind::Input {
                label: label.into(),
                value: String::new(),
                placeholder: String::new(),
                numeric: false,
                read_only: false,
            },
        );
        c.preferred = Some(Size::new(240.0, 52.0));
        c.minimum = Size::new(120.0, 48.0);
        c
    }
    pub fn number_input(id: impl Into<String>, label: impl Into<String>) -> Self {
        let mut c = Self::input(id, label);
        if let ComponentKind::Input { numeric, .. } = &mut c.kind {
            *numeric = true;
        }
        c
    }
    pub fn checkbox(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(
            id,
            ComponentKind::Checkbox {
                label: label.into(),
                checked: false,
            },
        )
    }
    pub fn switch(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(
            id,
            ComponentKind::Switch {
                label: label.into(),
                checked: false,
            },
        )
    }
    pub fn card(id: impl Into<String>, title: impl Into<String>) -> Self {
        let mut c = Self::new(
            id,
            ComponentKind::Card {
                title: title.into(),
            },
        );
        c.preferred = Some(Size::new(300.0, 180.0));
        c.minimum = Size::new(180.0, 100.0);
        c
    }
    pub fn divider(id: impl Into<String>) -> Self {
        let mut c = Self::new(id, ComponentKind::Divider);
        c.preferred = Some(Size::new(200.0, 1.0));
        c
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn kind(&self) -> &ComponentKind {
        &self.kind
    }
    pub fn children(&self) -> &[Component] {
        &self.children
    }
    pub fn find_mut(&mut self, id: &str) -> Option<&mut Component> {
        if self.id == id {
            return Some(self);
        }
        self.children
            .iter_mut()
            .find_map(|child| child.find_mut(id))
    }
    pub fn find_mut_by_prefix(&mut self, prefix: &str) -> Option<&mut Component> {
        if self.id.starts_with(prefix) {
            return Some(self);
        }
        self.children
            .iter_mut()
            .find_map(|child| child.find_mut_by_prefix(prefix))
    }
    pub fn find(&self, id: &str) -> Option<&Component> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|child| child.find(id))
    }
    pub fn bounds(&self) -> Rect {
        self.bounds
    }
    pub fn push(mut self, child: Component) -> Self {
        self.children.push(child);
        self
    }
    pub fn add(&mut self, child: Component) {
        self.children.push(child);
    }
    pub fn gap(mut self, gap: f32) -> Self {
        if let ComponentKind::Stack { gap: g, .. } = &mut self.kind {
            *g = gap.max(0.0);
        }
        self
    }
    pub fn padding(mut self, padding: f32) -> Self {
        if let ComponentKind::Stack { padding: p, .. } = &mut self.kind {
            *p = padding.max(0.0);
        }
        self
    }
    pub fn flex(mut self, factor: f32) -> Self {
        self.flex = factor.max(0.0);
        self
    }
    pub fn min_size(mut self, width: f32, height: f32) -> Self {
        self.minimum = Size::new(width.max(0.0), height.max(0.0));
        self
    }
    pub fn preferred_size(mut self, width: f32, height: f32) -> Self {
        self.preferred = Some(Size::new(width.max(0.0), height.max(0.0)));
        self
    }
    pub fn variant(mut self, variant: Variant) -> Self {
        if let ComponentKind::Button { variant: v, .. } = &mut self.kind {
            *v = variant;
        }
        self
    }
    pub fn color(mut self, color: SemanticColor) -> Self {
        if let ComponentKind::Button { color: c, .. } = &mut self.kind {
            *c = color;
        }
        self
    }
    pub fn size(mut self, size: ButtonSize) -> Self {
        if let ComponentKind::Button { size: s, .. } = &mut self.kind {
            *s = size;
        }
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        if let ComponentKind::Button { disabled: d, .. } = &mut self.kind {
            *d = disabled;
        }
        self
    }
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        if let ComponentKind::Input { placeholder: p, .. } = &mut self.kind {
            *p = text.into();
        }
        self
    }
    pub fn set_text(&mut self, value: impl Into<String>) {
        if let ComponentKind::Text { text, .. } = &mut self.kind {
            *text = value.into();
        }
    }
    pub fn set_label(&mut self, value: impl Into<String>) {
        if let ComponentKind::Button { label, .. } = &mut self.kind {
            *label = value.into();
        }
    }
    pub fn read_only(mut self, read_only: bool) -> Self {
        if let ComponentKind::Input {
            read_only: current, ..
        } = &mut self.kind
        {
            *current = read_only;
        }
        self
    }
    pub fn value(&self) -> Option<&str> {
        if let ComponentKind::Input { value, .. } = &self.kind {
            Some(value)
        } else {
            None
        }
    }
    pub fn set_value(&mut self, new_value: impl Into<String>) {
        if let ComponentKind::Input { value, .. } = &mut self.kind {
            *value = new_value.into();
        }
    }
    pub fn set_checked(&mut self, checked: bool) {
        match &mut self.kind {
            ComponentKind::Checkbox { checked: c, .. }
            | ComponentKind::Switch { checked: c, .. } => *c = checked,
            _ => {}
        }
    }
    pub fn is_checked(&self) -> Option<bool> {
        match self.kind {
            ComponentKind::Checkbox { checked, .. } | ComponentKind::Switch { checked, .. } => {
                Some(checked)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tree_composes_and_preserves_identity() {
        let tree = Component::column("root")
            .gap(8.0)
            .push(Component::button("ok", "Continue").variant(Variant::Ghost));
        assert_eq!(tree.children().len(), 1);
        assert_eq!(tree.children()[0].id(), "ok");
    }
    #[test]
    fn input_and_selection_state_are_owned_by_components() {
        let mut input = Component::input("name", "Name").placeholder("Type");
        input.set_value("ION");
        assert_eq!(input.value(), Some("ION"));
        let mut toggle = Component::switch("dark", "Dark mode");
        toggle.set_checked(true);
        assert_eq!(toggle.is_checked(), Some(true));
    }
}
