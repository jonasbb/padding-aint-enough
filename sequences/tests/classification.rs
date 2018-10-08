extern crate sequences;

use sequences::{common_sequence_classifications::*, *};

#[test]
fn test_classify_sequence_r001() {
    use SequenceElement::{Gap, Size};

    let sequence = Sequence::new(vec![Size(1), Size(2)], "".to_string());
    assert_eq!(sequence.classify(), Some(R001));

    let sequence = Sequence::new(vec![Size(1), Gap(3), Size(2)], "".to_string());
    assert_eq!(sequence.classify(), Some(R001));

    let sequence = Sequence::new(vec![Size(1), Gap(10), Size(2)], "".to_string());
    assert_eq!(sequence.classify(), Some(R001));

    let sequence = Sequence::new(vec![Gap(9), Size(1), Size(2), Gap(12)], "".to_string());
    assert_eq!(sequence.classify(), Some(R001));

    let sequence = Sequence::new(
        vec![Gap(9), Size(1), Gap(5), Size(2), Gap(12)],
        "".to_string(),
    );
    assert_eq!(sequence.classify(), Some(R001));
}

#[test]
fn test_classify_sequence_r002() {
    use SequenceElement::{Gap, Size};

    let sequence = Sequence::new(vec![Size(1), Size(2), Size(1)], "".to_string());
    assert_eq!(sequence.classify(), Some(R002));

    let sequence = Sequence::new(vec![Size(1), Gap(3), Size(2), Size(1)], "".to_string());
    assert_eq!(sequence.classify(), Some(R002));

    let sequence = Sequence::new(
        vec![Size(1), Gap(5), Size(2), Gap(10), Size(1)],
        "".to_string(),
    );
    assert_eq!(sequence.classify(), Some(R002));

    let sequence = Sequence::new(
        vec![Gap(2), Size(1), Gap(15), Size(2), Gap(2), Size(1), Gap(3)],
        "".to_string(),
    );
    assert_eq!(sequence.classify(), Some(R002));

    // These must fail

    let sequence = Sequence::new(vec![Size(1), Size(1), Size(1)], "".to_string());
    assert_ne!(sequence.classify(), Some(R002));

    let sequence = Sequence::new(vec![Size(1), Size(1), Size(2)], "".to_string());
    assert_ne!(sequence.classify(), Some(R002));

    let sequence = Sequence::new(vec![Size(2), Size(1), Size(1)], "".to_string());
    assert_ne!(sequence.classify(), Some(R002));
}

#[test]
fn test_classify_sequence_r004() {
    use SequenceElement::{Gap, Size};

    let sequence = Sequence::new(vec![Size(1)], "".to_string());
    assert_eq!(sequence.classify(), Some(R004_SIZE1));
}

#[test]
fn test_classify_sequence_r007() {
    use SequenceElement::{Gap, Size};

    let sequence = Sequence::new(vec![Size(1), Gap(3), Size(1)], "".to_string());
    assert_eq!(sequence.classify(), Some(R007));

    let sequence = Sequence::new(
        vec![Size(1), Gap(3), Size(1), Gap(3), Size(1), Gap(3), Size(1)],
        "".to_string(),
    );
    assert_eq!(sequence.classify(), Some(R007));

    // These must fail

    let sequence = Sequence::new(
        vec![
            Size(1),
            Gap(2),
            Size(2),
            Gap(9),
            Size(1),
            Gap(2),
            Size(1),
            Gap(9),
            Size(1),
            Gap(2),
            Size(1),
            Gap(1),
            Size(1),
            Gap(2),
            Size(1),
            Gap(8),
            Size(1),
            Gap(2),
            Size(1),
            Gap(2),
            Size(2),
            Gap(2),
            Size(2),
        ],
        "".to_string(),
    );

    assert_ne!(sequence.classify(), Some(R007));
}
