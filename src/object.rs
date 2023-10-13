use std::any::Any;
use std::collections::{hash_map::Entry as HashMapEntry, HashMap};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::ops::Deref;

use crate::archiver::{Decoder, Encoder};

mod __private {
    pub struct DummyMarker;
}

/// A type that can be encoded and decoded for archiving.
///
/// The trait provides some metadata of the implementation type. And the
/// type must also implement [`Debug`] trait.
///
/// ## Implementing `Archive`
///
/// `Archive` requires three methods `class_name`, `encode` and `decode` to
/// be implemented, plus an associated type `Super` to be specified.
///
/// `Super` is the type of the object's Cocoa superclass, and it must also
/// implement `Archive` trait. When implementing `Archive` for a type whose
/// Cocoa superclass is `NSObject`, specify [`RootObject`] as `Super` type.
///
/// The `encode` method will be called with an [`Encoder`] parameter, and
/// you can use it to encode arbitrary values. Correspondingly, the `decode`
/// method will be called with an [`Decoder`] parameter, and you need to
/// decode the previously encoded values with it and constructs an object of
/// this type.
///
/// An example looks like this:
///
/// ```rust
/// use nscoder::{Archive, Decoder, Encoder, RootObject};
///
/// #[derive(Debug)]
/// struct Person {
///     age: u32,
///     first_name: String,
///     last_name: String,
/// }
///
/// impl Archive for Person {
///     type Super = RootObject;
///
///     fn class_name() -> &'static str {
///         "RCDPerson"
///     }
///
///     fn encode(&self, archiver: &mut dyn Encoder) {
///         archiver.encode_i32(self.age as _, "Age");
///         archiver.encode_string(&self.first_name as _, "FirstName");
///         archiver.encode_string(&self.last_name, "LastName");
///     }
///
///     fn decode(unarchiver: &dyn Decoder) -> Option<Self> {
///         let age = unarchiver.decode_i32("Age") as u32;
///         let first_name = unarchiver.decode_string("FirstName")?;
///         let last_name = unarchiver.decode_string("LastName")?;
///         Some(Person {
///             age,
///             first_name,
///             last_name,
///         })
///     }
/// }
/// ```
///
/// When implementing `Archive`, you should be aware of the type's class
/// hierarchy, and always remember to encode and decode the data for its
/// superclass. You can do it in your own ways since Rust per se does not
/// have struct inheritance of any kind.
pub trait Archive: Debug + Sized {
    /// The super class of the type in its Cocoa class hierarchy.
    type Super: Archive;

    /// Returns a static string that represents the Cocoa class name of
    /// the type.
    fn class_name() -> &'static str;

    #[doc(hidden)]
    fn is_root_class(_marker: __private::DummyMarker) -> bool {
        false
    }

    /// Encodes this object with the given archiver.
    ///
    /// See the [Implementing `Archive`][impl-archive] section of the
    /// struct-level documentation for more information about how to implement
    /// this method.
    ///
    /// [impl-archive]: #implementing-archive
    fn encode(&self, archiver: &mut dyn Encoder);

    /// Returns an object initialized from data in a given unarchiver.
    ///
    /// See the [Implementing `Archive`][impl-archive] section of the
    /// struct-level documentation for more information about how to implement
    /// this method.
    ///
    /// [impl-archive]: #implementing-archive
    fn decode(unarchiver: &dyn Decoder) -> Option<Self>;
}

/// A type that represents the root class (`NSObject`) in Cocoa class hierarchy.
///
/// This is usually used as the `Super` type while you are implementing [`Archive`]
/// trait for a type whose Cocoa superclass is `NSObject`.
#[derive(Debug)]
pub struct RootObject;

impl Archive for RootObject {
    type Super = Self;

    fn class_name() -> &'static str {
        "NSObject"
    }

    fn is_root_class(_marker: __private::DummyMarker) -> bool {
        true
    }

    fn encode(&self, _archiver: &mut dyn Encoder) {}

    fn decode(_unarchiver: &dyn Decoder) -> Option<Self> {
        Some(Self)
    }
}

