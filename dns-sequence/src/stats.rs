// use super::*;
use csv::WriterBuilder;
use failure::{format_err, Error, ResultExt};
use misc_utils::fs::{file_open_write, WriteOptions};
use prettytable::{
    cell,
    format::{FormatBuilder, LinePosition, LineSeparator, TableFormat},
    row, Table,
};
use reverse_cum_sum;
use serde::Serialize;
use std::{
    collections::HashMap,
    fmt::{self, Display},
    fs::OpenOptions,
    hash::Hash,
    path::Path,
};
use string_cache::DefaultAtom as Atom;
use ClassificationResult;

const COLORS: &[&str] = &[
    "#349e35", "#98dd8b", "#df802e", "#feba7c", "#d33134", "#fe9897",
];

lazy_static! {
    /// A line separator made of light unicode table elements
    static ref UNICODE_LIGHT_SEP: LineSeparator = LineSeparator::new('─', '┼', '├', '┤');
    /// A line separator made of heavy unicode table elements
    static ref UNICODE_HEAVY_SEP: LineSeparator = LineSeparator::new('━', '┿', '┝', '┥');
    /// A line separator made of double unicode table elements
    static ref UNICODE_DOUBLE_SEP: LineSeparator = LineSeparator::new('═', '╪', '╞', '╡');

    static ref FORMAT_NO_BORDER_UNICODE: TableFormat = FormatBuilder::new()
        .padding(1, 1)
        // .separator(LinePosition::Intern, *UNICODE_LIGHT_SEP)
        .separator(LinePosition::Title, *UNICODE_DOUBLE_SEP)
        .column_separator('│')
        .build();
}

#[derive(Debug)]
pub(crate) struct StatsCollector<S: Eq + Hash = Atom> {
    data: HashMap<u8, StatsInternal<S>>,
}

#[derive(Debug)]
struct StatsCounter<S: Eq + Hash = Atom> {
    /// Counts pairs of `ClassificationResult` and if it is known problematic (bool).
    results: HashMap<(ClassificationResult, bool), usize>,
    /// Counts the problematic reasons
    reasons: HashMap<S, usize>,
}

#[derive(Debug)]
struct StatsInternal<S: Eq + Hash = Atom> {
    true_domain: HashMap<S, StatsCounter<S>>,
    mapped_domain: HashMap<S, StatsCounter<S>>,
    global: StatsCounter<S>,
}

impl<S: Eq + Hash> StatsCollector<S> {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn update(
        &mut self,
        k: u8,
        true_domain: S,
        mapped_domain: S,
        result: ClassificationResult,
        known_problems: Option<S>,
    ) where
        S: Clone,
    {
        let k_stats = self.data.entry(k).or_default();
        k_stats
            .true_domain
            .entry(true_domain)
            .or_default()
            .update(result, known_problems.clone());
        k_stats
            .mapped_domain
            .entry(mapped_domain)
            .or_default()
            .update(result, known_problems.clone());
        k_stats.global.update(result, known_problems);
    }

    pub fn dump_stats_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Error>
    where
        S: Serialize,
    {
        let wtr = file_open_write(
            path.as_ref(),
            WriteOptions::new().set_open_options(OpenOptions::new().create(true).truncate(true)),
        )
        .context("Cannot open writer for statistics.")?;
        let mut writer = WriterBuilder::new().has_headers(true).from_writer(wtr);

        #[derive(Serialize)]
        struct Out<'a, S> {
            k: u8,
            label: &'a S,
            corr: usize,
            corr_w_reason: usize,
            und: usize,
            und_w_reason: usize,
            wrong: usize,
            wrong_w_reason: usize,
            reasons: usize,
        };

