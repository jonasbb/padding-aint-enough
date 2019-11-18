use pretty_assertions::assert_eq;
use sequences::{
    LoadSequenceConfig, Sequence,
    SequenceElement::{Gap, Size},
    SimulatedCountermeasure,
};

/// Simple file with four messages and one "missing" [`Gap`]
const DNSTAP1: &str = "./tests/data/porno666.tv-6-0.dnstap.xz";
/// File containing two missing [`Gap`]s and one [`Size`](2) entry
const DNSTAP2: &str = "./tests/data/zuanke8.com-5-0.dnstap.xz";
/// Simple pcap file to test reading and extracting of pcaps
const PCAP1: &str = "./tests/data/google.com-0-0.pcap";
/// Simple pcap file to test reading and extracting of compressed pcaps
const PCAP_XZ1: &str = "./tests/data/1password.com-0-0.pcap.xz";
/// Simple pcap file to test parsing with duplicated aaa.aaa.aaa.aaa queries
const PCAP_XZ2: &str = "./tests/data/zju.edu.cn-8-0.pcap.xz";
/// Test parsing with reordered data
const PCAP_XZ3: &str = "./tests/data/vk.com-1217-242.pcap.xz";

#[test]
fn test_load_sequence_normal() {
    let config = LoadSequenceConfig {
        simulated_countermeasure: SimulatedCountermeasure::None,
        ..Default::default()
    };

    let seq = Sequence::from_path_with_config(DNSTAP1.as_ref(), config).unwrap();
    let expected = Sequence::new(
        vec![Size(1), Gap(11), Size(1), Size(1), Gap(10), Size(1)],
        DNSTAP1.to_string(),
    );
    assert_eq!(expected, seq);

    let seq = Sequence::from_path_with_config(DNSTAP2.as_ref(), config).unwrap();
    let expected = Sequence::new(
        vec![Size(1), Gap(9), Size(1), Size(2), Size(1), Gap(6), Size(1)],
        DNSTAP2.to_string(),
    );
    assert_eq!(expected, seq);
}

#[test]
fn test_load_sequence_perfect_padding() {
    let config = LoadSequenceConfig {
        simulated_countermeasure: SimulatedCountermeasure::PerfectPadding,
        ..Default::default()
    };

    let seq = Sequence::from_path_with_config(DNSTAP1.as_ref(), config).unwrap();
    let expected = Sequence::new(vec![Gap(11), Gap(0), Gap(10)], DNSTAP1.to_string());
    assert_eq!(expected, seq);

    let seq = Sequence::from_path_with_config(DNSTAP2.as_ref(), config).unwrap();
    let expected = Sequence::new(vec![Gap(9), Gap(0), Gap(0), Gap(6)], DNSTAP2.to_string());
    assert_eq!(expected, seq);
}

#[test]
fn test_load_sequence_perfect_timing() {
    let config = LoadSequenceConfig {
        simulated_countermeasure: SimulatedCountermeasure::PerfectTiming,
        ..Default::default()
    };

    let seq = Sequence::from_path_with_config(DNSTAP1.as_ref(), config).unwrap();
    let expected = Sequence::new(
        vec![Size(1), Size(1), Size(1), Size(1)],
        DNSTAP1.to_string(),
    );
    assert_eq!(expected, seq);

    let seq = Sequence::from_path_with_config(DNSTAP2.as_ref(), config).unwrap();
    let expected = Sequence::new(
        vec![Size(1), Size(1), Size(2), Size(1), Size(1)],
        DNSTAP2.to_string(),
    );
    assert_eq!(expected, seq);
}

/// Ensure that an uncompressed pcap file can be read
#[test]
#[cfg_attr(not(feature = "read_pcap"), ignore)]
fn test_load_pcap() {
    let seq = Sequence::from_path(PCAP1.as_ref()).unwrap();
    let expected = r##"{"./tests/data/google.com-0-0.pcap":["S01","G08","S01","G04","S01","G07","S01","G02","S01","G02","S01","G05","S01","G02","S01","G02","S01","G02","S01"]}"##;
    assert_eq!(
        expected,
        serde_json::to_string(&seq).unwrap(),
        "Failed to load google.com pcap file"
    );
}

/// Ensure that a compressed pcap file can be read
#[test]
#[cfg_attr(not(feature = "read_pcap"), ignore)]
fn test_load_pcap_xz() {
    let seq = Sequence::from_path(PCAP_XZ1.as_ref()).unwrap();
    let expected = r##"{"./tests/data/1password.com-0-0.pcap.xz":["S01","G08","S01","G08","S01","G03","S01"]}"##;
    assert_eq!(
        expected,
        serde_json::to_string(&seq).unwrap(),
        "Failed to load 1password.com pcap file"
    );
}

/// Test that parsing still works, even with repeated aaa queries
///
/// For a small fraction of cases, the large aaa.aaa.aaa.aaa query got duplicated.
/// This caused problems, as the second answer was interpreted as the closing zzz.zzz.zzz.zzz response and thus
#[test]
#[cfg_attr(not(feature = "read_pcap"), ignore)]
fn test_load_pcap_duplicate_aaa_query() {
    let seq = Sequence::from_path(PCAP_XZ2.as_ref()).unwrap();
    let expected =
        r##"{"./tests/data/zju.edu.cn-8-0.pcap.xz":["S01","G02","S01","G09","S01","G02","S01"]}"##;
    assert_eq!(
        expected,
        serde_json::to_string(&seq).unwrap(),
        "Failed to load zju.edu.cn pcap file"
    );
}

/// Test that parsing is resilient to reordered data
#[test]
#[cfg_attr(not(feature = "read_pcap"), ignore)]
fn test_load_pcap_reordered_packets() {
    let seq = Sequence::from_path(PCAP_XZ3.as_ref()).unwrap();
    let expected = r##"{"./tests/data/vk.com-1217-242.pcap.xz":["S01","G08","S01","G09","S01","G02","S01","G05","S01","G02","S01","G06","S01","G07","S01","G02","S01"]}"##;
    assert_eq!(
        expected,
        serde_json::to_string(&seq).unwrap(),
        "Failed to load vk.com pcap file"
    );
}
