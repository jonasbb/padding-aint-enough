#![feature(specialization)]
#![allow(clippy::all)]

use encrypted_dns::ErrorExt;
use failure::Error;
use pyo3::{self, exceptions::Exception, prelude::*, CompareOp, PyObjectProtocol};
use sequences::{load_all_dnstap_files_from_dir, OneHotEncoding, Sequence};
use std::path::Path;

fn error2py(err: Error) -> PyErr {
    PyErr::new::<Exception, _>(format!("{}", err.display_causes()))
}

// Function name is module name
#[pymodinit]
fn pylib(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<PySequence>()?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    /// Load a dnstap file from disk and create a `Sequence` object
    #[pyfn(m, "load_file")]
    fn load_file(path: String) -> PyResult<PySequence> {
        let seq = Sequence::from_path(Path::new(&path)).map_err(error2py)?;
        Ok(PySequence::new(seq))
    }

    /// Load a whole folder of dnstap files
    #[pyfn(m, "load_folder")]
    fn load_folder(py: Python<'_>, path: String) -> PyResult<Vec<(String, Vec<PySequence>)>> {
        let seqs = py
            .allow_threads(|| load_all_dnstap_files_from_dir(Path::new(&path)))
            .map_err(error2py)?;
        Ok(seqs
            .into_iter()
            .map(|(domain, seqs)| {
                (
                    domain,
                    seqs.into_iter().map(|seq| PySequence::new(seq)).collect(),
                )
            })
            .collect())
    }

    Ok(())
}

/// Represents a sequence of DNS packets as measured on the wire
#[pyclass(name=Sequence)]
pub struct PySequence {
    sequence: Sequence,
}

impl PySequence {
    pub fn new(sequence: Sequence) -> PySequence {
        PySequence { sequence }
    }
}

#[pymethods]
impl PySequence {
    /// Create a new class of type `Sequence` by loading the dnstap file
    #[new]
    pub fn __new__(obj: &PyRawObject, path: String) -> PyResult<()> {
        let seq = Sequence::from_path(Path::new(&path)).map_err(error2py)?;
        obj.init(|_| PySequence::new(seq))
    }

    /// Returns a unique identifier for this sequence
    pub fn id(&self) -> PyResult<String> {
        Ok(self.sequence.id().to_string())
    }

    /// Calculate the distance between two sequences
    pub fn distance(&self, other: &PySequence) -> PyResult<usize> {
        Ok(self.sequence.distance(&other.sequence))
    }

    /// Try to classify the sequence, if it belongs to one of a couple of common categories
    pub fn classify(&self) -> PyResult<Option<&'static str>> {
        Ok(self.sequence.classify())
    }

    pub fn to_one_hot_encoding(&self) -> PyResult<Vec<OneHotEncoding>> {
        Ok(self.sequence.to_one_hot_encoding())
    }

    /// Returns the number of elements in this sequence
    pub fn len(&self) -> PyResult<usize> {
        Ok(self.sequence.len())
    }

    /// Returns the number of DNS messages inside this sequence
    pub fn message_count(&self) -> usize {
        self.sequence.message_count()
    }

    /// Returns the complexity score of this sequence
    pub fn complexity(&self) -> usize {
        self.sequence.complexity()
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
