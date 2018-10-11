use knn::ClassifierData;

pub(crate) fn take_smallest<'a, I, F, S>(iter: I, n: usize) -> Vec<ClassifierData<'a, S>>
where
    I: IntoIterator<Item = F>,
    F: Fn(usize) -> ClassifierData<'a, S>,
{
    let mut iter = iter.into_iter();
    if n == 1 {
        // get a first element to make the rest of the code simpler
        let best = iter.next();
        // iter is empty
        if best.is_none() {
            return vec![];
        }
        let mut best = best.unwrap()(usize::max_value());
        // The first element could already be a best match
        if best.distance == 0 {
            return vec![best];
        }

        for elem in iter {
            // convert to ClassifierData with max distance
            let elem = elem(best.distance);
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
    res.extend((&mut iter).take(n).map(|f| f(usize::max_value())));
    res.sort();

    // the iterator is already exhausted, so we can stop early
    // This hopefully also tells LLVM, that array indexing is fine
    if res.len() < n {
        return res;
    }

    // replace exisiting elements keeping the heap size
    for v in iter {
        // convert to ClassifierData with max distance
        let v = v(res[n - 1].distance);
        // compare with worst element so far
        if v < res[n - 1] {
            res[n - 1] = v;
            res.sort();
        }
    }

    debug_assert!(
        res.len() <= n,
        "Output vector only contains more than n elements."
    );
    res
}

// #[allow(dead_code)]
// fn take_smallest<I, T>(iter: I, n: usize) -> Vec<T>
// where
//     I: IntoIterator<Item = T>,
//     T: Ord,
// {
//     let mut iter = iter.into_iter();
//     if n == 1 {
//         // simply take the largest value and return it
//         return iter.min().into_iter().collect();
//     }

//     let mut heap = MinMaxHeap::with_capacity(n);
//     // fill the heap with n elements
//     for _ in 0..n {
//         match iter.next() {
//             Some(v) => heap.push(v),
//             None => break,
//         }
//     }

//     // replace exisiting elements keeping the heap size
//     for v in iter {
//         heap.push_pop_max(v);
//     }

//     let res = heap.into_vec_asc();
//     assert!(
//         res.len() <= n,
//         "Output vector only contains more than n elements."
//     );
//     res
// }
