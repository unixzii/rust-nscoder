use std::path::Path;

use plist::{Uid as PlistUid, Value as PlistValue};

use crate::object::{get_classes, AnyObject, Archive, TypeRegistry};
use crate::types::{ArchiveDict, Error};

/// A type that can encode data into an object archive.
pub trait Encoder {
    /// Encodes an `i32` value and associates it with a given key.
    fn encode_i32(&mut self, value: i32, key: &str);

    /// Encodes an `i64` value and associates it with a given key.
    fn encode_i64(&mut self, value: i64, key: &str);

    /// Encodes a string value and associates it with a given key.
    fn encode_string(&mut self, value: &str, key: &str);

    /// Encodes an object and associates it with a given key.
    fn encode_object(&mut self, object: &AnyObject, key: &str);
}

/// A type that can decode data from an object archive.
pub trait Decoder {
    /// Decodes and returns an `i32` value associated with a given key.
    ///
    /// Returns `0` if key does not exist.
    fn decode_i32(&self, key: &str) -> i32;

    /// Decodes and returns an `i64` value associated with a given key.
    ///
    /// Returns `0` if key does not exist.
    fn decode_i64(&self, key: &str) -> i64;

    /// Decodes and returns a string associated with a given key.
    ///
    /// Returns `None` if key does not exist, or the value is not a string.
    fn decode_string(&self, key: &str) -> Option<String>;

    /// Decodes and returns an object associated with a given key.
    ///
    /// Returns `None` if key does not exist, or the object failed to decode.
    fn decode_object(&self, key: &str) -> Option<AnyObject>;
}

/// Decodes a previously-archived object graph from a file, and returns its root object.
pub fn from_file<P: AsRef<Path>>(path: P, registry: &TypeRegistry) -> Result<AnyObject, Error> {
    let dict: ArchiveDict = plist::from_file(path)?;
    from_archive_dict(dict, registry)
}

/// Decodes a previously-archived object graph from a byte slice, and returns its root object.
pub fn from_bytes(bytes: &[u8], registry: &TypeRegistry) -> Result<AnyObject, Error> {
    let dict: ArchiveDict = plist::from_bytes(bytes)?;
    from_archive_dict(dict, registry)
}

/// Decodes a previously-archived object graph from a [`plist::Value`], and returns its root object.
///
/// **Note:**
/// This is typically used to reuse a deserialized plist value, and the input should be previously
/// encoded with a keyed archiver. Arbitrarily modifying the value may lead to unexpected results.
pub fn from_plist_value(value: &PlistValue, registry: &TypeRegistry) -> Result<AnyObject, Error> {
    let dict: ArchiveDict = plist::from_value(value)?;
    from_archive_dict(dict, registry)
}

#[inline(always)]
fn from_archive_dict(dict: ArchiveDict, registry: &TypeRegistry) -> Result<AnyObject, Error> {
    let unarchiver = __impl::Unarchiver::new(dict, registry);
    unarchiver.unarchive_root_object()
}

/// Encodes an object graph with the given root object into a data representation, and returns the
/// archive data as bytes.
pub fn to_bytes<O: Archive>(object: &O) -> Result<Vec<u8>, Error> {
    let dict = to_archive_dict(object);
    let mut buf = Vec::new();
    plist::to_writer_binary(&mut buf, &dict)?;
    Ok(buf)
}

#[inline(always)]
fn to_archive_dict<O: Archive>(object: &O) -> ArchiveDict {
    let mut archiver = __impl::Archiver::new();
    let root_object = archiver.encode_new_object(|archiver| {
        object.encode(archiver);
        get_classes::<O>()
    });
    archiver.seal(root_object)
}

mod __impl {
    use std::cell::Cell;
    use std::collections::HashMap;

    use plist::Dictionary as PlistDictionary;

    use super::{Encoder, PlistUid, PlistValue};
    use crate::object::{AnyObject, TypeRegistry};
    use crate::types::{ArchiveDict, Error};

    mod traits {
        pub use crate::archiver::{Decoder, Encoder};
    }

    // Non-keyed archivers are not supported since `NSArchiver` is deprecated
    // for better forward and backward compatibility.
    const KEYED_ARCHIVER_CLASS_NAME: &str = "NSKeyedArchiver";

    #[derive(Default)]
    pub struct Archiver {
        objects: Vec<PlistValue>,
        active_object_index: Option<usize>,
    }

    impl Archiver {
        pub fn new() -> Self {
            let mut this = Self::default();
            this.objects.push(PlistValue::String("$null".to_owned()));
            this
        }

        pub fn encode_new_object<E>(&mut self, encode_f: E) -> PlistUid
        where
            E: FnOnce(&mut dyn Encoder) -> Vec<&'static str>,
        {
            let dict = PlistValue::Dictionary(PlistDictionary::new());
            self.objects.push(dict);
            let new_object_index = self.objects.len() - 1;

            self.with_active_object(new_object_index, |archiver| {
                let classes = encode_f(archiver);

                // Encodes the class info.
                let class = *classes.first().expect("the type should have a class");
                let mut class_info = PlistDictionary::new();
                class_info.insert(
                    "$classes".to_owned(),
                    PlistValue::Array(
                        classes
                            .into_iter()
                            .map(|s| PlistValue::String(s.to_owned()))
                            .collect(),
                    ),
                );
                class_info.insert(
                    "$classname".to_owned(),
                    PlistValue::String(class.to_owned()),
                );
                archiver.objects.push(PlistValue::Dictionary(class_info));
                let class_info_index = archiver.objects.len() - 1;

                let dict = archiver.ensure_active_object();
                dict.insert(
                    "$class".to_owned(),
                    PlistValue::Uid(PlistUid::new(class_info_index as _)),
                );
            });

            PlistUid::new(new_object_index as _)
        }

