//! `lutetium` CLI library: command implementations live here so they can be
//! integration-tested; `main.rs` is a thin wrapper.

mod explain;

use explain::explain_code;
use lu_compiler::compile_source;
use lu_diagnostics::{diagnostics_json, SourceFile};
use lu_vm::bytecode::{deserialize, serialize};
use lu_vm::verify;
use std::fs;
use std::io::Write;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn usage() -> i32 {
    println!(
        "Lutetium {v}\n\n\
         usage:\n\
         \x20 lutetium run <file.lu>                      compile and execute\n\
         \x20 lutetium build <file.lu> [--target t] [--opt 0-3] [--release]   write build/<name>.lbc or build/<name>.exe (target: bytecode|native|gui)\n\
         \x20 lutetium check <file.lu> [--diagnostics json]      type-check without running\n\
         \x20 lutetium inspect <file.lbc> | <file.lu> --ir [--optimized]   disassemble bytecode or typed IR\n\
         \x20 lutetium explain <CODE>                     explain an error code\n\
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
        other => {
            eprintln!("unknown command `{}`", other);
            eprintln!("run `lutetium help` for usage");
            2
        }
    }
}

fn take_file<'a>(args: &'a [String]) -> Result<&'a str, i32> {
    match args.first() {
        Some(f) => Ok(f.as_str()),
        None => {
            eprintln!("error: expected a source file argument");
            Err(2)
        }
    }
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
    let filtered: Vec<String> = args
        .iter()
        .filter(|a| match a.as_str() {
            "--gc-stats" => {
                gc_stats = true;
                false
            }
            "--gc-stress" => {
                gc_stress = true;
                false
            }
            _ => true,
        })
        .cloned()
        .collect();
    let path = match take_file(&filtered) {
        Ok(p) => p,
        Err(c) => return c,
    };
    let module = match compile_file(path) {
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

fn build_native(path: &str, opt: u8, release: bool, gui: bool) -> i32 {
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
    let stem = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("app");
    let out_dir = std::path::Path::new("build");
    if let Err(e) = fs::create_dir_all(out_dir) {
        eprintln!("error: cannot create build directory: {}", e);
        return 2;
    }
    let out_exe = out_dir.join(format!("{}.exe", stem));
    let started = std::time::Instant::now();
    let build = if gui {
        lu_native::build_gui_exe(&ir, &out_exe)
    } else {
        lu_native::build_exe(&ir, &out_exe)
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
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let path = match positional.first() {
        Some(p) => p.as_str(),
        None => {
            eprintln!("error: expected a source file argument");
            return 2;
        }
    };
    let mut target = "bytecode";
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
            "--target=bytecode" => target = "bytecode",
            "--target=native" => target = "native",
            "--target=gui" => target = "gui",
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
    if target == "native" || target == "gui" {
        return build_native(path, opt, release, target == "gui");
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
    let stem = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("app");
    let out_dir = std::path::Path::new("build");
    if let Err(e) = fs::create_dir_all(out_dir) {
        eprintln!("error: cannot create build directory: {}", e);
        return 2;
    }
    let out_path = out_dir.join(format!("{}.lbc", stem));
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
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let path = match positional.first() {
        Some(p) => p.as_str(),
        None => {
            eprintln!("error: expected a source file argument");
            return 2;
        }
    };
    let json = args
        .iter()
        .any(|a| a == "--diagnostics" && args.iter().any(|b| b == "json"))
        || args.iter().any(|a| a == "--diagnostics=json");
    if args.iter().any(|a| a == "--diagnostics") && !json {
        eprintln!("error: --diagnostics needs a value (json)");
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
