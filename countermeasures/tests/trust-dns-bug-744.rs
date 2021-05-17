use std::collections::HashMap;
use trust_dns_proto::{
    rr::rdata::opt::{EdnsCode, EdnsOption, OPT},
    serialize::binary::{BinDecoder, Restrict},
};

#[test]
fn trust_dns_bug_744_empty_option_at_end_of_opt() {
    let bytes: Vec<u8> = vec![
        0x00, 0x0a, 0x00, 0x08, 0x0b, 0x64, 0xb4, 0xdc, 0xd7, 0xb0, 0xcc, 0x8f, 0x00, 0x08, 0x00,
        0x04, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0b, 0x00, 0x00,
    ];

    let mut decoder: BinDecoder = BinDecoder::new(&*bytes);
    let read_rdata =
        trust_dns_proto::rr::rdata::opt::read(&mut decoder, Restrict::new(bytes.len() as u16));
    assert!(
        read_rdata.is_ok(),
        "error decoding: {:?}",
        read_rdata.unwrap_err()
    );

    let opt = read_rdata.unwrap();
    let mut options = HashMap::default();
    options.insert(EdnsCode::Subnet, EdnsOption::Unknown(8, vec![0, 1, 0, 0]));
    options.insert(
        EdnsCode::Cookie,
        EdnsOption::Unknown(10, vec![0x0b, 0x64, 0xb4, 0xdc, 0xd7, 0xb0, 0xcc, 0x8f]),
    );
    options.insert(EdnsCode::Keepalive, EdnsOption::Unknown(11, vec![]));
    let options = OPT::new(options);
    assert_eq!(opt, options);
}
