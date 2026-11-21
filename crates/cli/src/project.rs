//! Minimal, dependency-free Lutetium project manifest and scaffolding.
//!
//! The supported manifest is intentionally small and strict so project
//! discovery stays predictable while the language has no module system.

use std::path::{Component, Path, PathBuf};

pub const MANIFEST: &str = "lutetium.toml";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Template {
    Console,
    Gui,
}

impl Template {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Console => "console",
            Self::Gui => "gui",
        }
    }

    fn source(self, name: &str) -> String {
        match self {
            Self::Console => format!(
                "fn main() {{\n    print(\"Hello from {name}\")\n}}\n"
            ),
            Self::Gui => format!(
                "fn on_close(window: window) {{\n    gui_close(window)\n}}\n\nfn main() {{\n    let window = gui_window(\"{name}\", 640, 400)\n    gui_button(window, 1, \"Close\", 24, 24, 120, 36, on_close)\n    gui_run(window)\n}}\n"
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
    pub entry: PathBuf,
    pub kind: Template,
}

impl Project {
    pub fn source_path(&self) -> PathBuf {
        self.root.join(&self.entry)
    }

    pub fn output_path(&self, extension: &str) -> PathBuf {
        self.root
            .join("dist")
            .join(format!("{}.{}", self.name, extension))
    }
}

pub fn parse_manifest(root: &Path, input: &str) -> Result<Project, String> {
    let mut name = None;
    let mut entry = None;
    let mut kind = None;

    for (index, raw_line) in input.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            return Err(format!(
                "{}:{line_number}: expected `key = \"value\"`",
                MANIFEST
            ));
        };
        let key = key.trim();
        let value = parse_string(raw_value.trim())
            .map_err(|message| format!("{}:{line_number}: {message}", MANIFEST))?;
        let destination = match key {
            "name" => &mut name,
            "entry" => &mut entry,
            "kind" => &mut kind,
            other => {
                return Err(format!(
                    "{}:{line_number}: unknown key `{other}` (supported: name, entry, kind)",
                    MANIFEST
                ));
            }
        };
        if destination.replace(value).is_some() {
            return Err(format!("{}:{line_number}: duplicate key `{key}`", MANIFEST));
        }
    }

    let name = name.ok_or_else(|| format!("{}: missing required `name`", MANIFEST))?;
    validate_name(&name)?;
    let entry = entry.ok_or_else(|| format!("{}: missing required `entry`", MANIFEST))?;
    let entry_path = PathBuf::from(&entry);
    if entry_path.as_os_str().is_empty()
        || entry_path.is_absolute()
        || entry_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || entry_path
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("lu")
    {
        return Err(format!(
            "{}: `entry` must be a project-relative `.lu` path without `.` or `..` components",
            MANIFEST
        ));
    }
    let kind = match kind.as_deref() {
        Some("console") => Template::Console,
        Some("gui") => Template::Gui,
        Some(value) => {
            return Err(format!(
                "{}: unsupported kind `{value}` (expected `console` or `gui`)",
                MANIFEST
            ));
        }
        None => return Err(format!("{}: missing required `kind`", MANIFEST)),
    };

    Ok(Project {
        root: root.to_path_buf(),
        name,
        entry: entry_path,
        kind,
    })
}

fn parse_string(value: &str) -> Result<String, String> {
    if value.len() < 2 || !value.starts_with('"') || !value.ends_with('"') {
        return Err("expected a double-quoted string value".into());
    }
    let inner = &value[1..value.len() - 1];
    if inner.contains(['"', '\\', '\n', '\r', '\0']) {
        return Err(
            "manifest strings cannot contain quotes, escapes, or control characters".into(),
        );
    }
    Ok(inner.to_owned())
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(
            "project name must contain only ASCII letters, digits, `_`, or `-` (and cannot be `.` or `..`)".into(),
        );
    }
    Ok(())
}

pub fn create_project(parent: &Path, name: &str, template: Template) -> Result<PathBuf, String> {
    validate_name(name)?;
    let root = parent.join(name);
    std::fs::create_dir(&root).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            format!("cannot create project: `{}` already exists", root.display())
        } else {
            format!("cannot create `{}`: {error}", root.display())
        }
    })?;

    let result = (|| {
        let src = root.join("src");
        std::fs::create_dir(&src)
            .map_err(|error| format!("cannot create `{}`: {error}", src.display()))?;
        let manifest = format!(
            "# Lutetium project manifest\nname = \"{name}\"\nentry = \"src/main.lu\"\nkind = \"{}\"\n",
            template.as_str()
        );
        std::fs::write(root.join(MANIFEST), manifest).map_err(|error| {
            format!("cannot write `{}`: {error}", root.join(MANIFEST).display())
        })?;
        let source_path = src.join("main.lu");
        std::fs::write(&source_path, template.source(name))
            .map_err(|error| format!("cannot write `{}`: {error}", source_path.display()))?;
        Ok(root.clone())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&root);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_parent() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("lutetium_project_{nonce}"));
        std::fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn console_and_gui_templates_have_valid_manifest_and_entry() {
        let parent = temp_parent();
        for (name, template) in [("ConsoleApp", Template::Console), ("GuiApp", Template::Gui)] {
            let root = create_project(&parent, name, template).unwrap();
            let contents = std::fs::read_to_string(root.join(MANIFEST)).unwrap();
            let project = parse_manifest(&root, &contents).unwrap();
            assert_eq!(project.name, name);
            assert_eq!(project.kind, template);
            let source_path = project.source_path();
            let source = std::fs::read_to_string(&source_path).unwrap();
            assert!(source.contains("fn main()"));
            lu_compiler::compile_source(&source_path.to_string_lossy(), &source)
                .expect("project template must compile through the VM frontend");
            if template == Template::Gui {
                lu_compiler::compile_ir(&source_path.to_string_lossy(), &source)
                    .expect("GUI template must compile through the native frontend");
                assert!(source.contains("gui_button("));
                assert!(source.contains("gui_close(window)"));
            } else {
                assert!(source.contains("print(\"Hello from ConsoleApp\")"));
            }
        }
        std::fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn manifest_rejects_unknown_duplicate_and_escaping_entries() {
        let root = Path::new("project");
        let base = "name = \"Demo\"\nentry = \"src/main.lu\"\nkind = \"console\"\n";
        assert!(parse_manifest(root, base).is_ok());
        assert!(parse_manifest(root, &format!("{base}extra = \"no\"\n")).is_err());
        assert!(parse_manifest(root, &format!("{base}name = \"Other\"\n")).is_err());
        assert!(parse_manifest(
            root,
            "name = \"Demo\"\nentry = \"../outside.lu\"\nkind = \"console\"\n"
        )
        .is_err());
        assert!(parse_manifest(
            root,
            "name = \"../bad\"\nentry = \"src/main.lu\"\nkind = \"console\"\n"
        )
        .is_err());
    }

    #[test]
    fn project_creation_never_overwrites_an_existing_directory() {
        let parent = temp_parent();
        let root = create_project(&parent, "Demo", Template::Console).unwrap();
        assert!(create_project(&parent, "Demo", Template::Gui).is_err());
        assert!(std::fs::read_to_string(root.join(MANIFEST))
            .unwrap()
            .contains("kind = \"console\""));
        std::fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn project_names_reject_non_ascii_and_control_characters() {
        for name in ["é", "\u{007f}", "line\nfeed", "quote\"", "back\\slash"] {
            assert!(
                validate_name(name).is_err(),
                "name should be rejected: {name:?}"
            );
        }
        assert!(validate_name("Demo-App_1").is_ok());
    }
}
