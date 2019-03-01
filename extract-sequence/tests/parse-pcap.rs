use extract_sequence::{
    build_sequence, extract_tls_records, filter_tls_records, pcap_to_sequence, TlsRecord,
};
use pretty_assertions::assert_eq;
use sequences::{
    Sequence,
    SequenceElement::{Gap, Size},
};

#[test]
fn test_parse_and_filter_basic() {
    let file = "./tests/data/CF-constant-rate-400ms-2packets.pcap".to_string();

    // Test basic parsing of pcap
    let mut expected_records: Vec<TlsRecord> = ron::de::from_str(&PCAP_BASIC_ALL).unwrap();
    let mut records = extract_tls_records(&*file).unwrap();
    assert_eq!(
        expected_records.len(),
        records.len(),
        "Number of records must be equal"
    );
    assert_eq!(expected_records, records);

    // Test the filtering procedure
    expected_records = PCAP_BASIC_FILTER_IDS
        .iter()
        .map(|&i| expected_records[i])
        .collect();
    records = filter_tls_records(records);
    assert_eq!(
        expected_records.len(),
        records.len(),
        "Number of records must be equal"
    );
    assert_eq!(expected_records, records);

    // Test building a sequence
    let expected_sequence = Sequence::new(
        vec![Size(1), Gap(8), Size(1), Gap(8), Size(1)],
        file.clone(),
    );
    assert_eq!(
        expected_sequence,
        build_sequence(records, file.clone()).unwrap()
    );

    // End to end test
    assert_eq!(expected_sequence, pcap_to_sequence(file).unwrap());
}

#[test]
fn test_parse_and_filter_with_split_tls_record() {
    let file = "./tests/data/CF-constant-rate-400ms-2packets-with-fragment.pcap".to_string();

    // Test basic parsing of pcap
    let mut expected_records: Vec<TlsRecord> = ron::de::from_str(&PCAP_SPLIT_ALL).unwrap();
    let mut records = extract_tls_records(&*file).unwrap();
    assert_eq!(
        expected_records.len(),
        records.len(),
        "Number of records must be equal"
    );
    assert_eq!(expected_records, records);

    // Test the filtering procedure
    expected_records = PCAP_SPLIT_FILTER_IDS
        .iter()
        .map(|&i| expected_records[i])
        .collect();
    records = filter_tls_records(records);
    assert_eq!(
        expected_records.len(),
        records.len(),
        "Number of records must be equal"
    );
    assert_eq!(expected_records, records);

    // Test building a sequence
    let expected_sequence = Sequence::new(
        vec![Size(1), Gap(8), Size(1), Gap(8), Size(1)],
        file.clone(),
    );
    assert_eq!(
        expected_sequence,
        build_sequence(records, file.clone()).unwrap()
    );

    // End to end test
    assert_eq!(expected_sequence, pcap_to_sequence(file).unwrap());
}

/// All TLS records contained in `CF-constant-rate-400ms-2packets.pcap`
///
/// RON representation of a [`Vec<TlsRecord>`]
const PCAP_BASIC_ALL: &str = r#"[
(
    packet_in_pcap: 4,
    sender: "134.96.225.146",
    port: 59920,
    time: "2019-02-28T11:05:35.190368",
    message_type: Handshake,
    message_length: 206,
),// [0]
(
    packet_in_pcap: 6,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T11:05:35.196718",
    message_type: Handshake,
    message_length: 90,
),// [1]
(
    packet_in_pcap: 6,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T11:05:35.196718",
    message_type: ChangeCipherSpec,
    message_length: 1,
),// [2]
(
    packet_in_pcap: 8,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T11:05:35.197205",
    message_type: ApplicationData,
    message_length: 23,
),// [3]
(
    packet_in_pcap: 8,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T11:05:35.197205",
    message_type: ApplicationData,
    message_length: 2461,
),// [4]
(
    packet_in_pcap: 8,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T11:05:35.197205",
    message_type: ApplicationData,
    message_length: 95,
),// [5]
(
    packet_in_pcap: 8,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T11:05:35.197205",
    message_type: ApplicationData,
    message_length: 53,
),// [6]
(
    packet_in_pcap: 10,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T11:05:35.197342",
    message_type: ApplicationData,
    message_length: 220,
),// [7]
(
    packet_in_pcap: 12,
    sender: "134.96.225.146",
    port: 59920,
    time: "2019-02-28T11:05:35.199539",
    message_type: ChangeCipherSpec,
    message_length: 1,
),// [8]
(
    packet_in_pcap: 14,
    sender: "134.96.225.146",
    port: 59920,
    time: "2019-02-28T11:05:35.245369",
    message_type: ApplicationData,
    message_length: 53,
),// [9]
(
    packet_in_pcap: 16,
    sender: "134.96.225.146",
    port: 59920,
    time: "2019-02-28T11:05:35.603667",
    message_type: ApplicationData,
    message_length: 147,
),// [10]
(
    packet_in_pcap: 18,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T11:05:35.610312",
    message_type: ApplicationData,
    message_length: 487,
),// [11]
(
    packet_in_pcap: 20,
    sender: "134.96.225.146",
    port: 59920,
    time: "2019-02-28T11:05:36.004011",
    message_type: ApplicationData,
    message_length: 147,
),// [12]
(
    packet_in_pcap: 21,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T11:05:36.010081",
    message_type: ApplicationData,
    message_length: 487,
),// [13]
(
    packet_in_pcap: 23,
    sender: "134.96.225.146",
    port: 59920,
    time: "2019-02-28T11:05:36.402797",
    message_type: ApplicationData,
    message_length: 147,
),// [14]
(
    packet_in_pcap: 24,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T11:05:36.408922",
    message_type: ApplicationData,
    message_length: 487,
),// [15]
(
    packet_in_pcap: 26,
    sender: "134.96.225.146",
    port: 59920,
    time: "2019-02-28T11:05:36.802888",
    message_type: ApplicationData,
    message_length: 19,
),// [16]
]"#;
/// IDs of the records we want to keep after filtering [`PCAP_BASIC_ALL`]
const PCAP_BASIC_FILTER_IDS: [usize; 3] = [11, 13, 15];

