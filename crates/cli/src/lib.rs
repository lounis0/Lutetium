//! `lutetium` CLI library: command implementations live here so they can be
//! integration-tested; `main.rs` is a thin wrapper.

mod explain;
mod project;

use explain::explain_code;
use lu_compiler::compile_source;
use lu_diagnostics::{diagnostics_json, SourceFile};
use lu_vm::bytecode::{deserialize, serialize};
use lu_vm::verify;
use project::{Project, Template};
use std::fs;
use std::io::Write;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn usage() -> i32 {
    println!(
        "Lutetium {v}\n\n\
         usage:\n\
         \x20 lutetium new <name> [--template console|gui]   create a project\n\
         \x20 lutetium run [<file.lu>|<project-dir>]       compile and execute\n\
         \x20 lutetium build [<file.lu>|<project-dir>] [--target t] [--opt 0-3]\n\
         \x20 lutetium check [<file.lu>|<project-dir>] [--diagnostics json]\n\
         \x20 lutetium inspect <file.lbc> | <file.lu> --ir [--optimized]\n\
         \x20 lutetium explain <CODE>                     explain an error code\n\
         \x20 lutetium doctor                             inspect the native toolchain\n\
         \x20 lutetium --version",
        v = VERSION
    );
    0
}

pub fn real_main(args: Vec<String>) -> i32 {
    let cmd = match args.first() {
        Some(c) => c.as_str(),
        None => return usage(),
    };
    let rest = &args[1..];
    match cmd {
        "--version" | "version" => {
            println!("Lutetium {}", VERSION);
            0
        }
        "help" | "--help" | "-h" => usage(),
        "run" => cmd_run(rest),
        "build" => cmd_build(rest),
        "check" => cmd_check(rest),
        "inspect" => cmd_inspect(rest),
        "explain" => cmd_explain(rest),
        "new" => cmd_new(rest),
        "doctor" => cmd_doctor(),
        other => {
            eprintln!("unknown command `{}`", other);
            eprintln!("run `lutetium help` for usage");
            2
        }
    }
}

#[cfg(windows)]
fn normalize_windows_path(path: std::path::PathBuf) -> std::path::PathBuf {
    let text = path.to_string_lossy();
    if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        std::path::PathBuf::from(format!(r"\\{unc}"))
    } else if let Some(path) = text.strip_prefix(r"\\?\") {
        std::path::PathBuf::from(path)
    } else {
        path
    }
}

#[cfg(not(windows))]
fn normalize_windows_path(path: std::path::PathBuf) -> std::path::PathBuf {
    path
}

fn resolve_input(path: Option<&String>) -> Result<(String, Option<Project>), i32> {
    let candidate = match path {
        Some(path) => std::path::PathBuf::from(path),
        None => std::env::current_dir().map_err(|error| {
            eprintln!("error: cannot get current directory: {error}");
            2
        })?,
    };
    if candidate.is_dir() {
        let manifest_path = candidate.join(project::MANIFEST);
        let contents = fs::read_to_string(&manifest_path).map_err(|error| {
            eprintln!(
                "error: project manifest `{}` is unavailable: {error}",
                manifest_path.display()
            );
            2
        })?;
        let project = project::parse_manifest(&candidate, &contents).map_err(|error| {
            eprintln!("error: {error}");
            2
        })?;
        let source = project.source_path();
        if !source.is_file() {
            eprintln!("error: project entry `{}` does not exist", source.display());
            return Err(2);
        }
        let root = candidate.canonicalize().map_err(|error| {
            eprintln!(
                "error: cannot resolve project directory `{}`: {error}",
                candidate.display()
            );
            2
        })?;
        let source = source.canonicalize().map_err(|error| {
            eprintln!(
                "error: cannot resolve project entry `{}`: {error}",
                source.display()
            );
            2
        })?;
        if !source.starts_with(&root) {
            eprintln!("error: project entry resolves outside the project directory");
            return Err(2);
        }
        let project = Project {
            root: normalize_windows_path(root),
            ..project
        };
        let source = normalize_windows_path(source);
        return Ok((source.to_string_lossy().into_owned(), Some(project)));
    }
    if path.is_none() {
        eprintln!("error: expected a source file or project directory");
        return Err(2);
    }
    Ok((candidate.to_string_lossy().into_owned(), None))
}

