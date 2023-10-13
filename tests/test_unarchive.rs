use nscoder::{Archive, Decoder, Encoder, TypeRegistry};

#[derive(Debug)]
struct MBFile {
    group_id: u32,
    inode_number: u64,
    relative_path: String,
}

impl Archive for MBFile {
    type Super = nscoder::RootObject;

    fn class_name() -> &'static str {
        "MBFile"
    }

    fn encode(&self, archiver: &mut dyn Encoder) {
        archiver.encode_i32(self.group_id as _, "GroupID");
        archiver.encode_i64(self.inode_number as _, "InodeNumber");
        archiver.encode_string(&self.relative_path, "RelativePath");
    }

    fn decode(unarchiver: &dyn Decoder) -> Option<Self> {
        let group_id = unarchiver.decode_i32("GroupID") as u32;
        let inode_number: u64 = unarchiver.decode_i64("InodeNumber") as u64;
        let relative_path = unarchiver.decode_string("RelativePath")?;
        Some(MBFile {
            group_id,
            inode_number,
            relative_path,
        })
    }
}

#[test]
fn test_unarchive() {
    let mut registry = TypeRegistry::new();
    registry.register_type::<MBFile>();

    let bytes = include_bytes!("./fixtures/mobilesync_backup.plist");
    let object = nscoder::from_bytes(bytes, &registry).expect("should decode successfully");
    let file: &MBFile = object
        .downcast_ref()
        .expect("type of the value should be `MBFile`");

    assert_eq!(file.group_id, 501);
    assert_eq!(file.inode_number, 228000);
    assert_eq!(file.relative_path, "Library/PersistentStores");
}
