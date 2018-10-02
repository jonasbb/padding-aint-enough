#[macro_use]
extern crate criterion;
extern crate min_max_heap;

use criterion::{Criterion, Fun};
use min_max_heap::MinMaxHeap;
use std::cmp::Ordering;

fn make_data() -> impl Iterator<Item = ClassifierData<'static, &'static str>> {
    vec![
        vec![54, 84, 98, 40, 62, 62, 64, 64, 68],
        vec![727, 719, 719, 719, 750, 677, 646, 650, 680],
        vec![3397, 3318, 3317, 3311, 3459, 3233, 3328, 3173, 3032],
        vec![171, 166, 172, 162, 162, 177, 164, 162, 149],
        vec![165, 172, 167, 165, 163, 170, 175, 170, 165],
        vec![367, 361, 369, 367, 367, 367, 369, 367, 367],
        vec![1221, 1225, 1232, 1153, 1140, 1152, 1161, 1163, 1184],
        vec![955, 947, 980, 962, 947, 964, 980, 965, 959],
        vec![5593, 5526, 5560, 5328, 5263, 5800, 5360, 5329, 5288],
        vec![369, 306, 313, 325, 330, 320, 349, 272, 274],
        vec![2377, 2410, 2439, 2409, 2453, 2332, 2398, 2293, 2378],
        vec![399, 404, 398, 387, 416, 416, 384, 397, 391],
        vec![1416, 1384, 1415, 1461, 1370, 1374, 1348, 1357, 1413],
        vec![2802, 2605, 2607, 2645, 2607, 2621, 2608, 2853, 2603],
        vec![188, 204, 226, 212, 249, 226, 236, 249, 225],
        vec![765, 705, 666, 704, 633, 684, 729, 655, 600],
        vec![1765, 1612, 1611, 1564, 1631, 1616, 1500, 1533, 1371],
        vec![310, 310, 314, 314, 312, 314, 312, 312, 312],
        // here is a 0 distance value
        vec![218, 224, 208, 237, 227, 237, 204, 229, 0],
        vec![1926, 2122, 1921, 2256, 1715, 1674, 1459, 1428, 1520],
        vec![1434, 1370, 1354, 1344, 1338, 1354, 1323, 1294, 1321],
        vec![5166, 5226, 5234, 5167, 5257, 5176, 5171, 5219, 5105],
        vec![1300, 1179, 1258, 1236, 1153, 1176, 1195, 1183, 1223],
        vec![1879, 1771, 1749, 1676, 1676, 1646, 1648, 1638, 1664],
        vec![1505, 1492, 1547, 1514, 1475, 1475, 1450, 1429, 1422],
        vec![369, 369, 361, 369, 361, 361, 361, 361, 361],
        vec![1884, 1892, 1974, 1894, 1933, 1875, 1889, 1919, 1892],
        vec![777, 723, 718, 733, 737, 701, 724, 744, 705],
        vec![8103, 8216, 7872, 7870, 7956, 7942, 7215, 7158, 6855],
        vec![7478, 7302, 7232, 6971, 6925, 6173, 5487, 5476, 5130],
        vec![6927, 7003, 6496, 6578, 6500, 6477, 6504, 5866, 5641],
        vec![2481, 2371, 2442, 2432, 2392, 2378, 2303, 2448, 2274],
        vec![389, 383, 394, 386, 397, 410, 398, 385, 353],
        vec![810, 800, 818, 828, 813, 803, 802, 818, 780],
        vec![3685, 3440, 3598, 3496, 3374, 3503, 3390, 3514, 3347],
        vec![869, 724, 788, 733, 727, 816, 860, 714, 665],
        vec![7446, 7252, 6781, 6700, 6705, 6487, 6384, 6497, 5334],
        vec![269, 244, 261, 229, 226, 244, 201, 249, 202],
        vec![231, 253, 241, 254, 244, 266, 254, 248, 268],
        vec![6925, 6623, 6442, 6933, 6570, 6339, 5980, 5788, 5354],
        vec![4829, 5039, 5043, 4329, 4411, 4279, 4318, 4306, 4079],
        vec![2898, 2822, 2809, 2805, 2786, 2807, 2782, 2742, 2781],
        vec![412, 410, 412, 412, 412, 412, 412, 414, 412],
        vec![2592, 2584, 2580, 2488, 2521, 2489, 2518, 2583, 2533],
        vec![1737, 1742, 1716, 1722, 1719, 1732, 1682, 1661, 1626],
    ]
    .into_iter()
    .flat_map(|dists| {
        dists.into_iter().map(move |d| ClassifierData {
            label: &"Constant Label",
            distance: d,
        })
    })
}

