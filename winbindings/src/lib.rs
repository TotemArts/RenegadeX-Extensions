//! This crate serves only to expose Windows bindings and allow Cargo
//! to cache the build results.
//!
//! If you want to add/remove bindings, edit build.rs.
mod bindings;

pub use bindings::*;