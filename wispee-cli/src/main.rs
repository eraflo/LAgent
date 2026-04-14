// SPDX-License-Identifier: Apache-2.0
//! `wispee` command-line toolchain: build, run, check, and fmt `.wpee` source files.

// Phase 1 — API documentation will be added progressively.
#![allow(missing_docs)]

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "wispee", about = "Wispee language toolchain", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile a .wpee source file to .wbc bytecode (or .walb library bundle with --lib)
    Build {
        #[arg(help = "Source file (.wpee); optional when wispee.toml is present")]
        input: Option<PathBuf>,
        #[arg(short, long, help = "Output file (.wbc or .walb)")]
        output: Option<PathBuf>,
        #[arg(
            long,
            help = "Compile as a library (.walb) instead of an executable (.wbc)"
        )]
        lib: bool,
        #[arg(long, help = "Library name (overrides wispee.toml)")]
        name: Option<String>,
    },
    /// Compile and immediately execute a .wpee source file
    Run {
        #[arg(help = "Source file (.wpee)")]
        input: PathBuf,
        #[arg(
            short,
            long,
            default_value = "4096",
            help = "Context heap size in tokens"
        )]
        context: usize,
        #[arg(
            long,
            default_value = "simulated",
            help = "Inference backend: simulated | anthropic"
        )]
        backend: String,
        #[arg(long, help = "Use temperature=0 for deterministic inference")]
        deterministic: bool,
        #[arg(long, help = "Path to a JSON file for cross-run persistent memory")]
        persist: Option<PathBuf>,
    },
    /// Check a .wpee source file for errors without compiling
    Check {
        #[arg(help = "Source file (.wpee)")]
        input: PathBuf,
    },
    /// Auto-format a .wpee source file (writes in-place by default)
    Fmt {
        #[arg(help = "Source file (.wpee)")]
        input: PathBuf,
        #[arg(
            long,
            help = "Print formatted output to stdout and exit non-zero if the file would change"
        )]
        check: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Build {
            input,
            output,
            lib,
            name,
        } => {
            // Resolve the source file: explicit arg > wispee.toml entry > error.
            let (source_path, lib_name) = resolve_build_input(input, name.as_ref(), lib)?;

            if lib {
                let bundle = wispee_compiler::compile_library_file(&source_path, &lib_name)?;
                let out = output.unwrap_or_else(|| source_path.with_extension("walb"));
                std::fs::write(&out, &bundle)?;
                println!("Library {} -> {}", source_path.display(), out.display());
            } else {
                let bytecode = wispee_compiler::compile_file(&source_path)?;
                let out = output.unwrap_or_else(|| source_path.with_extension("wbc"));
                std::fs::write(&out, &bytecode)?;
                println!("Compiled {} -> {}", source_path.display(), out.display());
            }
        }
        Command::Run {
            input,
            context,
            backend,
            deterministic,
            persist,
        } => {
            let bytecode = wispee_compiler::compile_file(&input)?;
            let backend_impl = build_backend(&backend, deterministic)?;
            let vm = wispee_vm::Vm::new(context, backend_impl);

            let mut vm = if let Some(persist_path) = persist {
                let store = wispee_vm::persistent_store::FilePersistentStore::open(&persist_path)?;
                vm.with_persistent_store(Box::new(store))
            } else {
                vm
            };

            vm.execute(&bytecode)?;
        }
        Command::Check { input } => {
            wispee_compiler::compile_file(&input)?;
            println!("ok {} -- no errors", input.display());
        }
        Command::Fmt { input, check } => {
            let source = std::fs::read_to_string(&input)?;
            let formatted = wispee_compiler::format_source(&source)?;

            if check {
                if source == formatted {
                    // Already formatted — exit 0.
                } else {
                    eprintln!("{} would be reformatted", input.display());
                    std::process::exit(1);
                }
            } else {
                std::fs::write(&input, &formatted)?;
                println!("Formatted {}", input.display());
            }
        }
    }

    Ok(())
}

/// Resolve the source file path and library name for `wispee build`.
///
/// Priority order:
/// 1. Explicit `--input` CLI argument.
/// 2. `wispee.toml` in the current directory or any parent (uses `lib.entry` when
///    `--lib` is set, else `project.entry`).
fn resolve_build_input(
    input: Option<PathBuf>,
    name_override: Option<&String>,
    lib_mode: bool,
) -> Result<(PathBuf, String)> {
    if let Some(path) = input {
        // Derive a lib name from the override, then the filename stem.
        let lib_name = name_override
            .cloned()
            .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "library".to_string());
        return Ok((path, lib_name));
    }

    // No explicit input — try wispee.toml.
    let cwd = std::env::current_dir()?;
    if let Some((config, root)) = wispee_compiler::project::ProjectConfig::find(&cwd) {
        let (entry, lib_name) = if lib_mode {
            if let Some(lib) = &config.lib {
                (lib.entry.clone(), lib.name.clone())
            } else {
                // Fall back to project entry with project name.
                (config.project.entry.clone(), config.project.name.clone())
            }
        } else {
            (config.project.entry.clone(), config.project.name.clone())
        };

        let path = root.join(&entry);
        let lib_name = name_override.map_or(lib_name, String::clone);
        return Ok((path, lib_name));
    }

    anyhow::bail!("no input file specified and no wispee.toml found")
}

fn build_backend(
    name: &str,
    deterministic: bool,
) -> Result<Box<dyn wispee_vm::backends::InferenceBackend>> {
    match name {
        "anthropic" => {
            #[cfg(feature = "backend-remote")]
            {
                let key = std::env::var("WISPEE_API_KEY").map_err(|_| {
                    anyhow::anyhow!("WISPEE_API_KEY must be set for --backend anthropic")
                })?;
                Ok(Box::new(wispee_vm::backends::AnthropicBackend::new(
                    key,
                    deterministic,
                )))
            }
            #[cfg(not(feature = "backend-remote"))]
            {
                let _ = deterministic;
                anyhow::bail!(
                    "recompile with --features backend-remote to use the Anthropic backend"
                )
            }
        }
        _ => Ok(Box::new(wispee_vm::backends::SimulatedBackend::new("ok"))),
    }
}
