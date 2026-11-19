use crate::{
    component::{Alignment, Component, ComponentKind, Direction},
    geometry::{Rect, Size},
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
            Self::NonFiniteSize => write!(f, "layout bounds must be finite and non-negative"),
            Self::DuplicateId(id) => write!(f, "duplicate component id `{id}`"),
        }
    }
}
impl std::error::Error for LayoutError {}

/// Compute component bounds within `bounds`.
///
/// Stack children use their preferred size on the stack axis, share remaining
/// space by flex weight, and stretch across the cross axis. Container intrinsic
/// sizes include their children, so controls are not clipped by default stack
/// sizes when they have larger minimum/preferred dimensions.
pub fn compute(root: &mut Component, bounds: Rect) -> Result<(), LayoutError> {
    if !bounds.origin.x.is_finite()
        || !bounds.origin.y.is_finite()
        || bounds.origin.x.abs() > i32::MAX as f32 / 4.0
        || bounds.origin.y.abs() > i32::MAX as f32 / 4.0
        || !bounds.size.width.is_finite()
        || !bounds.size.height.is_finite()
        || bounds.size.width < 0.0
        || bounds.size.height < 0.0
    {
        return Err(LayoutError::NonFiniteSize);
    }
    let mut ids = HashSet::new();
    if let Some(id) = duplicate_id(root, &mut ids) {
        return Err(LayoutError::DuplicateId(id));
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

fn intrinsic(node: &Component) -> Size {
    let content = match &node.kind {
        ComponentKind::Stack {
            direction,
            gap,
            padding,
        } => {
            let children: Vec<Size> = node.children.iter().map(intrinsic).collect();
            let (width, height) = if *direction == Direction::Horizontal {
                (
                    children.iter().map(|s| s.width).sum::<f32>()
                        + gap * node.children.len().saturating_sub(1) as f32,
                    children.iter().map(|s| s.height).fold(0.0, f32::max),
                )
            } else {
                (
                    children.iter().map(|s| s.width).fold(0.0, f32::max),
                    children.iter().map(|s| s.height).sum::<f32>()
                        + gap * node.children.len().saturating_sub(1) as f32,
                )
            };
            Size::new(width + 2.0 * padding, height + 2.0 * padding)
        }
        _ => Size::default(),
    };
    let minimum = node.minimum;
    let preferred = node.preferred.unwrap_or_default();
    Size::new(
        content.width.max(minimum.width).max(preferred.width),
        content.height.max(minimum.height).max(preferred.height),
    )
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
    if node.children.is_empty() {
        return;
    }
    let horizontal = *direction == Direction::Horizontal;
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
    let mut cross = if horizontal {
        inner.size.height
    } else {
        inner.size.width
    };
    let cross_origin = if !horizontal {
        if let Some(max_width) = node.max_child_width {
            let width = max_width.min(cross);
            let origin = inner.origin.x + (cross - width) * 0.5;
            cross = width;
            origin
        } else {
            inner.origin.x
        }
    } else {
        inner.origin.y
    };
    let gap_total = *gap * node.children.len().saturating_sub(1) as f32;
    let free = (available - gap_total).max(0.0);
    let sizes: Vec<Size> = node.children.iter().map(intrinsic).collect();
    let main_sizes = distribute_main_sizes(&node.children, &sizes, free, horizontal);
    let mut offset = 0.0;

    for (index, (child, main)) in node.children.iter_mut().zip(main_sizes).enumerate() {
        let minimum_cross = if horizontal {
            child.minimum.height
        } else {
            child.minimum.width
        };
        // Stack children stretch across the cross axis by default; centered
        // gallery/form stacks can instead retain preferred component sizes.
        let preferred_cross = if horizontal {
            sizes[index].height
        } else {
            sizes[index].width
        };
        let (cross_size, cross_offset) = match node.child_alignment {
            Alignment::Stretch => (cross.max(minimum_cross), 0.0),
            Alignment::Start => (preferred_cross.max(minimum_cross).min(cross), 0.0),
            Alignment::Center => {
                let size = preferred_cross.max(minimum_cross).min(cross);
                (size, (cross - size) * 0.5)
            }
            Alignment::End => {
                let size = preferred_cross.max(minimum_cross).min(cross);
                (size, cross - size)
            }
        };
        let child_bounds = if horizontal {
            Rect::new(
                inner.origin.x + offset,
                cross_origin + cross_offset,
                main,
                cross_size,
            )
        } else {
            Rect::new(
                cross_origin + cross_offset,
                inner.origin.y + offset,
                cross_size,
                main,
            )
        };
        place(child, child_bounds);
        offset += main + *gap;
    }
}

fn distribute_main_sizes(
    children: &[Component],
    measured: &[Size],
    available: f32,
    horizontal: bool,
) -> Vec<f32> {
    let axis = |size: Size| if horizontal { size.width } else { size.height };
    let minimum = |child: &Component| {
        if horizontal {
            child.minimum.width
        } else {
            child.minimum.height
        }
    };
    let mut result = vec![0.0; children.len()];
    let fixed_indices: Vec<usize> = children
        .iter()
        .enumerate()
        .filter_map(|(i, child)| (child.flex <= 0.0).then_some(i))
        .collect();
    let flex_indices: Vec<usize> = children
        .iter()
        .enumerate()
        .filter_map(|(i, child)| (child.flex > 0.0).then_some(i))
        .collect();
    let fixed_total: f32 = fixed_indices.iter().map(|&i| axis(measured[i])).sum();
    let flex_minimum: f32 = flex_indices.iter().map(|&i| minimum(&children[i])).sum();
    let fixed_minimum: f32 = fixed_indices.iter().map(|&i| minimum(&children[i])).sum();

    if fixed_total + flex_minimum <= available {
        for &i in &fixed_indices {
            result[i] = axis(measured[i]);
        }
        let extra = available - fixed_total - flex_minimum;
        let weight: f32 = flex_indices.iter().map(|&i| children[i].flex).sum();
        for &i in &flex_indices {
            result[i] = minimum(&children[i])
                + if weight > 0.0 {
                    extra * children[i].flex / weight
                } else {
                    0.0
                };
        }
        return result;
    }

    // When fixed children do not fit, shrink their preferred excess fairly,
    // without violating each child's minimum. Flex children receive minimums.
    let deficit = fixed_total + flex_minimum - available;
    let shrinkable = (fixed_total - fixed_minimum).max(0.0);
    let shrink = deficit.min(shrinkable);
    for &i in &fixed_indices {
        let excess = (axis(measured[i]) - minimum(&children[i])).max(0.0);
        result[i] = axis(measured[i])
            - if shrinkable > 0.0 {
                shrink * excess / shrinkable
            } else {
                0.0
            };
    }
    for &i in &flex_indices {
        result[i] = minimum(&children[i]);
    }
    result
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
            Rect::new(10.0, 10.0, 64.0, 40.0)
        );
        assert_eq!(
            row.children()[1].bounds(),
            Rect::new(79.0, 10.0, 111.0, 40.0)
        );
    }

    #[test]
    fn vertical_stack_stretches_auto_width_rows_and_sizes_for_child_minimums() {
        let mut root = Component::column("root")
            .padding(10.0)
            .gap(4.0)
            .push(Component::heading("heading", "Title"))
            .push(
                Component::row("form")
                    .push(Component::input("input", "Name").flex(1.0))
                    .push(Component::number_input("number", "Age").flex(1.0)),
            );
        compute(&mut root, Rect::new(0.0, 0.0, 500.0, 300.0)).unwrap();
        let row = root.find("form").unwrap().bounds();
        assert_eq!(row.origin.x, 10.0);
        assert_eq!(row.size.width, 480.0);
        assert!(row.size.height >= 48.0);
        assert_eq!(root.find("input").unwrap().bounds().size.height, 52.0);
        assert_eq!(root.find("number").unwrap().bounds().size.height, 52.0);
    }

    #[test]
    fn vertical_stack_can_cap_and_center_child_width() {
        let mut root = Component::column("root")
            .padding(20.0)
            .max_child_width(300.0)
            .push(Component::button("button", "Centered"));
        compute(&mut root, Rect::new(0.0, 0.0, 800.0, 100.0)).unwrap();
        assert_eq!(
            root.find("button").unwrap().bounds(),
            Rect::new(250.0, 20.0, 300.0, 36.0)
        );
    }

    #[test]
    fn overconstrained_fixed_children_shrink_without_violating_minimums() {
        let mut row = Component::row("row")
            .push(Component::input("a", "A").preferred_size(180.0, 52.0))
            .push(Component::input("b", "B").preferred_size(180.0, 52.0));
        compute(&mut row, Rect::new(0.0, 0.0, 300.0, 60.0)).unwrap();
        assert_eq!(row.children()[0].bounds().size.width, 150.0);
        assert_eq!(row.children()[1].bounds().size.width, 150.0);
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
        assert_eq!(
            compute(&mut root, Rect::new(f32::INFINITY, 0.0, 1.0, 1.0)),
            Err(LayoutError::NonFiniteSize)
        );
    }
}
