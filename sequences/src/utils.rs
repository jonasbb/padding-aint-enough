use crate::{knn::ClassifierData, LoadSequenceConfig, Sequence};
use failure::{bail, Error, ResultExt};
use log::{debug, warn};
use misc_utils::path::PathExt;
use rayon::prelude::*;
use serde::Serialize;
use std::{
    cmp,
    ffi::OsStr,
    fmt, fs,
    path::{Path, PathBuf},
    str::FromStr,
};

pub fn load_all_dnstap_files_from_dir(
    base_dir: &Path,
) -> Result<Vec<(String, Vec<Sequence>)>, Error> {
    load_all_dnstap_files_from_dir_with_config(base_dir, LoadSequenceConfig::default())
}

pub fn load_all_dnstap_files_from_dir_with_config(
    base_dir: &Path,
    config: LoadSequenceConfig,
) -> Result<Vec<(String, Vec<Sequence>)>, Error> {
    load_all_files_with_extension_from_dir_with_config(base_dir, &OsStr::new("dnstap"), config)
}

pub fn load_all_files_with_extension_from_dir_with_config(
    base_dir: &Path,
    file_extension: &OsStr,
    config: LoadSequenceConfig,
) -> Result<Vec<(String, Vec<Sequence>)>, Error> {
    // Get a list of directories
    // Each directory corresponds to a label
    let mut directories: Vec<PathBuf> = fs::read_dir(base_dir)?
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
    directories.sort();

    // Pairs of Label with Data (the Sequences)
    let data: Vec<(String, Vec<Sequence>)> = directories
        .into_par_iter()
        .with_max_len(1)
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
                                && entry.path().extensions().any(|ext| ext == file_extension)
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
                .filter_map(|file| {
                    debug!("Processing {:?} file '{}'", file_extension, file.display());
                    match Sequence::from_path_with_config(&file, config).with_context(|_| {
                        format!("Processing {:?} file '{}'", file_extension, file.display())
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
        .filter_map(Result::transpose)
        .collect::<Result<_, Error>>()?;

    // return all loaded data
    Ok(data)
}

/// Take the `n` smallest elements from `iter`
///
/// It is unspecified which `n` smallest elements are being returned.
pub(crate) fn take_smallest<'a, I, S>(iter: I, n: usize) -> Vec<ClassifierData<'a, S>>
where
    I: IntoIterator<Item = ClassifierData<'a, S>>,
    S: ?Sized,
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
        }
    }

    debug_assert!(res.len() <= n, "Output vector only contains n elements.");
    res
}

#[test]
fn take_smallest_empty() {
    let res = take_smallest(
        vec![]
            .into_iter()
            .map(|_: usize| -> ClassifierData<'static, str> { unimplemented!() }),
        1,
    );
    assert!(res.is_empty());

    let res = take_smallest(
        vec![]
            .into_iter()
            .map(|_: usize| -> ClassifierData<'static, str> { unimplemented!() }),
        12,
    );
    assert!(res.is_empty());
}

/// Represents an arbitraty propability value
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Default, Serialize)]
pub struct Probability(f32);

impl Probability {
    /// Create a new probability value
    ///
    /// Returns an Error if the value is negative, larger than 1, or NaN.
    pub fn new(pb: f32) -> Result<Self, Error> {
        if !pb.is_finite() || pb < 0. || pb > 1. {
            bail!(
                "A probability has to be finite, not NaN and 0 <= x <= 1, but value was: {}",
                pb
            )
        } else {
            Ok(Probability(pb))
        }
    }

    pub fn to_float(self) -> f32 {
        self.0
    }
}

// Implementing `Eq` is fine, as the internal float cannot be `NaN` or infinite.
impl Eq for Probability {}

impl Ord for Probability {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.partial_cmp(other).unwrap_or(cmp::Ordering::Equal)
    }
}

impl FromStr for Probability {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let pb = f32::from_str(s)?;
        Ok(Self::new(pb)?)
    }
}

impl fmt::Display for Probability {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}
