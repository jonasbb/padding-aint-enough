#![feature(proc_macro, specialization, const_fn, nll)]

extern crate failure;
#[macro_use]
extern crate log;
extern crate misc_utils;
extern crate pyo3;
extern crate rayon;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_pickle;

mod knn;

pub use knn::*;

use failure::{format_err, Error};
use misc_utils::fs::{file_open_read, file_open_write, WriteOptions};
use pyo3::exc::Exception;
use pyo3::prelude::*;
use pyo3::py::class as pyclass;
use pyo3::py::methods as pymethods;
use pyo3::py::modinit as pymodinit;
use pyo3::py::proto as pyproto;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::mem;
use std::path::Path;

fn error2py(err: Error) -> PyErr {
    PyErr::new::<Exception, _>(format!("{}", err))
}

// Add bindings to the generated python module
// N.B: names: "librust2py" must be the name of the `.so` or `.pyd` file
/// This module is implemented in Rust.
#[pymodinit(_pylib)]
fn init_mod(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PySequence>()?;

    #[pyfn(m, "load_sequence")]
    fn load_sequence(py: Python, path: String) -> PyResult<Py<PySequence>> {
        let seq = load_as_pickle(Path::new(&*path)).map_err(error2py)?;
        py.init(|token| PySequence::new(seq, token))
    }

    Ok(())
}

#[pyclass(name=Sequence)]
pub struct PySequence {
    sequence: Sequence,
    token: PyToken,
}

impl PySequence {
    pub fn new(sequence: Sequence, token: PyToken) -> PySequence {
        PySequence { sequence, token }
    }
}

#[pymethods]
impl PySequence {
    pub fn distance(&self, other: &PySequence) -> PyResult<usize> {
        Ok(self.sequence.distance(&other.sequence))
    }
}

#[pyproto]
impl<'p> PyObjectProtocol<'p> for PySequence {
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self.sequence))
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self.sequence))
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default, Serialize, Deserialize)]
pub struct Sequence(Vec<SequenceElement>);

impl Sequence {
    pub fn new(sequence: Vec<SequenceElement>) -> Sequence {
        Sequence(sequence)
    }
}

impl Sequence {
    pub fn distance(&self, other: &Self) -> usize {
        if self.0.len() < other.0.len() {
            return other.distance(self);
        }
        // other is always shorter or equal sized

        if other.0.is_empty() {
            let mut cost: usize = 0;
            for x in &self.0 {
                cost = cost.saturating_add(x.insert_cost());
            }
            return cost;
        }

        let mut prev_prev_row = vec![0usize; other.0.len() + 1];
        // let mut previous_row: Vec<usize> = (0..(other.0.len() + 1)).into_iter().collect();
        let mut cost = 0;
        let mut previous_row: Vec<usize> = Some(0)
            .into_iter()
            .chain(other.0.iter().cloned().map(|elem| elem.insert_cost()))
            .map(|c| {
                cost += c;
                cost
            })
            .collect();
        let mut current_row = vec![0usize; other.0.len() + 1];
        assert_eq!(
            previous_row.len(),
            current_row.len(),
            "Row length must be equal"
        );

        for (i, elem1) in self.0.iter().enumerate() {
            current_row.clear();
            // TODO give different costs for different elements
            current_row.push(previous_row[0].saturating_add(elem1.delete_cost()));

            for (j, &elem2) in other.0.iter().enumerate() {
                let insertions = previous_row[j + 1].saturating_add(elem1.insert_cost());
                let deletions = current_row[j].saturating_add(elem2.delete_cost());
                let substitutions = previous_row[j].saturating_add(elem1.substitute_cost(elem2));
                let swapping =
                    if i > 0 && j > 0 && self.0[i] == other.0[j - 1] && self.0[i - 1] == other.0[j]
                    {
                        prev_prev_row[j - 1].saturating_add(elem1.swap_cost(elem2))
                    } else {
                        std::usize::MAX
                    };
                let cost = insertions.min(deletions).min(substitutions).min(swapping);
                current_row.push(cost);
            }

            mem::swap(&mut prev_prev_row, &mut previous_row);
            mem::swap(&mut previous_row, &mut current_row);
        }

        *previous_row
            .last()
            .expect("The rows are never empty, thus there is a last.")
    }
}

