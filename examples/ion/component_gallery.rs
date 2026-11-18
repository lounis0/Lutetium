use lutetium_ion::{Application, ButtonSize, Component, Event, SemanticColor, Theme, Variant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut root = Component::column("root").padding(28.0).gap(14.0);
    root.add(Component::heading("title", "ION Components"));
    root.add(Component::text(
        "subtitle",
        "Native controls, shared tokens, real event dispatch.",
    ));
    root.add(Component::divider("divider"));
    root.add(Component::text("buttons-label", "BUTTON VARIANTS"));
    let mut buttons = Component::row("buttons").gap(10.0);
    for (id, label, variant, color) in [
        ("primary", "Primary", Variant::Solid, SemanticColor::Primary),
        (
            "bordered",
            "Bordered",
            Variant::Bordered,
            SemanticColor::Default,
        ),
        ("ghost", "Ghost", Variant::Ghost, SemanticColor::Default),
        ("success", "Success", Variant::Solid, SemanticColor::Success),
    ] {
        buttons.add(
            Component::button(id, label)
                .variant(variant)
                .color(color)
                .size(ButtonSize::Medium)
                .flex(1.0),
        );
    }
    root.add(buttons);
    root.add(Component::text("form-label", "INPUT & SELECTION"));
    root.add(
        Component::input("name", "Display name")
            .placeholder("Ada Lovelace")
            .preferred_size(520.0, 54.0),
    );
    let mut choices = Component::row("choices").gap(20.0);
    choices.add(Component::checkbox("updates", "Product updates"));
    choices.add(Component::switch("dark-mode", "Dark theme"));
    root.add(choices);
    root.add(Component::card("card", "Reusable themed card").preferred_size(520.0, 84.0));
    root.add(Component::text(
        "status",
        "Click buttons or switches to see the shared event/state model.",
    ));

    Application::new("ION Component Gallery")
        .size(720, 520)
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
