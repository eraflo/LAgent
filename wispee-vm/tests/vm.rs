// SPDX-License-Identifier: Apache-2.0
//! Integration tests for the Wispee virtual machine.

use wispee_vm::{backends::SimulatedBackend, Vm};

/// The VM must successfully execute bytecode produced from an empty program.
#[test]
fn vm_executes_empty_program() {
    let bytecode = wispee_compiler::compile("").expect("compiler failed");
    let backend = Box::new(SimulatedBackend::new("ok"));
    let mut vm = Vm::new(4096, backend);
    assert!(vm.execute(&bytecode).is_ok());
}

/// The VM must not panic when given a minimal context heap.
#[test]
fn vm_accepts_minimal_heap_size() {
    let bytecode = wispee_compiler::compile("").expect("compiler failed");
    let backend = Box::new(SimulatedBackend::new(""));
    let mut vm = Vm::new(1, backend);
    assert!(vm.execute(&bytecode).is_ok());
}