        let mut ks: Vec<_> = self.data.keys().collect();
        ks.sort();
        for &k in ks {
            for (domain, stats) in &self.data[&k].true_domain {
                let out = Out {
                    k,
                    label: domain,
                    corr: stats
                        .results
                        .get(&(ClassificationResult::Correct, false))
                        .cloned()
                        .unwrap_or_default(),
                    corr_w_reason: stats
                        .results
                        .get(&(ClassificationResult::Correct, true))
                        .cloned()
                        .unwrap_or_default(),
                    und: stats
                        .results
                        .get(&(ClassificationResult::Undetermined, false))
                        .cloned()
                        .unwrap_or_default(),
                    und_w_reason: stats
                        .results
                        .get(&(ClassificationResult::Undetermined, true))
                        .cloned()
                        .unwrap_or_default(),
                    wrong: stats
                        .results
                        .get(&(ClassificationResult::Wrong, false))
                        .cloned()
                        .unwrap_or_default(),
                    wrong_w_reason: stats
                        .results
                        .get(&(ClassificationResult::Wrong, true))
                        .cloned()
                        .unwrap_or_default(),
                    reasons: stats.reasons.iter().map(|(_reason, count)| count).sum(),
                };

                writer
                    .serialize(&out)
                    .map_err(|err| format_err!("{}", err))?;
            }
        }

        Ok(())
    }

    pub fn plot(&self, output: impl AsRef<Path>) -> Result<(), Error>
    where
        S: Ord,
    {
        for k in self.data.keys() {
            let size = self.data[k].true_domain.len();
            let mut corr = Vec::with_capacity(size);
            let mut corr_w_reason = Vec::with_capacity(size);
            let mut und = Vec::with_capacity(size);
            let mut und_w_reason = Vec::with_capacity(size);
            let mut wrong = Vec::with_capacity(size);
            let mut wrong_w_reason = Vec::with_capacity(size);

            let mut data: Vec<_> = self.data[k].true_domain.iter().collect();
            data.sort_by_key(|x| x.0);
            for (_domain, stats) in data {
                corr.push(
                    stats
                        .results
                        .get(&(ClassificationResult::Correct, false))
                        .cloned()
                        .unwrap_or_default() as f64
                        + 0.1,
                );
                corr_w_reason.push(
                    stats
                        .results
                        .get(&(ClassificationResult::Correct, true))
                        .cloned()
                        .unwrap_or_default() as f64
                        + 0.1,
                );
                und.push(
                    stats
                        .results
                        .get(&(ClassificationResult::Undetermined, false))
                        .cloned()
                        .unwrap_or_default() as f64
                        + 0.1,
                );
                und_w_reason.push(
                    stats
                        .results
                        .get(&(ClassificationResult::Undetermined, true))
                        .cloned()
                        .unwrap_or_default() as f64
                        + 0.1,
                );
                wrong.push(
                    stats
                        .results
                        .get(&(ClassificationResult::Wrong, false))
                        .cloned()
                        .unwrap_or_default() as f64
                        + 0.1,
                );
                wrong_w_reason.push(
                    stats
                        .results
                        .get(&(ClassificationResult::Wrong, true))
                        .cloned()
                        .unwrap_or_default() as f64
                        + 0.1,
                );
            }

            let mut config = HashMap::new();
            config.insert("colors", &COLORS as &_);
            plot::percentage_stacked_area_chart(
                &[
                    ("Correct", corr),
                    ("Correct (wR)", corr_w_reason),
                    ("Undetermined", und),
                    ("Undetermined (wR)", und_w_reason),
                    ("Wrong", wrong),
                    ("Wrong (wR)", wrong_w_reason),
                ],
                output.as_ref().with_extension(format!("k{}.svg", k)),
                config,
            )?;
        }
        Ok(())
    }

    /// Count the number of domains with at least x correctly labelled domains, where x is the array index
    fn count_correct(&self) -> HashMap<u8, Vec<usize>> {
        self.data
            .iter()
            .map(|(&k, stats)| {
                // Count how many domains have x correctly labelled domains
                // x will be used as index into the vector
                // needs to store values from 0 to 10 (inclusive)
                let mut counts = vec![0; 11];
                stats
                    .true_domain
                    .iter()
                    .for_each(|(_domain, internal_stats)| {
                        let corrects = internal_stats
                            .results
                            .get(&(ClassificationResult::Correct, false))
                            .cloned()
                            .unwrap_or(0)
                            + internal_stats
                                .results
                                .get(&(ClassificationResult::Correct, true))
                                .cloned()
                                .unwrap_or(0);
                        counts[corrects] += 1;
                    });
                counts = reverse_cum_sum(&counts);
                (k, counts)
            })
            .collect()
    }
}

