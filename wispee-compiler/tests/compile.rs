// SPDX-License-Identifier: Apache-2.0
//! Integration tests for the Wispee compiler pipeline.

use wispee_compiler::compile;

/// An empty source file must compile without error.
#[test]
fn compiles_empty_source() {
    assert!(compile("").is_ok());
}

/// The first 4 bytes of any compiled output must be the `WSPW` magic header.
#[test]
fn compile_produces_wspw_magic_header() {
    let bytecode = compile("").expect("compilation failed");
    assert!(
        bytecode.len() >= 4,
        "bytecode too short to contain magic header"
    );
    assert_eq!(&bytecode[0..4], b"WSPW", "missing WSPW magic header");
}

/// Whitespace-only source should behave identically to empty source.
#[test]
fn compiles_whitespace_only_source() {
    assert!(compile("   \n\t  ").is_ok());
}

/// A line comment should not cause a compilation error.
#[test]
fn compiles_source_with_only_comments() {
    assert!(compile("// this is a comment\n// another one").is_ok());
}
