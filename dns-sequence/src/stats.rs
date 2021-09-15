use crate::reverse_cum_sum;
use anyhow::{anyhow, Context as _, Error};
use csv::WriterBuilder;
use misc_utils::fs::file_write;
use once_cell::sync::Lazy;
use prettytable::{
    cell,
    format::{FormatBuilder, LinePosition, LineSeparator, TableFormat},
    row, Table,
};
use sequences::knn::ClassificationResultQuality;
use serde::Serialize;
use std::{
    collections::HashMap,
    fmt::{self, Display},
    hash::Hash,
    path::Path,
};
use string_cache::DefaultAtom as Atom;

const COLORS: &[&str] = &[
    "#2ca02c", "#98df8a", "#bcbd22", "#dbdb8d", "#1f77b4", "#aec7e8", "#ff7f0e", "#ffbb78",
    "#9467bd", "#c5b0d5", "#d62728", "#ff9896",
];

/// A line separator made of light unicode table elements
#[allow(dead_code)]
static UNICODE_LIGHT_SEP: Lazy<LineSeparator> =
    Lazy::new(|| LineSeparator::new('─', '┼', '├', '┤'));
/// A line separator made of heavy unicode table elements
#[allow(dead_code)]
static UNICODE_HEAVY_SEP: Lazy<LineSeparator> =
    Lazy::new(|| LineSeparator::new('━', '┿', '┝', '┥'));
/// A line separator made of double unicode table elements
#[allow(dead_code)]
static UNICODE_DOUBLE_SEP: Lazy<LineSeparator> =
    Lazy::new(|| LineSeparator::new('═', '╪', '╞', '╡'));
static FORMAT_NO_BORDER_UNICODE: Lazy<TableFormat> = Lazy::new(|| {
    FormatBuilder::new()
        .padding(1, 1)
        // .separator(LinePosition::Intern, *UNICODE_LIGHT_SEP)
        .separator(LinePosition::Title, *UNICODE_DOUBLE_SEP)
        .column_separator('│')
        .build()
});

#[derive(Debug)]
pub(crate) struct StatsCollector<S: Eq + Hash = Atom> {
    data: HashMap<u8, StatsInternal<S>>,
}

#[derive(Debug)]
struct StatsCounter<S: Eq + Hash = Atom> {
    /// Counts pairs of `ClassificationResultQuality` and if it is known problematic (bool).
    results: HashMap<(ClassificationResultQuality, bool), usize>,
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
        result: ClassificationResultQuality,
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
        let wtr = file_write(path.as_ref())
            .create(true)
            .truncate()
            .context("Cannot open writer for statistics.")?;
        let mut writer = WriterBuilder::new().has_headers(true).from_writer(wtr);

