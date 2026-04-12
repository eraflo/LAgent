// SPDX-License-Identifier: Apache-2.0
//! `lagent` command-line toolchain: build, run, and check `.la` source files.

// Phase 1 — API documentation will be added progressively.
#![allow(missing_docs)]

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lagent", about = "L-Agent language toolchain", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile a .la source file to .lbc bytecode
    Build {
        #[arg(help = "Source file (.la)")]
        input: PathBuf,
        #[arg(short, long, help = "Output file (.lbc)")]
        output: Option<PathBuf>,
    },
    /// Compile and immediately execute a .la source file
    Run {
        #[arg(help = "Source file (.la)")]
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
    },
    /// Check a .la source file for errors without compiling
    Check {
        #[arg(help = "Source file (.la)")]
        input: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Build { input, output } => {
            let bytecode = lagent_compiler::compile_file(&input)?;
            let out = output.unwrap_or_else(|| input.with_extension("lbc"));
            std::fs::write(&out, &bytecode)?;
            println!("Compiled {} -> {}", input.display(), out.display());
        }
        Command::Run {
            input,
            context,
            backend,
            deterministic,
        } => {
            let bytecode = lagent_compiler::compile_file(&input)?;
            let backend_impl = build_backend(&backend, deterministic)?;
            let mut vm = lagent_vm::Vm::new(context, backend_impl);
            vm.execute(&bytecode)?;
        }
        Command::Check { input } => {
            lagent_compiler::compile_file(&input)?;
            println!("ok {} -- no errors", input.display());
        }
    }

    Ok(())
}

fn build_backend(
    name: &str,
    deterministic: bool,
) -> Result<Box<dyn lagent_vm::backends::InferenceBackend>> {
    match name {
        "anthropic" => {
            #[cfg(feature = "backend-remote")]
            {
                let key = std::env::var("LAGENT_API_KEY").map_err(|_| {
                    anyhow::anyhow!("LAGENT_API_KEY must be set for --backend anthropic")
                })?;
                Ok(Box::new(lagent_vm::backends::AnthropicBackend::new(
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
        _ => Ok(Box::new(lagent_vm::backends::SimulatedBackend::new("ok"))),
    }
}
