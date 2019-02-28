use extract_sequence::{extract_tls_records, filter_tls_records};
use failure::Error;

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

    let mut records = extract_tls_records("./tests/data/CF-constant-rate-400ms-2packets.pcap")?;
    records = filter_tls_records(records);

    // println!(
    //     "{}",
    //     ron::ser::to_string_pretty(
    //         &records,
    //         ron::ser::PrettyConfig {
    //             enumerate_arrays: true,
    //             depth_limit: 3,
    //             ..Default::default()
    //         }
    //     )
    //     .unwrap()
    // );

    for r in records {
        println!("{:?}", r);
    }

    Ok(())
}