        #[derive(Serialize)]
        struct Out<'a, S> {
            k: u8,
            label: &'a S,
            no_result: usize,
            no_result_w_reason: usize,
            wrong: usize,
            wrong_w_reason: usize,
            contains: usize,
            contains_w_reason: usize,
            plurality_and_dist: usize,
            plurality_and_dist_w_reason: usize,
            plurality: usize,
            plurality_w_reason: usize,
            majority: usize,
            majority_w_reason: usize,
            exact: usize,
            exact_w_reason: usize,
            reasons: usize,
        }

        let mut ks: Vec<_> = self.data.keys().collect();
        ks.sort();
        for &k in ks {
            for (domain, stats) in &self.data[&k].true_domain {
                let out = Out {
                    k,
                    label: domain,
                    no_result: stats
                        .results
                        .get(&(ClassificationResultQuality::NoResult, false))
                        .cloned()
                        .unwrap_or_default(),
                    no_result_w_reason: stats
                        .results
                        .get(&(ClassificationResultQuality::NoResult, true))
                        .cloned()
                        .unwrap_or_default(),
                    wrong: stats
                        .results
                        .get(&(ClassificationResultQuality::Wrong, false))
                        .cloned()
                        .unwrap_or_default(),
                    wrong_w_reason: stats
                        .results
                        .get(&(ClassificationResultQuality::Wrong, true))
                        .cloned()
                        .unwrap_or_default(),
                    contains: stats
                        .results
                        .get(&(ClassificationResultQuality::Contains, false))
                        .cloned()
                        .unwrap_or_default(),
                    contains_w_reason: stats
                        .results
                        .get(&(ClassificationResultQuality::Contains, true))
                        .cloned()
                        .unwrap_or_default(),
                    plurality_and_dist: stats
                        .results
                        .get(&(ClassificationResultQuality::PluralityThenMinDist, false))
                        .cloned()
                        .unwrap_or_default(),
                    plurality_and_dist_w_reason: stats
                        .results
                        .get(&(ClassificationResultQuality::PluralityThenMinDist, true))
                        .cloned()
                        .unwrap_or_default(),
                    plurality: stats
                        .results
                        .get(&(ClassificationResultQuality::Plurality, false))
                        .cloned()
                        .unwrap_or_default(),
                    plurality_w_reason: stats
                        .results
                        .get(&(ClassificationResultQuality::Plurality, true))
                        .cloned()
                        .unwrap_or_default(),
                    majority: stats
                        .results
                        .get(&(ClassificationResultQuality::Majority, false))
                        .cloned()
                        .unwrap_or_default(),
                    majority_w_reason: stats
                        .results
                        .get(&(ClassificationResultQuality::Majority, true))
                        .cloned()
                        .unwrap_or_default(),
                    exact: stats
                        .results
                        .get(&(ClassificationResultQuality::Exact, false))
                        .cloned()
                        .unwrap_or_default(),
                    exact_w_reason: stats
                        .results
                        .get(&(ClassificationResultQuality::Exact, true))
                        .cloned()
                        .unwrap_or_default(),
                    reasons: stats.reasons.iter().map(|(_reason, count)| count).sum(),
                };

                writer.serialize(&out).map_err(|err| anyhow!("{}", err))?;
            }
        }

        Ok(())
    }

    pub fn plot(&self, output: impl AsRef<Path>) -> Result<(), Error>
    where
        S: Ord,
    {
        for k in self.data.keys() {
            let mut plot_data: HashMap<(ClassificationResultQuality, bool), Vec<f64>> =
                HashMap::default();

            let mut data: Vec<_> = self.data[k].true_domain.iter().collect();
            data.sort_by_key(|x| x.0);
            for (_domain, stats) in data {
                for quality in ClassificationResultQuality::iter_variants() {
                    for &with_problems in &[false, true] {
                        let entry = plot_data.entry((quality, with_problems)).or_default();
                        entry.push(
                            stats
                                .results
                                .get(&(quality, with_problems))
                                .cloned()
                                .unwrap_or_default() as f64
                                + 0.1,
                        );
                    }
                }
            }

            let plot_data = &plot_data;
            let tmp: Vec<(String, &Vec<f64>)> = ClassificationResultQuality::iter_variants()
                .rev()
                .flat_map(|quality| {
                    [false, true].iter().cloned().map(move |with_problems| {
                        let mut label = quality.to_string();
                        if with_problems {
                            label.push_str(" (wR)");
                        }
                        let datapoints = &plot_data[&(quality, with_problems)];
                        (label, datapoints)
                    })
                })
                .collect();

            let mut config = HashMap::new();
            config.insert("colors", COLORS as &_);
            plot::percentage_stacked_area_chart(
                &tmp,
                output.as_ref().with_extension(format!("k{}.svg", k)),
                config,
            )?;
        }
        Ok(())
    }

    /// Count the number of domains with at least x correctly labelled domains, where x is the array index
    fn count_correct(&self) -> HashMap<u8, HashMap<ClassificationResultQuality, Vec<usize>>> {
        self.data
            .iter()
            .map(|(&k, stats)| {
                let res: HashMap<ClassificationResultQuality, Vec<_>> =
                    ClassificationResultQuality::iter_variants()
                        .map(|quality| {
                            // Count how many domains have x domains with a classification result of quality
                            // `quality` or higher.
                            // x will be used as index into the vector
                            // needs to store values from 0 to 10 (inclusive)
                            let mut counts = vec![0; 11];
                            stats
                                .true_domain
                                .iter()
                                .for_each(|(_domain, internal_stats)| {
                                    let mut corrects = 0;
                                    for higher_q in ClassificationResultQuality::iter_variants()
                                        .filter(|&other_q| other_q >= quality)
                                    {
                                        corrects += internal_stats
                                            .results
                                            .get(&(higher_q, false))
                                            .cloned()
                                            .unwrap_or(0)
                                            + internal_stats
                                                .results
                                                .get(&(higher_q, true))
                                                .cloned()
                                                .unwrap_or(0);
                                    }
                                    counts[corrects] += 1;
                                });
                            (quality, counts)
                        })
                        .collect();
                (k, res)
            })
            .collect()
    }
}

