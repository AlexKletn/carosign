use std::io::{Cursor, ErrorKind};

use super::{MAX_POLICY_BYTES, read_document};

#[test]
fn reads_two_documents_without_interpreting_their_contents() {
    let mut data = Vec::new();
    for document in ["# policy\n", "uid=1000 subject=42 actions=inventory\n"] {
        data.extend((document.len() as u32).to_be_bytes());
        data.extend(document.as_bytes());
    }
    let mut input = Cursor::new(data);
    assert_eq!(read_document(&mut input).unwrap(), "# policy\n");
    assert_eq!(
        read_document(&mut input).unwrap(),
        "uid=1000 subject=42 actions=inventory\n"
    );
}

#[test]
fn rejects_oversized_truncated_and_non_utf8_documents() {
    assert_eq!(
        read_document(&mut &((MAX_POLICY_BYTES + 1) as u32).to_be_bytes()[..])
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidData
    );
    assert_eq!(
        read_document(&mut &[0, 0, 0, 2, b'a'][..])
            .unwrap_err()
            .kind(),
        ErrorKind::UnexpectedEof
    );
    assert_eq!(
        read_document(&mut &[0, 0, 0, 1, 0xff][..])
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidData
    );
}