impl From<Vec<SequenceElement>> for Sequence {
    fn from(sequence: Vec<SequenceElement>) -> Self {
        Self::new(sequence)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum SequenceElement {
    Size(u8),
    Gap(u8),
}

impl SequenceElement {
    fn insert_cost(self) -> usize {
        use SequenceElement::*;
        match self {
            Size(0) => {
                // A size 0 packet should never occur
                error!("Sequence contains a Size(0) elements");
                std::usize::MAX
            }
            Size(_) => 10,
            Gap(g) => g as usize * 3,
        }
    }

    fn delete_cost(self) -> usize {
        // The delete costs have to be identical to the insert costs in order to be a metric.
        // There is no order in which two Sequences will be compared, so
        // xABCy -> xACy
        // must be the same as
        // xACy -> xABCy
        self.insert_cost()
    }

    fn substitute_cost(self, other: Self) -> usize {
        if self == other {
            return 0;
        }

        use SequenceElement::*;
        match (self, other) {
            // 2/3rds cost
            (Size(_), Size(_)) => self.insert_cost().saturating_add(other.delete_cost()) / 3,
            (Gap(g1), Gap(g2)) => (g1.max(g2) - g1.min(g2)) as usize * 2,
            (a, b) => a.delete_cost().saturating_add(b.insert_cost()),
        }
    }

    fn swap_cost(self, other: Self) -> usize {
        if self == other {
            return 0;
        }

        10
    }
}

#[allow(dead_code)]
fn load_as_pickle<'de, D: Deserialize<'de>>(path: &Path) -> Result<D, Error> {
    let mut rdr = file_open_read(path)
        .map_err(|err| format_err!("Opening input file '{}' failed: {}", path.display(), err))?;
    Ok(serde_pickle::from_reader(&mut rdr)?)
}

#[allow(dead_code)]
fn save_as_pickle<D: Serialize>(path: &Path, data: &D) -> Result<(), Error> {
    let mut wtr = file_open_write(
        path,
        WriteOptions::default().set_open_options(OpenOptions::new().create(true).truncate(true)),
    ).map_err(|err| {
        format_err!("Opening output file '{}' failed: {}", path.display(), err)
    })?;
    serde_pickle::to_writer(&mut wtr, data, true)?;
    Ok(())
}

#[test]
fn test_edit_distance_dist1() {
    use SequenceElement::*;
    let seq1 = Sequence(vec![Size(1), Gap(2), Size(1), Size(2), Size(1)]);

    // substitution
    let seq2 = Sequence(vec![Size(2), Gap(2), Size(1), Size(2), Size(1)]);
    assert_eq!(6, seq1.distance(&seq2));

    // swapping
    let seq3 = Sequence(vec![Size(1), Gap(2), Size(2), Size(1), Size(1)]);
    assert_eq!(10, seq1.distance(&seq3));

    // deletion
    let seq4 = Sequence(vec![Size(1), Size(1), Size(2), Size(1)]);
    assert_eq!(6, seq1.distance(&seq4));

    // insertion
    let seq5 = Sequence(vec![Size(1), Size(2), Gap(2), Size(1), Size(2), Size(1)]);
    assert_eq!(10, seq1.distance(&seq5));
}

#[test]
fn test_edit_distance_inserts() {
    use SequenceElement::*;
    let seq1 = Sequence(vec![]);
    let seq2 = Sequence(vec![Size(1), Size(1)]);

    let seq6 = Sequence(vec![Gap(3)]);
    let seq7 = Sequence(vec![Gap(10)]);
    println!("Smaller gap: {}", seq1.distance(&seq6));
    println!("Bigger gap: {}", seq1.distance(&seq7));
    assert!(
        seq1.distance(&seq6) < seq1.distance(&seq7),
        "Bigger Gaps have higher cost."
    );

    let seq6 = Sequence(vec![Size(1), Gap(3), Size(1)]);
    let seq7 = Sequence(vec![Size(1), Gap(10), Size(1)]);
    println!("Smaller gap: {}", seq2.distance(&seq6));
    println!("Bigger gap: {}", seq2.distance(&seq7));
    assert!(
        seq2.distance(&seq6) < seq2.distance(&seq7),
        "Bigger Gaps have higher cost."
    );
}

#[test]
fn test_edit_distance_substitutions() {
    use SequenceElement::*;
    let seq1 = Sequence(vec![Size(1)]);
    let seq2 = Sequence(vec![Gap(10)]);

    let seqa = Sequence(vec![Gap(9)]);
    let seqb = Sequence(vec![Gap(1)]);
    println!("Smaller gap change: {}", seq2.distance(&seqa));
    println!("Bigger gap change: {}", seq2.distance(&seqb));
    assert!(
        seq2.distance(&seqa) < seq2.distance(&seqb),
        "Bigger Gap changes have higher cost."
    );

    println!("Size to Gap change: {}", seq1.distance(&seqa));
    println!("Gap to Gap change: {}", seq2.distance(&seqa));
    assert!(
        seq1.distance(&seqa) > seq2.distance(&seqa),
        "Gap to Gap change is smaller than Size to Gap change"
    )
}

#[test]
fn test_edit_distance_equal() {
    use SequenceElement::*;
    let seq1 = Sequence::new(vec![]);
    let seq2 = Sequence::new(vec![]);
    assert_eq!(seq1, seq2);
    assert_eq!(0, seq1.distance(&seq2));

    let seq3 = Sequence(vec![Size(1), Gap(2), Size(1), Size(2), Size(1)]);
    let seq4 = Sequence(vec![Size(1), Gap(2), Size(1), Size(2), Size(1)]);
    assert_eq!(0, seq3.distance(&seq4));
}