impl<S> Display for StatsCollector<S>
where
    S: Display + Eq + Hash,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use prettytable::Row;

        let mut keys: Vec<_> = self.data.keys().collect();
        let count_corrects = self.count_correct();
        keys.sort();

        let mut first = true;
        for k in keys {
            if !first {
                write!(f, "\n\n")?;
            }
            first = false;

            // key must exist, because we just got it from the HashMap
            let k_stats = &self.data[k];
            writeln!(f, "knn with k={}:", k)?;
            k_stats.global.fmt(f)?;

            writeln!(
                f,
                "\n#Domains with at least x classification results of quality or higher:"
            )?;
            let header = Row::new(
                Some(cell!(bc->"Method"))
                    .into_iter()
                    .chain((0..=10).map(|c| cell!(bc->c)))
                    .collect(),
            );
            let tmp = &count_corrects[k];

            // For each quality level, we want to count the number of classifications with equal or better quality
            let counts: Vec<_> = ClassificationResultQuality::iter_variants()
                // skip some qualitys, as this does not match the semantics of the rest
                .filter(|&q| q != ClassificationResultQuality::NoResult)
                .filter(|&q| q != ClassificationResultQuality::Wrong)
                .map(|quality| {
                    let mut num_class = tmp[&quality].clone();

                    num_class = reverse_cum_sum(&num_class);
                    Row::new(
                        Some(cell!(l->quality))
                            .into_iter()
                            .chain(num_class.into_iter().map(|c| cell!(r->c)))
                            .collect(),
                    )
                })
                .collect();
            let mut table = Table::init(counts);
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut rows = Vec::with_capacity(4);
        // build table rows
        for res in ClassificationResultQuality::iter_variants().rev() {
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
            bc->"Quality",
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
    fn update(&mut self, result: ClassificationResultQuality, known_problems: Option<S>) {
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
/// Instead of plotting this simply dumps the plotting data as JSON
mod plot {
    use anyhow::Error;
    use log::info;
    use misc_utils::fs::file_write;
    use std::{collections::HashMap, path::Path};

    pub fn percentage_stacked_area_chart<S: ::std::hash::BuildHasher>(
        data: &[(impl AsRef<str>, impl AsRef<[f64]>)],
        output: impl AsRef<Path>,
        config: HashMap<&str, &[&str], S>,
    ) -> Result<(), Error> {
        // The JSON data will have the following shape:
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

        info!("Dump json of plotting data");
        let path = output.as_ref().with_extension("json");

        let mut wtr = file_write(&path).create(true).truncate()?;
        let data: Vec<(&str, &[f64])> = data
            .iter()
            .map(|(label, value)| (label.as_ref(), value.as_ref()))
            .collect();
        serde_json::to_writer(&mut wtr, &(data, config))?;
        Ok(())
    }
}