fn take_smallest_baseline<'a, I, S>(iter: I, n: usize) -> Vec<ClassifierData<'a, S>>
where
    I: IntoIterator<Item = ClassifierData<'a, S>>,
{
    let mut iter = iter.into_iter();
    if n == 1 {
        // simply take the largest value and return it
        return iter.min().into_iter().collect();
    }

    let mut heap = MinMaxHeap::with_capacity(n);
    // fill the heap with n elements
    for _ in 0..n {
        match iter.next() {
            Some(v) => heap.push(v),
            None => break,
        }
    }

    // replace exisiting elements keeping the heap size
    for v in iter {
        heap.push_pop_max(v);
    }

    let res = heap.into_vec_asc();
    assert!(
        res.len() <= n,
        "Output vector only contains more than n elements."
    );
    res
}

fn take_smallest_new<'a, I, S>(iter: I, n: usize) -> Vec<ClassifierData<'a, S>>
where
    I: IntoIterator<Item = ClassifierData<'a, S>>,
{
    let mut iter = iter.into_iter();
    if n == 1 {
        // get a first element to make the rest of the code simpler
        let best = iter.next();
        // iter is empty
        if best.is_none() {
            return vec![];
        }
        let mut best = best.unwrap();
        // The first element could already be a best match
        if best.distance == 0 {
            return vec![best];
        }

        for elem in iter {
            if elem < best {
                // found a better element, so replace the current best
                best = elem;
                // better element is also best possible, stop search
                if best.distance == 0 {
                    return vec![best];
                }
            }
        }

        // return whatever the current best is
        return vec![best];
    }

    let mut heap = MinMaxHeap::with_capacity(n);
    // fill the heap with n elements
    for _ in 0..n {
        match iter.next() {
            Some(v) => heap.push(v),
            None => break,
        }
    }

    // replace exisiting elements keeping the heap size
    for v in iter {
        heap.push_pop_max(v);
    }

    let res = heap.into_vec_asc();
    assert!(
        res.len() <= n,
        "Output vector only contains more than n elements."
    );
    res
}

fn take_smallest_new_no_heap<'a, I, S>(iter: I, n: usize) -> Vec<ClassifierData<'a, S>>
where
    I: IntoIterator<Item = ClassifierData<'a, S>>,
{
    let mut iter = iter.into_iter();
    if n == 1 {
        // get a first element to make the rest of the code simpler
        let best = iter.next();
        // iter is empty
        if best.is_none() {
            return vec![];
        }
        let mut best = best.unwrap();
        // The first element could already be a best match
        if best.distance == 0 {
            return vec![best];
        }

        for elem in iter {
            if elem < best {
                // found a better element, so replace the current best
                best = elem;
                // better element is also best possible, stop search
                if best.distance == 0 {
                    return vec![best];
                }
            }
        }

        // return whatever the current best is
        return vec![best];
    }

    let mut res = Vec::with_capacity(n);
    // fill the vector with n elements
    res.extend((&mut iter).take(n));
    res.sort();

    // the iterator is already exhausted, so we can stop early
    // This hopefully also tells LLVM, that array indexing is fine
    if res.len() < n {
        return res;
    }

    // replace exisiting elements keeping the heap size
    for v in iter {
        // compare with worst element so far
        if v < res[n - 1] {
            res[n - 1] = v;
            res.sort();
            // found enough 0-distance cases that all cases are best
            if res[n - 1].distance == 0 {
                return res;
            }
        }
    }

    debug_assert!(
        res.len() <= n,
        "Output vector only contains more than n elements."
    );
    res
}

#[derive(Debug)]
struct ClassifierData<'a, S>
where
    S: 'a,
{
    label: &'a S,
    pub distance: usize,
}

impl<'a, S> PartialEq for ClassifierData<'a, S> {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl<'a, S> Eq for ClassifierData<'a, S> {}

impl<'a, S> PartialOrd for ClassifierData<'a, S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl<'a, S> Ord for ClassifierData<'a, S> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance.cmp(&other.distance)
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let mkfun = || {
        let smallest_baseline = Fun::new("Baseline", |b, k| {
            b.iter_with_setup(make_data, |iter| take_smallest_baseline(iter, *k))
        });
        let smallest_new = Fun::new("New", |b, k| {
            b.iter_with_setup(make_data, |iter| take_smallest_new(iter, *k))
        });
        let take_smallest_new_no_heap = Fun::new("New No heap", |b, k| {
            b.iter_with_setup(make_data, |iter| take_smallest_new_no_heap(iter, *k))
        });
        vec![smallest_baseline, smallest_new, take_smallest_new_no_heap]
    };

    c.bench_functions("k=1", mkfun(), 1);
    c.bench_functions("k=3", mkfun(), 3);
    c.bench_functions("k=5", mkfun(), 5);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