        pub fn seal(self, root_object: PlistUid) -> ArchiveDict {
            ArchiveDict {
                archiver_class_name: KEYED_ARCHIVER_CLASS_NAME.to_owned(),
                objects: self.objects,
                top_objects: HashMap::from([("root".to_owned(), root_object)]),
                version: 100000,
            }
        }

        fn with_active_object<F: FnOnce(&mut Self)>(&mut self, index: usize, f: F) {
            let last_index = self.active_object_index.take();
            self.active_object_index = Some(index);
            f(self);
            self.active_object_index = last_index;
        }

        fn ensure_active_object(&mut self) -> &mut PlistDictionary {
            let index = self
                .active_object_index
                .expect("expected an active object index");
            if self.objects.len() <= index {
                panic!("internal state of archiver is inconsistent");
            }
            self.objects[index]
                .as_dictionary_mut()
                .expect("expected a dictionary")
        }
    }

    impl traits::Encoder for Archiver {
        fn encode_i32(&mut self, value: i32, key: &str) {
            self.encode_i64(value as i64, key)
        }

        fn encode_i64(&mut self, value: i64, key: &str) {
            let dict = self.ensure_active_object();
            dict.insert(key.to_owned(), PlistValue::Integer(value.into()));
        }

        fn encode_string(&mut self, value: &str, key: &str) {
            self.objects.push(PlistValue::String(value.to_owned()));
            let index = self.objects.len() - 1;

            let dict = self.ensure_active_object();
            dict.insert(key.to_owned(), PlistValue::Uid(PlistUid::new(index as _)));
        }

        fn encode_object(&mut self, object: &AnyObject, key: &str) {
            let object = self.encode_new_object(|archiver| {
                object.encode(archiver);
                object.get_classes()
            });

            let dict = self.ensure_active_object();
            dict.insert(key.to_owned(), PlistValue::Uid(object));
        }
    }

    pub struct Unarchiver<'t> {
        dict: ArchiveDict,
        active_object: Cell<Option<PlistUid>>,
        type_registry: &'t TypeRegistry,
    }

    impl<'t> Unarchiver<'t> {
        pub fn new(dict: ArchiveDict, registry: &'t TypeRegistry) -> Self {
            Self {
                dict,
                active_object: Cell::new(None),
                type_registry: registry,
            }
        }

        pub fn unarchive_root_object(&self) -> Result<AnyObject, Error> {
            // Validate the archiver class before actually unarchiving.
            let archiver_class_name = &self.dict.archiver_class_name;
            if archiver_class_name != KEYED_ARCHIVER_CLASS_NAME {
                return Err(Error::UnsupportedArchiver(archiver_class_name.clone()));
            }

            let Some(root_object) = self.dict.top_objects.get("root") else {
                return Err(Error::NoRootObject);
            };
            if self.dict.objects.len() <= root_object.get() as usize {
                return Err(Error::MalformedObject);
            }

            // Set the root object as active and start decoding.
            self.active_object.set(Some(*root_object));
            self.decode_active_object()
        }

        fn ensure_active_object(&self) -> &PlistValue {
            let uid = self.active_object.get().expect("expected an active object");
            let index = uid.get() as usize;
            if self.dict.objects.len() <= index {
                panic!("internal state of unarchiver is inconsistent");
            }
            &self.dict.objects[index]
        }

        fn decode_active_object(&self) -> Result<AnyObject, Error> {
            let Some(dict) = self.ensure_active_object().as_dictionary() else {
                return Err(Error::MalformedObject);
            };
            let Some(class) = dict.get("$class").and_then(|value| value.as_uid()) else {
                return Err(Error::MalformedObject);
            };

            let class_index = class.get() as usize;
            if self.dict.objects.len() <= class_index {
                return Err(Error::MalformedObject);
            }
            let Some(class_name) = self.dict.objects[class_index]
                .as_dictionary()
                .and_then(|dict| dict.get("$classname"))
                .and_then(|value| value.as_string())
            else {
                return Err(Error::MalformedObject);
            };

            let Some(unarchive_fn) = self.type_registry.get_unarchive_fn(class_name) else {
                return Err(Error::UnknownClass(class_name.to_owned()));
            };

            match unarchive_fn(self) {
                Some(object) => Ok(object),
                None => Err(Error::MalformedObject),
            }
        }
    }

    impl<'t> traits::Decoder for Unarchiver<'t> {
        fn decode_i32(&self, key: &str) -> i32 {
            self.decode_i64(key) as i32
        }

        fn decode_i64(&self, key: &str) -> i64 {
            let Some(dict) = self.ensure_active_object().as_dictionary() else {
                return 0;
            };
            dict.get(key)
                .and_then(|value| value.as_signed_integer())
                .unwrap_or(0)
        }

        fn decode_string(&self, key: &str) -> Option<String> {
            let Some(dict) = self.ensure_active_object().as_dictionary() else {
                return None;
            };
            let Some(object) = dict.get(key).and_then(|value| value.as_uid()) else {
                return None;
            };
            let index = object.get() as usize;
            if self.dict.objects.len() <= index {
                return None;
            }

            self.dict.objects[index].as_string().map(str::to_owned)
        }

        fn decode_object(&self, key: &str) -> Option<AnyObject> {
            let Some(dict) = self.ensure_active_object().as_dictionary() else {
                return None;
            };
            let Some(object) = dict.get(key).and_then(|value| value.as_uid()) else {
                return None;
            };
            if self.dict.objects.len() <= object.get() as usize {
                return None;
            }

            let last_object = self.active_object.replace(Some(*object));
            let decoded_object = self.decode_active_object().ok();
            self.active_object.set(last_object);

            decoded_object
        }
    }
}
