#[macro_use]
extern crate failure;
extern crate serde_pickle;
extern crate tempfile;

use failure::Error;
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};
use tempfile::NamedTempFile;

/// Plot a Percentage Stacked Area Chart
///
/// https://python-graph-gallery.com/255-percentage-stacked-area-chart/
pub fn percentage_stacked_area_chart(
    data: &[(impl AsRef<str>, impl AsRef<[f64]>)],
    output: impl AsRef<Path>,
) -> Result<(), Error> {
    let mut file = NamedTempFile::new()?;
    // Convert the data into &str and &[f64]
    let data: Vec<_> = data
        .iter()
        .map(|(label, value)| (label.as_ref(), value.as_ref()))
        .collect();
    serde_pickle::to_writer(&mut file, &data, true).unwrap();

    let mut child = Command::new("python3")
        .arg("-")
        .arg(file.path())
        .arg(output.as_ref())
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // FIXME remove after NLL
    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        stdin.write_all(include_bytes!("./percentage_stacked_area_chart.py"))?;
    }
    let output = child.wait_with_output().expect("Python3 did not start");

    if !output.status.success() {
        bail!(
            "Plotting failed with error message: '{}'",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[test]
fn test_percentage_stacked_area_chart() {
    let data = vec![("A", vec![1., 4., 6., 8.]), ("B", vec![9., 8., 6., 4.])];
    percentage_stacked_area_chart(&data, "/dev/null").unwrap();
}
