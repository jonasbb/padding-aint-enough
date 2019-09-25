#![feature(specialization)]
#![allow(clippy::all)]

use encrypted_dns::ErrorExt;
use failure::Error;
use pyo3::{
    self, basic::CompareOp, exceptions::Exception, prelude::*, types::PyType, PyObjectProtocol,
};
use sequences::{
    distance_cost_info::CostTracker, load_all_files_with_extension_from_dir_with_config,
    LoadSequenceConfig, OneHotEncoding, Sequence,
};
use std::{collections::BTreeMap, ffi::OsStr, path::Path};

fn error2py(err: Error) -> PyErr {
    PyErr::new::<Exception, _>(format!("{}", err.display_causes()))
}

// Function name is module name
#[pymodule]
fn pylib(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<PySequence>()?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    /// load_file(path, /)
    /// --
    ///
    /// Load a dnstap file from disk and create a `Sequence` object
    #[pyfn(m, "load_file")]
    fn load_file(path: String) -> PyResult<PySequence> {
        let seq = Sequence::from_path(Path::new(&path)).map_err(error2py)?;
        Ok(seq.into())
    }

    /// load_folder(path, extension = "dnstap", /)
    /// --
    ///
    /// Load a whole folder of files with given `extension`.
    /// `extension` defaults to the value "dnstap".
    #[pyfn(m, "load_folder")]
    fn load_folder(
        py: Python<'_>,
        path: String,
        extension: Option<String>,
    ) -> PyResult<Vec<(String, Vec<PySequence>)>> {
        let extension = extension.unwrap_or_else(|| "dnstap".to_string());
        let seqs = py
            .allow_threads(|| {
                load_all_files_with_extension_from_dir_with_config(
                    Path::new(&path),
                    &OsStr::new(&extension),
                    LoadSequenceConfig::default(),
                )
            })
            .map_err(error2py)?;
        Ok(seqs
            .into_iter()
            .map(|(domain, seqs)| (domain, seqs.into_iter().map(Into::into).collect()))
            .collect())
    }

    Ok(())
}

/// Represents a sequence of DNS packets as measured on the wire
#[pyclass(name=Sequence)]
pub struct PySequence {
    sequence: Sequence,
}

#[pymethods]
impl PySequence {
    /// Create a new class of type `Sequence` by loading the dnstap file
    #[classmethod]
    pub fn from_path(_cls: &PyType, path: String) -> PyResult<PySequence> {
        let seq = Sequence::from_path(Path::new(&path)).map_err(error2py)?;
        Ok(seq.into())
    }

    /// Returns a unique identifier for this sequence
    pub fn id(&self) -> PyResult<String> {
        Ok(self.sequence.id().to_string())
    }

    /// Calculate the distance between two sequences
    pub fn distance(&self, other: &PySequence) -> PyResult<usize> {
        Ok(self.sequence.distance(&other.sequence))
    }

    /// Calculate the distance between two sequences
    pub fn distance_with_details(
        &self,
        other: &PySequence,
    ) -> PyResult<(usize, BTreeMap<String, usize>)> {
        let (cost, cost_info) = self.sequence.distance_with_limit::<CostTracker>(
            &other.sequence,
            usize::max_value(),
            false,
            false,
        );
        Ok((cost, cost_info.as_btreemap()))
    }

    /// Try to classify the sequence, if it belongs to one of a couple of common categories
    pub fn classify(&self) -> PyResult<Option<&'static str>> {
        Ok(self.sequence.classify())
    }

    /// Convert the Sequence into a List of Lists suitable for ML.
    pub fn to_one_hot_encoding(&self) -> PyResult<Vec<OneHotEncoding>> {
        Ok(self.sequence.to_one_hot_encoding())
    }

    /// Convert the Sequence into a List of Lists suitable for ML.
    pub fn to_vector_encoding(&self) -> PyResult<Vec<(u16, u16)>> {
        Ok(self.sequence.to_vector_encoding())
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

    /// Returns a [`String`] with the JSON representation of this Sequence
    pub fn to_json(&self) -> PyResult<String> {
        self.sequence.to_json().map_err(error2py)
    }
}

impl From<Sequence> for PySequence {
    fn from(other: Sequence) -> Self {
        PySequence { sequence: other }
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
