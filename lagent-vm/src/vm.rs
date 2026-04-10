use crate::runtime::TokenHeap;
use crate::backends::InferenceBackend;
use anyhow::Result;

/// The L-Agent Virtual Machine.
pub struct Vm {
    heap: TokenHeap,
    backend: Box<dyn InferenceBackend>,
}

impl Vm {
    pub fn new(heap_capacity: usize, backend: Box<dyn InferenceBackend>) -> Self {
        Self {
            heap: TokenHeap::new(heap_capacity),
            backend,
        }
    }

    /// Execute raw bytecode bytes.
    pub fn execute(&mut self, _bytecode: &[u8]) -> Result<()> {
        // TODO: deserialize Bytecode and dispatch OpCodes in Phase 1
        println!("[lagent-vm] execution placeholder — Phase 1 in progress");
        Ok(())
    }
}
