extern crate minmax;

use minmax::Min;

#[test]
fn test_min_usize() {
    let mut m = Min::default();
    assert_eq!(None, m.get_min());
    assert_eq!(usize::max_value(), m.get_min_extreme());

    m.update(999);
    assert_eq!(Some(999), m.get_min());
    assert_eq!(999, m.get_min_extreme());
    m.update(1000);
    assert_eq!(Some(999), m.get_min());
    assert_eq!(999, m.get_min_extreme());
    m.update(100);
    assert_eq!(Some(100), m.get_min());
    assert_eq!(100, m.get_min_extreme());
}

#[test]
fn test_min_isize() {
    let mut m = Min::default();
    assert_eq!(None, m.get_min());
    assert_eq!(isize::max_value(), m.get_min_extreme());

    m.update(999);
    assert_eq!(Some(999), m.get_min());
    assert_eq!(999, m.get_min_extreme());
    m.update(1000);
    assert_eq!(Some(999), m.get_min());
    assert_eq!(999, m.get_min_extreme());
    m.update(100);
    assert_eq!(Some(100), m.get_min());
    assert_eq!(100, m.get_min_extreme());
    m.update(-5);
    assert_eq!(Some(-5), m.get_min());
    assert_eq!(-5, m.get_min_extreme());
}

#[test]
fn test_min_i16_update_min() {
    let mut m1 = Min::<i16>::default();
    let m2 = Min::default();

    m1.update(m2);
    assert_eq!(None, m1.get_min());

    let m2 = Min::with_initial(1000);
    m1.update(m2);
    assert_eq!(Some(1000), m1.get_min());

    let m2 = Min::with_initial(9999);
    m1.update(m2);
    assert_eq!(Some(1000), m1.get_min());

    let m2 = Min::with_initial(-200);
    m1.update(m2);
    assert_eq!(Some(-200), m1.get_min());
}

#[test]
fn test_min_u8_from() {
    assert_eq!(Min::with_initial(99u8), 99.into());
    assert_eq!(Min::with_initial(123u8), Min::from(123));
    assert_eq!(Min::with_initial(1u8), vec![1, 2, 3].into_iter().collect());
    assert_eq!(Min::<u8>::default(), vec![].into_iter().collect());
}