fn validate_cli_flags(
    args: &[String],
    value_flags: &[&str],
    boolean_flags: &[&str],
    value_domains: &[(&str, &[&str])],
) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let (name, inline_value) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(name, value)| (name, Some(value)));
        if value_flags.contains(&name) {
            if !seen.insert(name) {
                return Err(format!("{name} may only be specified once"));
            }
            let value = match inline_value {
                Some(value) if !value.is_empty() => value,
                Some(_) => return Err(format!("{name} needs a value")),
                None => match args.next() {
                    Some(value) if !value.starts_with("--") && !value.is_empty() => value,
                    _ => return Err(format!("{name} needs a value")),
                },
            };
            if let Some((_, allowed)) = value_domains.iter().find(|(flag, _)| *flag == name) {
                if !allowed.contains(&value) {
                    return Err(format!("{name} has an invalid value `{value}`"));
                }
            }
            continue;
        }
        if boolean_flags.contains(&name) {
            if inline_value.is_some() {
                return Err(format!("{name} does not take a value"));
            }
            if !seen.insert(name) {
                return Err(format!("{name} may only be specified once"));
            }
            continue;
        }
        if arg.starts_with("--") {
            return Err(format!("unknown or invalid flag `{arg}`"));
        }
    }
    Ok(())
}

fn command_input<'a>(
    args: &'a [String],
    value_flags: &[&str],
    boolean_flags: &[&str],
) -> Result<Option<&'a String>, String> {
    let mut args = args.iter();
    let mut input = None;
    while let Some(arg) = args.next() {
        if value_flags.contains(&arg.as_str()) {
            let Some(value) = args.next() else {
                return Err(format!("{arg} needs a value"));
            };
            if value.starts_with("--") || value.is_empty() {
                return Err(format!("{arg} needs a value"));
            }
        } else if let Some((name, value)) = arg.split_once('=') {
            if value_flags.contains(&name) {
                if value.is_empty() {
                    return Err(format!("{name} needs a value"));
                }
                continue;
            }
            if boolean_flags.contains(&name) {
                return Err(format!("{name} does not take a value"));
            }
            if arg.starts_with("--") {
                continue;
            }
            if input.replace(arg).is_some() {
                return Err("expected only one source file or project directory".into());
            }
        } else if boolean_flags.contains(&arg.as_str()) || arg.starts_with("--") {
            continue;
        } else if input.replace(arg).is_some() {
            return Err("expected only one source file or project directory".into());
        }
    }
    Ok(input)
}

