use pretty_assertions::assert_eq;
use sequences::{
    LoadDnstapConfig, Sequence,
    SequenceElement::{Gap, Size},
};

/// Simple file with four messages and one "missing" [`Gap`]
const DNSTAP1: &str = "./tests/data/porno666.tv-6-0.dnstap.xz";
/// File containing two missing [`Gap`]s and one [`Size`](2) entry
const DNSTAP2: &str = "./tests/data/zuanke8.com-5-0.dnstap.xz";

#[test]
fn test_load_sequence_normal() {
    let seq = Sequence::from_path_with_config(DNSTAP1.as_ref(), LoadDnstapConfig::Normal).unwrap();
    let expected = Sequence::new(
        vec![Size(1), Gap(11), Size(1), Size(1), Gap(10), Size(1)],
        DNSTAP1.to_string(),
    );
    assert_eq!(expected, seq);

    let seq = Sequence::from_path_with_config(DNSTAP2.as_ref(), LoadDnstapConfig::Normal).unwrap();
    let expected = Sequence::new(
        vec![Size(1), Gap(9), Size(1), Size(2), Size(1), Gap(6), Size(1)],
        DNSTAP2.to_string(),
    );
    assert_eq!(expected, seq);
}

#[test]
fn test_load_sequence_perfect_padding() {
    let seq = Sequence::from_path_with_config(DNSTAP1.as_ref(), LoadDnstapConfig::PerfectPadding)
        .unwrap();
    let expected = Sequence::new(vec![Gap(11), Gap(0), Gap(10)], DNSTAP1.to_string());
    assert_eq!(expected, seq);

    let seq = Sequence::from_path_with_config(DNSTAP2.as_ref(), LoadDnstapConfig::PerfectPadding)
        .unwrap();
    let expected = Sequence::new(vec![Gap(9), Gap(0), Gap(0), Gap(6)], DNSTAP2.to_string());
    assert_eq!(expected, seq);
}

#[test]
fn test_load_sequence_perfect_timing() {
    let seq =
        Sequence::from_path_with_config(DNSTAP1.as_ref(), LoadDnstapConfig::PerfectTiming).unwrap();
    let expected = Sequence::new(
        vec![Size(1), Size(1), Size(1), Size(1)],
        DNSTAP1.to_string(),
    );
    assert_eq!(expected, seq);

    let seq =
        Sequence::from_path_with_config(DNSTAP2.as_ref(), LoadDnstapConfig::PerfectTiming).unwrap();
    let expected = Sequence::new(
        vec![Size(1), Size(1), Size(2), Size(1), Size(1)],
        DNSTAP2.to_string(),
    );
    assert_eq!(expected, seq);
}
