#![feature(specialization)]

extern crate failure;
extern crate pyo3;
#[cfg(test)]
extern crate tempfile;

use failure::Error;
use pyo3::{prelude::*, types::PyDict};
use std::{collections::HashMap, path::Path};

fn pyerr_to_error(py: Python, pyerr: &PyErr) -> Error {
    let err: String = pyerr
        .into_object(py)
        .call_method0(py, "__repr__")
        .and_then(|obj| obj.extract(py))
        .unwrap();
    let mut traceback: Option<Vec<String>> = None;
    if let Some(ref tb) = pyerr.ptraceback {
        let traceback_mod = py.import("traceback").unwrap();
        traceback = traceback_mod
            .call1("format_tb", (tb,))
            .and_then(|obj| obj.extract::<Vec<String>>())
            .ok();
    }

    let msg: Vec<String> = Some(err)
        .into_iter()
        .chain(traceback.unwrap_or_else(|| vec![]))
        .collect();
    failure::err_msg(msg.join("\n"))
}

/// Plot a Percentage Stacked Area Chart
///
/// https://python-graph-gallery.com/255-percentage-stacked-area-chart/
pub fn percentage_stacked_area_chart<S: ::std::hash::BuildHasher>(
    data: &[(impl AsRef<str>, impl AsRef<[f64]>)],
    output: impl AsRef<Path>,
    config: HashMap<&str, &dyn ToPyObject, S>,
) -> Result<(), Error> {
    let python_code = include_str!("./percentage_stacked_area_chart.py");
    let data: Vec<_> = data
        .iter()
        .map(|(label, value)| (label.as_ref(), value.as_ref()))
        .collect();

    let gil = Python::acquire_gil();
    let py = gil.python();
    let locals = PyDict::new(py);
    locals
        .set_item("rawdata", data)
        .map_err(|pyerr| pyerr_to_error(py, &pyerr))?;
    locals
        .set_item("rawimgpath", output.as_ref().to_string_lossy())
        .map_err(|pyerr| pyerr_to_error(py, &pyerr))?;
    if !config.is_empty() {
        locals
            .set_item("config", config)
            .map_err(|pyerr| pyerr_to_error(py, &pyerr))?;
    }
    py.run(python_code, None, Some(&locals))
        .map_err(|pyerr| pyerr_to_error(py, &pyerr))?;

    Ok(())
}

// If errors like `main thread is not in main loop` occur you must specify the matplotlib backend
// to be something thread-safe, like `Agg`.
// Create the file `~/.config/matplotlib/matplotlibrc` and write
// ```
// backend: Agg
// ```

// #[test]
// fn test_percentage_stacked_area_chart() {
//     use tempfile::NamedTempFile;

//     let data = vec![("A", vec![1., 4., 6., 8.]), ("B", vec![9., 8., 6., 4.])];
//     let file = NamedTempFile::new().unwrap();
//     let res = percentage_stacked_area_chart(&data, file.path(), HashMap::new());
//     // Cleanup temp file
//     drop(file);
//     if let Err(err) = res {
//         panic!("{:#}", err);
//     }
// }

#[test]
fn test_percentage_stacked_area_chart_with_colors() {
    use tempfile::NamedTempFile;

    let data = vec![("A", vec![1., 4., 6., 8.]), ("B", vec![9., 8., 6., 4.])];
    let file = NamedTempFile::new().unwrap();
    let colors = &["#ff0000", "#00ff00"] as &[&str];
    let mut config = HashMap::new();
    config.insert("colors", &colors as &ToPyObject);
    let res = percentage_stacked_area_chart(&data, file.path(), config);
    // leanup temp file
    drop(file);
    if let Err(err) = res {
        panic!("{:#}", err);
    }
}