fn compile_file(path: &str) -> Result<lu_vm::bytecode::BytecodeModule, i32> {
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{}`: {}", path, e);
            return Err(2);
        }
    };
    match compile_source(path, &src) {
        Ok(m) => Ok(m),
        Err(diags) => {
            let file = SourceFile::new(path, src);
            for d in &diags {
                eprint!("{}", file.render(d));
            }
            eprintln!(
                "{}: {} error(s)",
                path,
                diags.iter().filter(|d| d.code.starts_with('E')).count()
            );
            Err(1)
        }
    }
}

fn cmd_run(args: &[String]) -> i32 {
    let mut gc_stats = false;
    let mut gc_stress = false;
    if let Err(error) = validate_cli_flags(args, &[], &["--gc-stats", "--gc-stress"], &[]) {
        eprintln!("error: {error}");
        return 2;
    }
    let mut filtered = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--gc-stats" => gc_stats = true,
            "--gc-stress" => gc_stress = true,
            _ => filtered.push(arg.clone()),
        }
    }
    let input = match command_input(&filtered, &[], &[]) {
        Ok(Some(input)) => input,
        Ok(None) => {
            eprintln!("error: expected a source file or project directory");
            return 2;
        }
        Err(error) => {
            eprintln!("error: {error}");
            return 2;
        }
    };
    let (path, project) = match resolve_input(Some(input)) {
        Ok(input) => input,
        Err(code) => return code,
    };
    if let Some(project) = project.filter(|project| project.kind == Template::Gui) {
        return run_gui_project(&project);
    }
    let module = match compile_file(&path) {
        Ok(m) => m,
        Err(c) => return c,
    };
    if let Err(e) = verify(&module) {
        eprintln!("{}", e);
        return 1;
    }
    let mut stdout = std::io::stdout().lock();
    let result = lu_vm::run_with_options(
        &module,
        |line| {
            let _ = stdout.write_all(line.as_bytes());
            let _ = stdout.flush();
        },
        lu_vm::VmOptions { gc_stress },
    );
    match result {
        Ok((_, stats)) => {
            if gc_stats {
                println!(
                    "gc: {} allocation(s), {} object(s) freed, {} bytes freed, {} minor, {} major, {} promotion(s)",
                    stats.allocations,
                    stats.objects_freed,
                    stats.freed_bytes,
                    stats.minor_collections,
                    stats.major_collections,
                    stats.promotions
                );
                println!(
                    "gc: live {} object(s), {} bytes",
                    stats.live_slots, stats.live_bytes
                );
            }
            0
        }
        Err(e) => {
            eprintln!("runtime error: {}", e);
            eprintln!("run `lutetium explain {}` for details", e.code);
            1
        }
    }
}

fn build_native(path: &str, out_exe: &std::path::Path, opt: u8, release: bool, gui: bool) -> i32 {
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{}`: {}", path, e);
            return 2;
        }
    };
    let ir = match lu_compiler::compile_ir_with_opts(path, &src, opt) {
        Ok(ir) => ir,
        Err(diags) => {
            let file = SourceFile::new(path, src);
            for d in &diags {
                eprint!("{}", file.render(d));
            }
            eprintln!("{}: {} error(s)", path, diags.len());
            return 1;
        }
    };
    if let Some(parent) = out_exe.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!(
                "error: cannot create output directory `{}`: {error}",
                parent.display()
            );
            return 2;
        }
    }
    let started = std::time::Instant::now();
    let build = if gui {
        lu_native::build_gui_exe(&ir, out_exe)
    } else {
        lu_native::build_exe(&ir, out_exe)
    };
    if let Err(e) = build {
        eprintln!("{}", e);
        return 1;
    }
    println!(
        "wrote {} (native{}, opt {}, release {}, {} ms)",
        out_exe.display(),
        if gui { " GUI" } else { "" },
        opt,
        release,
        started.elapsed().as_millis()
    );
    0
}

