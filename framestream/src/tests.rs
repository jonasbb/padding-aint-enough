use EncoderWriter;
use std::io::prelude::*;
use std::io::BufWriter;

#[test]
fn test_basic_encoding() {
    let mut enc = EncoderWriter::new(
        BufWriter::new(Vec::new()),
        Some("test-content-type".to_owned()),
    );
    enc.write(b"test-content").unwrap();
    let enc = enc.finish().unwrap();
    let out = enc.into_inner().unwrap();
    let expected: [u8; 65] = [
        0, 0, 0, 0, 0, 0, 0, 29, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 17, 116, 101, 115, 116, 45, 99,
        111, 110, 116, 101, 110, 116, 45, 116, 121, 112, 101, 0, 0, 0, 12, 116, 101, 115, 116, 45,
        99, 111, 110, 116, 101, 110, 116, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 3,
    ];
    assert_eq!(out, &expected[..]);
}
