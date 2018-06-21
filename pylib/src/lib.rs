#![feature(proc_macro, specialization, const_fn)]

extern crate pyo3;

use pyo3::prelude::*;

use pyo3::py::class as pyclass;
use pyo3::py::methods as pymethods;
use pyo3::py::modinit as pymodinit;
use pyo3::py::proto as pyproto;
use std::mem;

// Add bindings to the generated python module
// N.B: names: "librust2py" must be the name of the `.so` or `.pyd` file
/// This module is implemented in Rust.
#[pymodinit(_pylib)]
fn init_mod(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PySequence>()?;

    #[pyfn(m, "get_sequence")]
    fn get_sequence(py: Python) -> PyResult<Py<PySequence>> {
        py.init(|token| PySequence::new(Sequence::new(), token))
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

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct Sequence(Vec<SequenceElement>);

impl Sequence {
    pub fn new() -> Sequence {
        Sequence(vec![
            SequenceElement::Size(1),
            SequenceElement::Gap(5),
            SequenceElement::Size(3),
        ])
    }
}

impl Sequence {
    pub fn distance(&self, other: &Self) -> usize {
        if self.0.len() < other.0.len() {
            return other.distance(self);
        }
        // other is always shorter or equal sized

        if other.0.is_empty() {
            // TODO give different costs for different elements
            return self.0.len();
        }

        let mut prev_prev_row = vec![0usize; other.0.len() + 1];
        let mut previous_row: Vec<usize> = (0..(other.0.len() + 1)).into_iter().collect();
        let mut current_row = vec![0usize; other.0.len() + 1];
        assert_eq!(
            previous_row.len(),
            current_row.len(),
            "Row length must be equal"
        );

        for (i, elem1) in self.0.iter().enumerate() {
            current_row.clear();
            // TODO give different costs for different elements
            current_row.push(i + 1);

            for (j, elem2) in other.0.iter().enumerate() {
                // TODO give different costs for different elements
                let insertions = previous_row[j + 1] + 1;
                let deletions = current_row[j] + 1;
                let substitutions = if elem1 == elem2 {
                    previous_row[j]
                } else {
                    previous_row[j] + 1
                };
                let mut cost = insertions.min(deletions).min(substitutions);

                // swapping
                if i > 0 && j > 0 && self.0[i] == other.0[j - 1] && self.0[i - 1] == other.0[j] {
                    // TODO give different costs for different elements
                    cost = cost.min(prev_prev_row[j - 1] + 1)
                }

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

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum SequenceElement {
    Size(u8),
    Gap(u8),
}

#[test]
fn test_edit_distance_dist1() {
    use SequenceElement::*;
    let seq1 = Sequence(vec![Size(1), Gap(2), Size(1), Size(2), Size(1)]);

    // substitution
    let seq2 = Sequence(vec![Size(2), Gap(2), Size(1), Size(2), Size(1)]);
    assert_eq!(1, seq1.distance(&seq2));

    // swapping
    let seq3 = Sequence(vec![Size(1), Gap(2), Size(2), Size(1), Size(1)]);
    assert_eq!(1, seq1.distance(&seq3));

    // deletion
    let seq4 = Sequence(vec![Size(1), Size(1), Size(2), Size(1)]);
    assert_eq!(1, seq1.distance(&seq4));

    // insertion
    let seq5 = Sequence(vec![Size(1), Size(2), Gap(2), Size(1), Size(2), Size(1)]);
    assert_eq!(1, seq1.distance(&seq5));
}

#[test]
fn test_edit_distance_equal() {
    use SequenceElement::*;
    let seq1 = Sequence::new();
    let seq2 = Sequence::new();
    assert_eq!(seq1, seq2);
    assert_eq!(0, seq1.distance(&seq2));

    let seq3 = Sequence(vec![Size(1), Gap(2), Size(1), Size(2), Size(1)]);
    let seq4 = Sequence(vec![Size(1), Gap(2), Size(1), Size(2), Size(1)]);
    assert_eq!(0, seq3.distance(&seq4));
}
