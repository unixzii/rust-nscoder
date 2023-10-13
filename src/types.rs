use std::collections::HashMap;

use plist::{Uid as PlistUid, Value as PlistValue};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Deserialize, Serialize, Debug)]
pub struct ArchiveDict {
    #[serde(rename = "$archiver")]
    pub archiver_class_name: String,
    #[serde(rename = "$objects")]
    pub objects: Vec<PlistValue>,
    #[serde(rename = "$top")]
    pub top_objects: HashMap<String, PlistUid>,
    #[serde(rename = "$version")]
    pub version: u32,
}

/// A type represents all possible errors that can occur when using this crate.
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum Error {
    #[error("archive data is malformed")]
    MalformedData(#[from] plist::Error),
    #[error("archiver `{0}` is not supported")]
    UnsupportedArchiver(String),
    #[error("root object is not found")]
    NoRootObject,
    #[error("structure of the decoding object is malformed")]
    MalformedObject,
    #[error("decoding class `{0}` is unknown, did you forget to register?")]
    UnknownClass(String),
}
