#![feature(specialization)]
#![cfg_attr(
    feature = "cargo-clippy",
    allow(renamed_and_removed_lints, clippy)
)]

extern crate encrypted_dns;
extern crate failure;
extern crate pyo3;

use encrypted_dns::{dnstap_to_sequence, ErrorExt, Sequence};
use failure::Error;
use pyo3::{exc::Exception, prelude::*};
use std::path::Path;

fn error2py(err: Error) -> PyErr {
    PyErr::new::<Exception, _>(format!("{}", err.display_causes()))
}

// Function name is module name
#[pymodinit]
fn pylib(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PySequence>()?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    #[pyfn(m, "load_file")]
    fn load_file(py: Python, path: String) -> PyResult<Py<PySequence>> {
        let seq = dnstap_to_sequence(Path::new(&path)).map_err(error2py)?;
        py.init(|token| PySequence::new(seq, token))
    }

    Ok(())
}

/// Represents a sequence of DNS packets as measured on the wire
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
    /// Returns a unique identifier for this sequence
    pub fn id(&self) -> PyResult<String> {
        Ok(self.sequence.id().to_string())
    }

    /// Calculate the distance between two sequences
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

    fn __richcmp__(&self, other: &Self, op: CompareOp) -> PyResult<bool> {
        Ok(match op {
            CompareOp::Eq => self.sequence == other.sequence,
            CompareOp::Ge => self.sequence >= other.sequence,
            CompareOp::Gt => self.sequence > other.sequence,
            CompareOp::Le => self.sequence <= other.sequence,
            CompareOp::Lt => self.sequence < other.sequence,
            CompareOp::Ne => self.sequence != other.sequence,
        })
    }
}