pub(crate) fn get_classes<T: Archive>() -> Vec<&'static str> {
    let mut classes = if !T::is_root_class(__private::DummyMarker) {
        get_classes::<T::Super>()
    } else {
        vec![]
    };
    classes.insert(0, T::class_name());
    classes
}

/// A type-erased container that holds an object that implements [`Archive`].
///
/// `AnyObject` automatically dereferences to `dyn Any` (via the [`Deref`] trait),
/// so you can call methods of [`Any`] trait on an `AnyObject` value to perform
/// operations like downcasting.
pub struct AnyObject {
    class_name: &'static str,
    ptr: Box<dyn Any>,
    debug_fn: fn(*const (), &mut Formatter) -> FmtResult,
    encode_fn: fn(*const (), &mut dyn Encoder),
    get_classes_fn: fn() -> Vec<&'static str>,
}

impl AnyObject {
    /// Constructs an `AnyObject` by erasing a given object.
    pub fn erasing<T: Archive + 'static>(object: T) -> Self {
        fn typed_debug<T: Debug>(ptr: *const (), f: &mut Formatter) -> FmtResult {
            let object = unsafe { &*(ptr as *const T) };
            Debug::fmt(object, f)
        }

        fn typed_encode<T: Archive>(ptr: *const (), archiver: &mut dyn Encoder) {
            let object = unsafe { &*(ptr as *const T) };
            object.encode(archiver);
        }

        Self {
            class_name: T::class_name(),
            ptr: Box::new(object),
            debug_fn: typed_debug::<T>,
            encode_fn: typed_encode::<T>,
            get_classes_fn: get_classes::<T>,
        }
    }

    /// Returns a static string that represents the Cocoa class name of
    /// the type.
    pub fn class_name(&self) -> &'static str {
        self.class_name
    }

    /// Attempt to downcast the object to a concrete type.
    pub fn downcast<T: Any>(self) -> Result<Box<T>, Box<dyn Any + 'static>> {
        self.ptr.downcast()
    }

    /// Encodes this object with the given archiver.
    pub fn encode(&self, archiver: &mut dyn Encoder) {
        (self.encode_fn)(&*self.ptr as *const _ as *const (), archiver);
    }

    pub(crate) fn get_classes(&self) -> Vec<&'static str> {
        (self.get_classes_fn)()
    }
}

impl Debug for AnyObject {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        (self.debug_fn)(&*self.ptr as *const _ as *const (), f)
    }
}

impl Deref for AnyObject {
    type Target = dyn Any;

    fn deref(&self) -> &Self::Target {
        &*self.ptr
    }
}

pub(crate) type UnarchiveFn = fn(&dyn Decoder) -> Option<AnyObject>;

/// Registers custom types so that they can be instantiated by the
/// unarchiver later.
#[derive(Default)]
pub struct TypeRegistry {
    unarchive_fns: HashMap<&'static str, UnarchiveFn>,
}

impl TypeRegistry {
    /// Constructs a new `TypeRegistry`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a given type.
    ///
    /// If the type has been already registered, then the method will
    /// return without doing anything.
    pub fn register_type<T: Archive + 'static>(&mut self) {
        if !T::is_root_class(__private::DummyMarker) {
            self.register_type::<T::Super>();
        }

        fn typed_unarchive<T: Archive + 'static>(unarchiver: &dyn Decoder) -> Option<AnyObject> {
            let Some(object) = T::decode(unarchiver) else {
                return None;
            };

            Some(AnyObject::erasing(object))
        }

        let class_name = T::class_name();
        match self.unarchive_fns.entry(class_name) {
            HashMapEntry::Occupied(_) => (),
            HashMapEntry::Vacant(vacant_entry) => {
                vacant_entry.insert(typed_unarchive::<T>);
            }
        }
    }

    pub(crate) fn get_unarchive_fn(&self, class_name: &str) -> Option<&UnarchiveFn> {
        self.unarchive_fns.get(class_name)
    }
}