fn cmd_build(args: &[String]) -> i32 {
    if let Err(error) = validate_cli_flags(
        args,
        &["--target", "--opt"],
        &["--release"],
        &[("--target", &["bytecode", "native", "gui"])],
    ) {
        eprintln!("error: {error}");
        return 2;
    }
    let input = match command_input(args, &["--target", "--opt"], &["--release"]) {
        Ok(Some(input)) => input,
        Ok(None) => {
            eprintln!("error: expected a source file or project directory");
            return 2;
        }
        Err(error) => {
            eprintln!("error: {error}");
            return 2;
        }
    };
    let resolved = match resolve_input(Some(input)) {
        Ok(input) => input,
        Err(code) => return code,
    };
    let path = resolved.0.as_str();
    let project = resolved.1;
    let mut target = match project.as_ref().map(|project| project.kind) {
        Some(Template::Console) => "native",
        Some(Template::Gui) => "gui",
        None => "bytecode",
    };
    let mut release = false;
    let mut opt: u8 = 2;
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--target" => match it.next() {
                Some(t) => target = t.as_str(),
                None => {
                    eprintln!("error: --target needs a value (bytecode|native|gui)");
                    return 2;
                }
            },
            other if other.starts_with("--target=") => target = &other[9..],
            "--release" => release = true,
            "--opt" => match it.next().and_then(|v| v.parse::<u8>().ok()) {
                Some(v) if v <= 3 => opt = v,
                _ => {
                    eprintln!("error: --opt needs a value 0..=3");
                    return 2;
                }
            },
            other if other.starts_with("--opt=") => match other[6..].parse::<u8>() {
                Ok(v) if v <= 3 => opt = v,
                _ => {
                    eprintln!("error: --opt needs a value 0..=3");
                    return 2;
                }
            },
            other if other.starts_with("--") => {
                eprintln!("error: unknown flag `{}`", other);
                return 2;
            }
            _ => {}
        }
    }
    if target != "bytecode" && target != "native" && target != "gui" {
        eprintln!(
            "error: unknown build target `{}` (expected bytecode, native, or gui)",
            target
        );
        return 2;
    }
    if target == "gui"
        && project
            .as_ref()
            .is_some_and(|project| project.kind != Template::Gui)
    {
        eprintln!("error: GUI target requires a project with `kind = \"gui\"`");
        return 2;
    }
    if target == "native" || target == "gui" {
        let output = if let Some(project) = &project {
            project.output_path("exe")
        } else {
            let stem = std::path::Path::new(path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("app");
            std::path::PathBuf::from("build").join(format!("{stem}.exe"))
        };
        let status = build_native(path, &output, opt, release, target == "gui");
        if project.is_some() {
            let _ = fs::remove_file(output.with_extension("obj"));
            let _ = fs::remove_file(output.with_extension("runtime.a"));
        }
        return status;
    }
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{}`: {}", path, e);
            return 2;
        }
    };
    let ir = match lu_compiler::compile_ir_with_opts(path, &src, opt) {
        Ok(ir) => ir,
        Err(diags) => {
            let file = SourceFile::new(path, src);
            for d in &diags {
                eprint!("{}", file.render(d));
            }
            eprintln!("{}: {} error(s)", path, diags.len());
            return 1;
        }
    };
    let module = lu_compiler::codegen::codegen(&ir);
    if let Err(e) = verify(&module) {
        eprintln!("{}", e);
        return 1;
    }
    let out_path = if let Some(project) = &project {
        project.output_path("lbc")
    } else {
        let stem = std::path::Path::new(path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("app");
        std::path::Path::new("build").join(format!("{stem}.lbc"))
    };
    if let Some(parent) = out_path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!(
                "error: cannot create output directory `{}`: {error}",
                parent.display()
            );
            return 2;
        }
    }
    if let Err(e) = fs::write(&out_path, serialize(&module)) {
        eprintln!("error: cannot write `{}`: {}", out_path.display(), e);
        return 2;
    }
    if release {
        println!("release profile: optimization level {} applied", opt);
    }
    println!("wrote {} (opt level {})", out_path.display(), opt);
    0
}

fn cmd_check(args: &[String]) -> i32 {
    if let Err(error) = validate_cli_flags(
        args,
        &["--diagnostics"],
        &[],
        &[("--diagnostics", &["json"])],
    ) {
        eprintln!("error: {error}");
        return 2;
    }
    let input = match command_input(args, &["--diagnostics"], &[]) {
        Ok(Some(input)) => input,
        Ok(None) => {
            eprintln!("error: expected a source file or project directory");
            return 2;
        }
        Err(error) => {
            eprintln!("error: {error}");
            return 2;
        }
    };
    let resolved = match resolve_input(Some(input)) {
        Ok(input) => input,
        Err(code) => return code,
    };
    let path = resolved.0.as_str();
    let json = args.iter().any(|arg| arg == "--diagnostics=json")
        || args
            .windows(2)
            .any(|pair| pair[0] == "--diagnostics" && pair[1] == "json");
    if args
        .iter()
        .any(|arg| arg == "--diagnostics" || arg.starts_with("--diagnostics="))
        && !json
    {
        eprintln!("error: --diagnostics supports only `json`");
        return 2;
    }
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{}`: {}", path, e);
            return 2;
        }
    };
    let file = SourceFile::new(path, src);
    match compile_source(path, &file.text) {
        Ok(_) => {
            if json {
                println!("{}", diagnostics_json(&file, &[]));
            } else {
                println!("ok: no problems found in {}", path);
            }
            0
        }
        Err(diags) => {
            if json {
                println!("{}", diagnostics_json(&file, &diags));
            } else {
                for d in &diags {
                    print!("{}", file.render(d));
                }
                println!(
                    "{}: {} error(s)",
                    path,
                    diags.iter().filter(|d| d.code.starts_with('E')).count()
                );
            }
            1
        }
    }
}

