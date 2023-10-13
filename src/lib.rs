//! A Rust implementation of Cocoa archiving capability.
#![deny(warnings)]
#![deny(missing_docs)]

mod archiver;
mod object;
mod types;

pub use self::{
    archiver::{from_bytes, from_file, from_plist_value, to_bytes, Decoder, Encoder},
    object::{AnyObject, Archive, RootObject, TypeRegistry},
    types::Error,
};

// Optionally exporting `plist` crate.
#[cfg(feature = "export_plist")]
pub use plist;
