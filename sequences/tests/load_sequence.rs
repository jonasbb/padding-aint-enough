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
