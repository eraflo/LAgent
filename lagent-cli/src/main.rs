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
            let source = std::fs::read_to_string(&input)?;
            let bytecode = lagent_compiler::compile(&source)?;
            let out = output.unwrap_or_else(|| input.with_extension("lbc"));
            std::fs::write(&out, &bytecode)?;
            println!("Compiled {} -> {}", input.display(), out.display());
        }
        Command::Run { input, context } => {
            let source = std::fs::read_to_string(&input)?;
            let bytecode = lagent_compiler::compile(&source)?;
            let backend = Box::new(lagent_vm::backends::SimulatedBackend::new("ok"));
            let mut vm = lagent_vm::Vm::new(context, backend);
            vm.execute(&bytecode)?;
        }
        Command::Check { input } => {
            let source = std::fs::read_to_string(&input)?;
            lagent_compiler::compile(&source)?;
            println!("ok {} -- no errors", input.display());
        }
    }

    Ok(())
}
