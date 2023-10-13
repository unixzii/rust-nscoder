use nscoder::{Archive, Decoder, Encoder, TypeRegistry};

#[derive(Debug)]
struct Person {
    age: u32,
    first_name: String,
    last_name: String,
}

impl Archive for Person {
    type Super = nscoder::RootObject;

    fn class_name() -> &'static str {
        "RCDPerson"
    }

    fn encode(&self, archiver: &mut dyn Encoder) {
        archiver.encode_i32(self.age as _, "Age");
        archiver.encode_string(&self.first_name as _, "FirstName");
        archiver.encode_string(&self.last_name, "LastName");
    }

    fn decode(unarchiver: &dyn Decoder) -> Option<Self> {
        let age = unarchiver.decode_i32("Age") as u32;
        let first_name = unarchiver.decode_string("FirstName")?;
        let last_name = unarchiver.decode_string("LastName")?;
        Some(Person {
            age,
            first_name,
            last_name,
        })
    }
}

#[test]
fn test_archive() {
    let person = Person {
        age: 26,
        first_name: "Cyan".to_owned(),
        last_name: "Yang".to_owned(),
    };

    let encoded_bytes = nscoder::to_bytes(&person).expect("should encode successfully");

    let mut registry = TypeRegistry::new();
    registry.register_type::<Person>();

    let object =
        nscoder::from_bytes(&encoded_bytes, &registry).expect("should decode successfully");
    let decoded_person: &Person = object
        .downcast_ref()
        .expect("type of the value should be `Person`");

    assert_eq!(decoded_person.age, 26);
    assert_eq!(decoded_person.first_name, "Cyan");
    assert_eq!(decoded_person.last_name, "Yang");
}
