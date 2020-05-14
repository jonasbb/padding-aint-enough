use failure::{format_err, Error, ResultExt};
use pyo3::{types::PyDict, PyErr, PyResult, Python};
use sequences::dnstap::Query;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(global_settings(&[
    structopt::clap::AppSettings::ColoredHelp,
    structopt::clap::AppSettings::VersionlessSubcommands
]))]
struct CliArgs {
    /// Place files into output directory instead of next to the dnstap file
    #[structopt(long, value_name = "DIR")]
    output: Option<PathBuf>,
    /// Only produce a single output file with all the input files merged together
    #[structopt(short = "s", long)]
    single_file: bool,
    /// Width of the output graphic in inches
    #[structopt(short, long, default_value = "10")]
    width: u32,
    /// Height of the output graphic in inches
    #[structopt(short, long, default_value = "6")]
    height: u32,
    /// List of DNSTAP files to process and plot
    #[structopt(value_name = "DNSTAP FILES")]
    dnstap_files: Vec<PathBuf>,
}
fn main() {
    use std::io::{self, Write};

    if let Err(err) = run() {
        let stderr = io::stderr();
        let mut out = stderr.lock();
        // cannot handle a write error here, we are already in the outermost layer
        let _ = writeln!(out, "An error occured:");
        for fail in err.iter_chain() {
            let _ = writeln!(out, "  {}", fail);
        }
        let _ = writeln!(out, "{}", err.backtrace());
        std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    // generic setup
    env_logger::init();
    let cli_args = CliArgs::from_args();

    if cli_args.dnstap_files.is_empty() {
        return Ok(());
    }

    let outdir = &cli_args.output;
    let width = cli_args.width;
    let height = cli_args.height;

    let querysets: Vec<(Vec<Query>, PathBuf)> = cli_args
        .dnstap_files
        .into_iter()
        .map(|file| {
            let queries = sequences::dnstap::load_matching_query_responses_from_dnstap(&file)
                .with_context(|_| format_err!("Cannot process file {}", file.display()))?;
            let outfile = if let Some(outdir) = outdir {
                outdir.join(file.file_name().unwrap()).with_extension("svg")
            } else {
                file.with_extension("svg")
            };

            Ok((queries, outfile))
        })
        .collect::<Result<_, Error>>()?;

    if cli_args.single_file {
        let outfile = querysets[0].1.clone();
        let querysets = querysets
            .into_iter()
            .map(|(qs, fname)| (qs, stem_file(&fname)))
            .collect();
        plot_queries(querysets, &outfile, width, height).map_err(pyerr2error)?;
    } else {
        querysets
            .into_iter()
            .map(|(queries, outfile)| {
                plot_queries(
                    vec![(queries, stem_file(&outfile))],
                    &outfile,
                    width,
                    height,
                )
                .map_err(pyerr2error)
            })
            .collect::<Result<(), Error>>()?;
    }

    Ok(())
}

fn plot_queries(
    queries: Vec<(Vec<Query>, String)>,
    output_filename: &Path,
    width: u32,
    height: u32,
) -> PyResult<()> {
    let gil = Python::acquire_gil();
    let py = gil.python();

    let main_module = py.import("__main__").unwrap();
    let globals = main_module.dict();
    globals.set_item("queries", serde_json::to_string_pretty(&queries).unwrap())?;
    globals.set_item("image_width", width)?;
    globals.set_item("image_height", height)?;
    globals.set_item("output_filename", output_filename.to_string_lossy())?;
    py.run(include_str!("plot.py"), Some(globals), None)?;
    Ok(())
}

/// Convert a [`PyErr`] into an [`Error`]
fn pyerr2error(err: PyErr) -> Error {
    let gil = Python::acquire_gil();
    let py = gil.python();

    err.clone_ref(py).print_and_set_sys_last_vars(py);
    let locals = PyDict::new(py);
    let _ = locals.set_item("err", err);
    let err_msg: String = py
        .eval("repr(err)", None, Some(&locals))
        .unwrap()
        .extract()
        .unwrap();
    format_err!("{}", err_msg)
}

/// Return the filename without the extension
fn stem_file(file: &Path) -> String {
    file.file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "<unknown>".into())
}
