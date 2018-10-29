use crate::Sequence;
use failure::{Error, ResultExt};
use knn::ClassifierData;
use log::{debug, warn};
use rayon::prelude::*;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn load_all_dnstap_files_from_dir(
    base_dir: &Path,
) -> Result<Vec<(String, Vec<Sequence>)>, Error> {
    // Get a list of directories
    // Each directory corresponds to a label
    let directories: Vec<PathBuf> = fs::read_dir(base_dir)?
        .flat_map(|x| {
            x.and_then(|entry| {
                // Result<Option<PathBuf>>
                entry.file_type().map(|ft| {
                    if ft.is_dir()
                        || (ft.is_symlink() && fs::metadata(&entry.path()).ok()?.is_dir())
                    {
                        Some(entry.path())
                    } else {
                        None
                    }
                })
            })
            .transpose()
        })
        .collect::<Result<_, _>>()?;

    // Pairs of Label with Data (the Sequences)
    let data: Vec<(String, Vec<Sequence>)> = directories
        .into_par_iter()
        .map(|dir| {
            let label = dir
                .file_name()
                .expect("Each directory has a name")
                .to_string_lossy()
                .into();

            let mut filenames: Vec<PathBuf> = fs::read_dir(&dir)?
                .flat_map(|x| {
                    x.and_then(|entry| {
                        // Result<Option<PathBuf>>
                        entry.file_type().map(|ft| {
                            if ft.is_file()
                                && entry.file_name().to_string_lossy().contains(".dnstap")
                            {
                                Some(entry.path())
                            } else {
                                None
                            }
                        })
                    })
                    .transpose()
                })
                .collect::<Result<_, _>>()?;
            // sort filenames for predictable results
            filenames.sort();

            let sequences: Vec<Sequence> = filenames
                .into_iter()
                .filter_map(|dnstap_file| {
                    debug!("Processing dnstap file '{}'", dnstap_file.display());
                    match Sequence::from_path(&*dnstap_file).with_context(|_| {
                        format!("Processing dnstap file '{}'", dnstap_file.display())
                    }) {
                        Ok(seq) => Some(seq),
                        Err(err) => {
                            warn!("{}", err);
                            None
                        }
                    }
                })
                .collect();

            // Some directories do not contain data, e.g., because the site didn't exists
            // Skip all directories with 0 results
            if sequences.is_empty() {
                warn!("Directory contains no data: {}", dir.display());
                Ok(None)
            } else {
                Ok(Some((label, sequences)))
            }
        })
        // Remove all the empty directories from the previous step
        .filter_map(|x| x.transpose())
        .collect::<Result<_, Error>>()?;

    // return all loaded data
    Ok(data)
}

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
