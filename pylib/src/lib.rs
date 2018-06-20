#![feature(proc_macro, specialization, const_fn)]

extern crate pyo3;

use pyo3::prelude::*;

use pyo3::py::class as pyclass;
use pyo3::py::methods as pymethods;
use pyo3::py::modinit as pymodinit;
use pyo3::py::proto as pyproto;

// Add bindings to the generated python module
// N.B: names: "librust2py" must be the name of the `.so` or `.pyd` file
/// This module is implemented in Rust.
#[pymodinit(_pylib)]
fn init_mod(py: Python, m: &PyModule) -> PyResult<()> {
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

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct Sequence {
    seq: Vec<SequenceElement>,
}

impl Sequence {
    pub fn new() -> Sequence {
        Sequence {
            seq: vec![
                SequenceElement::Size(1),
                SequenceElement::Gap(5),
                SequenceElement::Size(3),
            ],
        }
    }
}

// #[pymethods]
impl Sequence {
    // #[new]
    // fn __new__(obj: &PyRawObject, num: usize) -> PyResult<()> {
    //     obj.init(|token| MyClass { num, debug: false, token })
    // }
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

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum SequenceElement {
    Size(u8),
    Gap(u8),
}
