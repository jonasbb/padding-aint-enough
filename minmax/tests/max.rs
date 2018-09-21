extern crate minmax;

use minmax::Max;

#[test]
fn test_max_usize() {
    let mut m = Max::default();
    assert_eq!(None, m.get_max());
    assert_eq!(usize::min_value(), m.get_max_extreme());

    m.update(100);
    assert_eq!(Some(100), m.get_max());
    assert_eq!(100, m.get_max_extreme());
    m.update(1000);
    assert_eq!(Some(1000), m.get_max());
    assert_eq!(1000, m.get_max_extreme());
    m.update(999);
    assert_eq!(Some(1000), m.get_max());
    assert_eq!(1000, m.get_max_extreme());
}

#[test]
fn test_max_isize() {
    let mut m = Max::default();
    assert_eq!(None, m.get_max());
    assert_eq!(isize::min_value(), m.get_max_extreme());

    m.update(-5);
    assert_eq!(Some(-5), m.get_max());
    assert_eq!(-5, m.get_max_extreme());
    m.update(100);
    assert_eq!(Some(100), m.get_max());
    assert_eq!(100, m.get_max_extreme());
    m.update(1000);
    assert_eq!(Some(1000), m.get_max());
    assert_eq!(1000, m.get_max_extreme());
    m.update(999);
    assert_eq!(Some(1000), m.get_max());
    assert_eq!(1000, m.get_max_extreme());
}

#[test]
fn test_max_i16_update_max() {
    let mut m1 = Max::<i16>::default();
    let m2 = Max::default();

    m1.update(m2);
    assert_eq!(None, m1.get_max());

    let m2 = Max::with_initial(1000);
    m1.update(m2);
    assert_eq!(Some(1000), m1.get_max());

    let m2 = Max::with_initial(9999);
    m1.update(m2);
    assert_eq!(Some(9999), m1.get_max());

    let m2 = Max::with_initial(-200);
    m1.update(m2);
    assert_eq!(Some(9999), m1.get_max());
}

#[test]
fn test_max_u8_from() {
    assert_eq!(Max::with_initial(99u8), 99.into());
    assert_eq!(Max::with_initial(123u8), Max::from(123));
    assert_eq!(Max::with_initial(3u8), vec![1, 2, 3].into_iter().collect());
    assert_eq!(Max::<u8>::default(), vec![].into_iter().collect());
}
