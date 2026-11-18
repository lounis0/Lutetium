use crate::{
    component::{Component, ComponentKind, Direction},
    geometry::{Constraints, Rect, Size},
};
use std::collections::HashSet;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayoutError {
    NonFiniteSize,
    DuplicateId(String),
}
impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFiniteSize => write!(f, "layout size must be finite"),
            Self::DuplicateId(id) => write!(f, "duplicate component id `{id}`"),
        }
    }
}
impl std::error::Error for LayoutError {}
pub fn compute(root: &mut Component, bounds: Rect) -> Result<(), LayoutError> {
    let mut ids = HashSet::new();
    if let Some(id) = duplicate_id(root, &mut ids) {
        return Err(LayoutError::DuplicateId(id));
    }
    if !bounds.size.width.is_finite()
        || !bounds.size.height.is_finite()
        || bounds.size.width < 0.0
        || bounds.size.height < 0.0
    {
        return Err(LayoutError::NonFiniteSize);
    }
    place(root, bounds);
    Ok(())
}
fn duplicate_id(node: &Component, ids: &mut HashSet<String>) -> Option<String> {
    if !ids.insert(node.id().to_owned()) {
        return Some(node.id().to_owned());
    }
    node.children()
        .iter()
        .find_map(|child| duplicate_id(child, ids))
}
fn place(node: &mut Component, bounds: Rect) {
    node.bounds = bounds;
    let ComponentKind::Stack {
        direction,
        gap,
        padding,
    } = &node.kind
    else {
        return;
    };
    let horizontal = *direction == Direction::Horizontal;
    let gap = *gap;
    let padding = *padding;
    if node.children.is_empty() {
        return;
    }
    let inner = Rect::new(
        bounds.origin.x + padding,
        bounds.origin.y + padding,
        (bounds.size.width - 2.0 * padding).max(0.0),
        (bounds.size.height - 2.0 * padding).max(0.0),
    );
    let available = if horizontal {
        inner.size.width
    } else {
        inner.size.height
    };
    let cross = if horizontal {
        inner.size.height
    } else {
        inner.size.width
    };
    let gaps = gap * (node.children.len().saturating_sub(1) as f32);
    let free = (available - gaps).max(0.0);
    let flex_sum: f32 = node.children.iter().map(|c| c.flex).sum();
    let fixed: f32 = node
        .children
        .iter()
        .filter(|c| c.flex == 0.0)
        .map(|c| {
            c.preferred
                .map(|s| {
                    if horizontal {
                        s.width.max(c.minimum.width)
                    } else {
                        s.height.max(c.minimum.height)
                    }
                })
                .unwrap_or(if horizontal {
                    c.minimum.width
                } else {
                    c.minimum.height
                })
        })
        .sum();
    let remaining = (free - fixed).max(0.0);
    let mut offset = 0.0;
    for child in &mut node.children {
        let main = if child.flex > 0.0 && flex_sum > 0.0 {
            remaining * child.flex / flex_sum
        } else {
            child
                .preferred
                .map(|s| {
                    if horizontal {
                        s.width.max(child.minimum.width)
                    } else {
                        s.height.max(child.minimum.height)
                    }
                })
                .unwrap_or(if horizontal {
                    child.minimum.width
                } else {
                    child.minimum.height
                })
        };
        let preferred_cross = child
            .preferred
            .map(|s| if horizontal { s.height } else { s.width })
            .unwrap_or(cross);
        let main_min = if horizontal {
            child.minimum.width
        } else {
            child.minimum.height
        };
        let cross_min = if horizontal {
            child.minimum.height
        } else {
            child.minimum.width
        };
        let (min_width, max_width, min_height, max_height) = if horizontal {
            (
                main_min,
                main.max(main_min),
                cross_min,
                cross.max(cross_min),
            )
        } else {
            (
                cross_min,
                cross.max(cross_min),
                main_min,
                main.max(main_min),
            )
        };
        let constraints = Constraints {
            min: Size::new(min_width, min_height),
            max: Size::new(max_width, max_height),
        };
        let size = constraints.constrain(if horizontal {
            Size::new(main, preferred_cross)
        } else {
            Size::new(preferred_cross, main)
        });
        let child_main = if horizontal { size.width } else { size.height };
        let child_cross = if horizontal { size.height } else { size.width }.min(cross);
        let rect = if horizontal {
            Rect::new(
                inner.origin.x + offset,
                inner.origin.y,
                child_main,
                child_cross,
            )
        } else {
            Rect::new(
                inner.origin.x,
                inner.origin.y + offset,
                child_cross,
                child_main,
            )
        };
        place(child, rect);
        offset += child_main + gap;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Component;
    #[test]
    fn horizontal_layout_respects_gap_padding_minimum_and_flex() {
        let mut row = Component::row("row")
            .padding(10.0)
            .gap(5.0)
            .push(Component::button("fixed", "A").preferred_size(40.0, 30.0))
            .push(Component::button("flex", "B").flex(1.0));
        compute(&mut row, Rect::new(0.0, 0.0, 200.0, 60.0)).unwrap();
        assert_eq!(
            row.children()[0].bounds(),
            Rect::new(10.0, 10.0, 64.0, 32.0)
        );
        assert_eq!(
            row.children()[1].bounds(),
            Rect::new(79.0, 10.0, 111.0, 36.0)
        );
    }
    #[test]
    fn duplicate_ids_are_rejected() {
        let mut root = Component::column("root")
            .push(Component::button("same", "A"))
            .push(Component::button("same", "B"));
        assert_eq!(
            compute(&mut root, Rect::new(0.0, 0.0, 200.0, 100.0)),
            Err(LayoutError::DuplicateId("same".into()))
        );
    }
    #[test]
    fn invalid_bounds_fail() {
        let mut root = Component::column("root");
        assert_eq!(
            compute(&mut root, Rect::new(0.0, 0.0, f32::NAN, 10.0)),
            Err(LayoutError::NonFiniteSize)
        );
    }
}
