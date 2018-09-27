use min_max_heap::MinMaxHeap;

pub fn take_smallest<I, T>(iter: I, n: usize) -> Vec<T>
where
    I: IntoIterator<Item = T>,
    T: Ord,
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