impl<S> Display for StatsCollector<S>
where
    S: Display + Eq + Hash,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use prettytable::Row;

        let mut keys: Vec<_> = self.data.keys().collect();
        let count_corrects = self.count_correct();
        keys.sort();

        let mut first = true;
        for k in keys {
            if !first {
                write!(f, "\n\n");
            }
            first = false;

            // key must exist, because we just got it from the HashMap
            let k_stats = &self.data[k];
            writeln!(f, "knn with k={}:", k)?;
            k_stats.global.fmt(f)?;

            writeln!(f, "\n#Domains with x correctly labelled traces:");
            let header = Row::new((0..=10).map(|c| cell!(bc->c)).collect());
            let counts = Row::new(count_corrects[k].iter().map(|c| cell!(r->c)).collect());
            let mut table = Table::init(vec![counts]);
            table.set_titles(header);
            table.set_format(*FORMAT_NO_BORDER_UNICODE);
            table.fmt(f)?;
        }
        Ok(())
    }
}

impl<S: Eq + Hash> Default for StatsInternal<S> {
    fn default() -> Self {
        Self {
            true_domain: HashMap::default(),
            mapped_domain: HashMap::default(),
            global: StatsCounter::default(),
        }
    }
}

impl<S> Display for StatsCounter<S>
where
    S: Display + Eq + Hash,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut rows = Vec::with_capacity(4);
        // build table rows
        for &res in &[
            (ClassificationResult::Correct),
            (ClassificationResult::Undetermined),
            (ClassificationResult::Wrong),
        ] {
            let wo_problems_count = self.results.get(&(res, false)).cloned().unwrap_or_default();
            let with_problems_count = self.results.get(&(res, true)).cloned().unwrap_or_default();
            rows.push(row!(
                l->res,
                r->wo_problems_count,
                r->with_problems_count,
                r->wo_problems_count + with_problems_count,
            ));
        }

        let mut table = Table::init(rows);
        table.set_titles(row!(
            bc->"Success",
            bc->"#",
            bc->"#With Prob.",
            bc->"Total",
        ));
        table.set_format(*FORMAT_NO_BORDER_UNICODE);
        table.fmt(f)?;
        Ok(())
    }
}

impl<S: Eq + Hash> Default for StatsCounter<S> {
    fn default() -> Self {
        Self {
            results: HashMap::default(),
            reasons: HashMap::default(),
        }
    }
}

impl<S: Eq + Hash> StatsCounter<S> {
    fn update(&mut self, result: ClassificationResult, known_problems: Option<S>) {
        *self
            .results
            .entry((result, known_problems.is_some()))
            .or_default() += 1;
        if let Some(reason) = known_problems {
            *self.reasons.entry(reason).or_default() += 1;
        }
    }
}

/// Fake implementation of the plot feature such that this binary can be build without python dependencies
///
/// Instead of plotting this simply pickles the input data, such
#[cfg(not(feature = "plot"))]
mod plot {
    use failure::Error;
    use misc_utils::fs::{file_open_write, WriteOptions};
    use serde_pickle;
    use std::{collections::HashMap, fs::OpenOptions, path::Path};

    pub fn percentage_stacked_area_chart<S: ::std::hash::BuildHasher>(
        data: &[(impl AsRef<str>, impl AsRef<[f64]>)],
        output: impl AsRef<Path>,
        config: HashMap<&str, &[&str], S>,
    ) -> Result<(), Error> {
        // The pickle data will have the following shape:
        // t.Tuple[
        //     # This is the data to plot. They will be plottet in order
        //     t.List[t.Tuple[
        //         str,  # part of the legend
        //         t.List[float]  # the data
        //     ]],
        //     # Additional configuration parameters
        //     t.Dict[
        //         str,
        //         t.List[str]
        //     ]
        // ]

        info!("Pickle plotting data");
        let path = output.as_ref().with_extension("pickle");

        let mut wtr = file_open_write(
            &path,
            WriteOptions::new().set_open_options(OpenOptions::new().create(true).truncate(true)),
        )?;
        let data: Vec<(&str, &[f64])> = data
            .iter()
            .map(|(label, value)| (label.as_ref(), value.as_ref()))
            .collect();
        serde_pickle::to_writer(&mut wtr, &(data, config), true)?;
        Ok(())
    }
}
