use lutetium_ion::{Application, ButtonSize, Component, Event, SemanticColor, Theme, Variant};

type ButtonSpec = (
    &'static str,
    &'static str,
    Variant,
    SemanticColor,
    ButtonSize,
    bool,
);
type ButtonRow = (&'static str, &'static [ButtonSpec]);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut root = Component::column("root")
        .padding(24.0)
        .gap(12.0)
        .max_child_width(1160.0);
    root.add(Component::heading("title", "ION Component Gallery"));
    root.add(Component::text(
        "subtitle",
        "A native showcase of every component in the ION 0.5 starter set.",
    ));
    root.add(Component::divider("divider-top"));
    root.add(Component::text(
        "buttons-label",
        "BUTTONS · VARIANTS · SIZES · COLORS",
    ));

    let button_rows: [ButtonRow; 4] = [
        (
            "buttons-solid",
            &[
                (
                    "solid-small",
                    "Solid / Small",
                    Variant::Solid,
                    SemanticColor::Primary,
                    ButtonSize::Small,
                    false,
                ),
                (
                    "solid-medium",
                    "Solid / Medium",
                    Variant::Solid,
                    SemanticColor::Primary,
                    ButtonSize::Medium,
                    false,
                ),
                (
                    "solid-large",
                    "Solid / Large",
                    Variant::Solid,
                    SemanticColor::Primary,
                    ButtonSize::Large,
                    false,
                ),
                (
                    "solid-disabled",
                    "Disabled",
                    Variant::Solid,
                    SemanticColor::Default,
                    ButtonSize::Medium,
                    true,
                ),
            ],
        ),
        (
            "buttons-styles",
            &[
                (
                    "bordered",
                    "Bordered",
                    Variant::Bordered,
                    SemanticColor::Default,
                    ButtonSize::Medium,
                    false,
                ),
                (
                    "light",
                    "Light",
                    Variant::Light,
                    SemanticColor::Primary,
                    ButtonSize::Medium,
                    false,
                ),
                (
                    "flat",
                    "Flat",
                    Variant::Flat,
                    SemanticColor::Success,
                    ButtonSize::Medium,
                    false,
                ),
                (
                    "ghost",
                    "Ghost",
                    Variant::Ghost,
                    SemanticColor::Danger,
                    ButtonSize::Medium,
                    false,
                ),
            ],
        ),
        (
            "buttons-colors",
            &[
                (
                    "default",
                    "Default",
                    Variant::Solid,
                    SemanticColor::Default,
                    ButtonSize::Medium,
                    false,
                ),
                (
                    "primary",
                    "Primary",
                    Variant::Solid,
                    SemanticColor::Primary,
                    ButtonSize::Medium,
                    false,
                ),
                (
                    "success",
                    "Success",
                    Variant::Solid,
                    SemanticColor::Success,
                    ButtonSize::Medium,
                    false,
                ),
            ],
        ),
        (
            "buttons-colors-alt",
            &[
                (
                    "warning",
                    "Warning",
                    Variant::Solid,
                    SemanticColor::Warning,
                    ButtonSize::Medium,
                    false,
                ),
                (
                    "danger",
                    "Danger",
                    Variant::Solid,
                    SemanticColor::Danger,
                    ButtonSize::Medium,
                    false,
                ),
            ],
        ),
    ];
    for (row_id, specs) in button_rows {
        let mut row = Component::row(row_id).gap(8.0);
        for &(id, label, variant, color, size, disabled) in specs {
            row.add(
                Component::button(id, label)
                    .variant(variant)
                    .color(color)
                    .size(size)
                    .disabled(disabled)
                    .flex(1.0),
            );
        }
        root.add(row);
    }

    root.add(Component::divider("divider-form"));
    root.add(Component::text("form-label", "TEXT & NUMBER INPUTS"));
    let mut inputs = Component::row("inputs").gap(12.0);
    inputs.add(
        Component::input("name", "Text input")
            .placeholder("Ada Lovelace")
            .flex(1.0),
    );
    inputs.add(
        Component::number_input("quantity", "Number input")
            .placeholder("42")
            .flex(1.0),
    );
    inputs.add(
        Component::input("readonly", "Read-only input")
            .read_only(true)
            .preferred_size(160.0, 54.0),
    );
    root.add(inputs);

    root.add(Component::text("selection-label", "SELECTION & CONTAINERS"));
    let mut choices = Component::row("choices").gap(16.0);
    choices.add(Component::checkbox("updates", "Checkbox"));
    choices.add(Component::switch("dark-mode", "Live dark theme"));
    choices.add(
        Component::card("card", "Themed card")
            .min_size(180.0, 72.0)
            .preferred_size(200.0, 72.0)
            .flex(1.0),
    );
    root.add(choices);
    root.add(Component::text(
        "status",
        "Click a button to see its event here.",
    ));
    root.add(Component::divider("divider-bottom"));
    root.add(Component::text(
        "footer",
        "Text · Heading · Stack · Button · Input · NumberInput · Checkbox · Switch · Card · Divider",
    ));

    Application::new("ION 0.5 Component Gallery")
        .size(820, 720)
        .theme(Theme::light())
        .content(root)
        .on_event(|event, tree, theme| match event {
            Event::ButtonPressed(id) => {
                if let Some(status) = tree.find_mut("status") {
                    status.set_text(format!("Pressed: {id}"));
                }
            }
            Event::SelectionChanged { id, selected } if id == "dark-mode" => {
                *theme = if selected {
                    Theme::dark()
                } else {
                    Theme::light()
                };
                if let Some(button) = tree.find_mut_by_prefix("theme-") {
                    button.set_label(if selected {
                        "Switch to light theme"
                    } else {
                        "Switch to dark theme"
                    });
                }
            }
            _ => {}
        })
        .run()?;
    Ok(())
}