fn cmd_inspect(args: &[String]) -> i32 {
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let path = match positional.first() {
        Some(p) => p.as_str(),
        None => {
            eprintln!("error: expected a file argument");
            return 2;
        }
    };
    let ir_mode = args.iter().any(|a| a == "--ir");
    let optimized = args.iter().any(|a| a == "--optimized");
    if ir_mode {
        // typed IR inspection of a source file
        let src = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read `{}`: {}", path, e);
                return 2;
            }
        };
        let opt: u8 = if optimized { 2 } else { 0 };
        match lu_compiler::compile_ir_with_opts(path, &src, opt) {
            Ok(ir) => {
                print!("{}", lu_compiler::ir::disassemble_ir(&ir));
                0
            }
            Err(diags) => {
                let file = SourceFile::new(path, src);
                for d in &diags {
                    eprint!("{}", file.render(d));
                }
                1
            }
        }
    } else {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("error: cannot read `{}`: {}", path, e);
                return 2;
            }
        };
        match deserialize(&bytes) {
            Ok(m) => {
                if let Err(e) = verify(&m) {
                    eprintln!("{}", e);
                    return 1;
                }
                print!("{}", lu_vm::disassemble(&m));
                0
            }
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        }
    }
}

fn cmd_new(args: &[String]) -> i32 {
    let mut name = None;
    let mut template = Template::Console;
    let mut template_seen = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let value = if arg == "--template" {
            match it.next() {
                Some(value) => Some(value.as_str()),
                None => {
                    eprintln!("error: --template needs a value (console|gui)");
                    return 2;
                }
            }
        } else if let Some(value) = arg.strip_prefix("--template=") {
            Some(value)
        } else if arg.starts_with("--") {
            eprintln!("error: unknown flag `{arg}`");
            return 2;
        } else {
            if name.replace(arg.as_str()).is_some() {
                eprintln!("error: expected one project name");
                return 2;
            }
            None
        };
        if let Some(value) = value {
            if template_seen {
                eprintln!("error: --template may only be specified once");
                return 2;
            }
            template_seen = true;
            template = match value {
                "console" => Template::Console,
                "gui" => Template::Gui,
                _ => {
                    eprintln!("error: unknown template `{value}` (expected console or gui)");
                    return 2;
                }
            };
        }
    }
    let Some(name) = name else {
        eprintln!("error: expected a project name");
        return 2;
    };
    let parent = match std::env::current_dir() {
        Ok(parent) => parent,
        Err(error) => {
            eprintln!("error: cannot get current directory: {error}");
            return 2;
        }
    };
    match project::create_project(&parent, name, template) {
        Ok(path) => {
            println!(
                "created {} project at {}",
                template.as_str(),
                path.display()
            );
            println!("next: lutetium check {}", path.display());
            println!("      lutetium build {}", path.display());
            0
        }
        Err(error) => {
            eprintln!("error: {error}");
            2
        }
    }
}

fn run_gui_project(project: &Project) -> i32 {
    let source = project.source_path();
    let output = project.output_path("exe");
    let status = build_native(&source.to_string_lossy(), &output, 2, false, true);
    let _ = fs::remove_file(output.with_extension("obj"));
    let _ = fs::remove_file(output.with_extension("runtime.a"));
    if status != 0 {
        return status;
    }
    match std::process::Command::new(&output).status() {
        Ok(status) if status.success() => 0,
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!("error: cannot launch `{}`: {error}", output.display());
            1
        }
    }
}

fn cmd_doctor() -> i32 {
    println!("Lutetium {}", VERSION);
    println!("host: {}", std::env::consts::OS);
    println!("native target: {}", lu_native::TARGET_TRIPLE);
    match lu_native::linker::find_toolchain() {
        Ok(toolchain) => {
            println!("native linker: {}", toolchain.gcc.display());
            println!("native libraries: {}", toolchain.libs.display());
            println!("native build: available");
            0
        }
        Err(error) => {
            println!("native build: unavailable ({error})");
            1
        }
    }
}

fn cmd_explain(args: &[String]) -> i32 {
    let code = match args.first() {
        Some(c) => c.as_str(),
        None => {
            eprintln!("error: expected an error code, e.g. E3001");
            return 2;
        }
    };
    match explain_code(code) {
        Some(e) => {
            println!("{} — {}", e.code, e.title);
            println!("\n{}", e.body);
            println!("\nfix: {}", e.fix);
            0
        }
        None => {
            eprintln!("error: unknown error code `{}`", code);
            eprintln!("run `lutetium explain E3001` to see the format, or see ERROR_CODES.json");
            2
        }
    }
}
