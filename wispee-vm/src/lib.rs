// SPDX-License-Identifier: Apache-2.0
//! Wispee virtual machine: executes `.wbc` bytecode and manages inference backends.

// Phase 1 — API documentation and pedantic lint compliance will be added progressively.
#![allow(
    missing_docs,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::cast_precision_loss
)]

pub mod backends;
pub mod persistent_store;
pub mod runtime;
pub mod vm;

pub use backends::InferenceBackend;
pub use persistent_store::PersistentStore;
pub use vm::Vm;
