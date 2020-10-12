use anyhow::{Context as _, Error};
use chrome::ChromeDebuggerMessage;
use criterion::{criterion_group, criterion_main, Criterion};
use misc_utils::fs::file_open_read;
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

fn parse_file_old((rdr, path): (Box<dyn Read>, PathBuf)) -> Result<usize, Error> {
    let msgs: Vec<ChromeDebuggerMessage> = serde_json::from_reader(rdr)
        .with_context(|| format!("Error while deserializing '{}'", path.display()))?;
    Ok(msgs.len())
}

fn parse_file_new((mut rdr, path): (Box<dyn Read>, PathBuf)) -> Result<usize, Error> {
    // open file and parse it
    let mut content = String::with_capacity(1024 * 1024 * 25);
    rdr.read_to_string(&mut content)
        .with_context(|| format!("Error while reading '{}'", path.display()))?;
    let msgs: Vec<ChromeDebuggerMessage<&str>> = serde_json::from_str(&content)
        .with_context(|| format!("Error while deserializing '{}'", path.display()))?;
    Ok(msgs.len())
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("Parse File (reader)", |b| {
        b.iter_with_setup(mkreader, |init| parse_file_old(init).unwrap())
    });
    c.bench_function("Parse File (borrow)", |b| {
        b.iter_with_setup(mkreader, |init| parse_file_new(init).unwrap())
    });
}

fn mkreader() -> (Box<dyn Read>, PathBuf) {
    let path = Path::new("./benches/yy.com-7.json.xz").to_path_buf();
    (
        file_open_read(&path)
            .with_context(|| format!("Failed to read {}", path.display()))
            .unwrap(),
        path,
    )
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(Duration::new(120, 0));
    targets = criterion_benchmark
);
criterion_main!(benches);