/// All TLS records contained in `CF-constant-rate-400ms-2packets-with-split.pcap`
///
/// RON representation of a [`Vec<TlsRecord>`]
const PCAP_SPLIT_ALL: &str = r#"[
(
    packet_in_pcap: 4,
    sender: "134.96.225.146",
    port: 58800,
    time: "2019-02-28T10:48:48.129180",
    message_type: Handshake,
    message_length: 206,
),// [0]
(
    packet_in_pcap: 6,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T10:48:48.135276",
    message_type: Handshake,
    message_length: 90,
),// [1]
(
    packet_in_pcap: 6,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T10:48:48.135276",
    message_type: ChangeCipherSpec,
    message_length: 1,
),// [2]
(
    packet_in_pcap: 8,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T10:48:48.135607",
    message_type: ApplicationData,
    message_length: 23,
),// [3]
(
    packet_in_pcap: 10,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T10:48:48.135634",
    message_type: ApplicationData,
    message_length: 2461,
),// [4]
(
    packet_in_pcap: 10,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T10:48:48.135634",
    message_type: ApplicationData,
    message_length: 95,
),// [5]
(
    packet_in_pcap: 10,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T10:48:48.135634",
    message_type: ApplicationData,
    message_length: 53,
),// [6]
(
    packet_in_pcap: 11,
    sender: "134.96.225.146",
    port: 58800,
    time: "2019-02-28T10:48:48.135645",
    message_type: ChangeCipherSpec,
    message_length: 1,
),// [7]
(
    packet_in_pcap: 12,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T10:48:48.135687",
    message_type: ApplicationData,
    message_length: 220,
),// [8]
(
    packet_in_pcap: 15,
    sender: "134.96.225.146",
    port: 58800,
    time: "2019-02-28T10:48:48.182400",
    message_type: ApplicationData,
    message_length: 53,
),// [9]
(
    packet_in_pcap: 17,
    sender: "134.96.225.146",
    port: 58800,
    time: "2019-02-28T10:48:48.539666",
    message_type: ApplicationData,
    message_length: 147,
),// [10]
(
    packet_in_pcap: 19,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T10:48:48.548103",
    message_type: ApplicationData,
    message_length: 487,
),// [11]
(
    packet_in_pcap: 21,
    sender: "134.96.225.146",
    port: 58800,
    time: "2019-02-28T10:48:48.939153",
    message_type: ApplicationData,
    message_length: 147,
),// [12]
(
    packet_in_pcap: 22,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T10:48:48.945785",
    message_type: ApplicationData,
    message_length: 487,
),// [13]
(
    packet_in_pcap: 24,
    sender: "134.96.225.146",
    port: 58800,
    time: "2019-02-28T10:48:49.339112",
    message_type: ApplicationData,
    message_length: 147,
),// [14]
(
    packet_in_pcap: 25,
    sender: "1.1.1.1",
    port: 853,
    time: "2019-02-28T10:48:49.345236",
    message_type: ApplicationData,
    message_length: 487,
),// [15]
(
    packet_in_pcap: 27,
    sender: "134.96.225.146",
    port: 58800,
    time: "2019-02-28T10:48:49.740813",
    message_type: ApplicationData,
    message_length: 19,
),// [16]
]"#;
/// IDs of the records we want to keep after filtering [`PCAP_BASIC_ALL`]
const PCAP_SPLIT_FILTER_IDS: [usize; 3] = [11, 13, 15];
