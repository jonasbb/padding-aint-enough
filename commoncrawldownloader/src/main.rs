//! Download [Common Crawl] URIs matching certain domains
//!
//! It only downloads URIs belonging to certain domains or arbitrary subdomains thereof.
//! Only URIs which resolve to a 200 status code will be returned.
//! This ensures that only "working" URIs are listed.
//!
//! [Common Crawl]: https://commoncrawl.org/

use aho_corasick::AhoCorasick;
use failure::{bail, Error};
use flate2::read::MultiGzDecoder;
use misc_utils::fs;
use serde::Deserialize;
use std::{
    borrow::{Borrow, Cow},
    char,
    collections::BTreeMap,
    io::{BufRead, BufReader, Read},
    ops::Bound,
};
use structopt::StructOpt;
use url::Url;

static BASEURL: &str = "https://commoncrawl.s3.amazonaws.com/";

#[derive(Deserialize)]
struct UrlContainer<'a> {
    url: Cow<'a, str>,
    status: &'a str,
}

/// Download Common Crawl URIs matching certain domains
///
/// It only downloads URIs belonging to certain domains or arbitrary subdomains thereof.
/// Only URIs which resolve to a 200 status code will be returned.
/// This ensures that only "working" URIs are listed.
#[derive(StructOpt)]
#[structopt(global_settings(&[
    structopt::clap::AppSettings::ColoredHelp,
    structopt::clap::AppSettings::VersionlessSubcommands
]))]
struct CliArgs {
    /// List of Domains we want to extract URIs for
    #[structopt(value_name = "DOMAIN")]
    domains: Vec<String>,
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

    let response = reqwest::get(
        "https://commoncrawl.s3.amazonaws.com/crawl-data/CC-MAIN-2019-47/cc-index.paths.gz",
    )?;
    if !response.status().is_success() {
        bail!("Error while fetching paths: {}", response.status());
    }
    let mut output = String::with_capacity(1024 * 1024);
    MultiGzDecoder::new(response).read_to_string(&mut output)?;

    // Search for the index file
    let mut base_file = None;
    let mut index_file = None;
    for line in output.lines() {
        if line.ends_with("cdx-00000.gz") {
            base_file = Some(line);
        } else if line.ends_with("cluster.idx") {
            index_file = Some(line);
        }
    }
    let base_file = base_file
        .expect("There must be cdx-00000.gz file.")
        .to_string();
    let index_file = index_file
        .expect("There must be a cluster.idx file.")
        .to_string();

    let mut url = BASEURL.to_string();
    url += &index_file;
    let mut response = reqwest::get(&url)?;
    if !response.status().is_success() {
        bail!("Error while fetching cluster.idx: {}", response.status());
    }
    output.clear();
    response.read_to_string(&mut output)?;
    // let output = fs::read_to_string("/home/jbushart/Downloads/cluster.idx")?;

    // Maps from the SURT domain to which common crawl file the entry is contained in
    let mut index_map: BTreeMap<String, u16> = BTreeMap::default();

    for line in output.lines() {
        let mut parts = line.split('\t');
        // Contains SURT and timestamp, e.g.: 0,0,1)/ 20191118114721
        // http://crawler.archive.org/articles/user_manual/glossary.html#surt
        let part1 = parts.next().expect("Failed getting SURT part in index");
        // Contains the data file, e.g.: cdx-00159.gz
        let data_file = parts.next().expect("Failed to get data file from index");
        let domain = part1.split(')').next().expect("Failed to extract SURT");
        let data_file_number: u16 = data_file[4..9]
            .parse()
            .expect("Failed to parse data file id");
        index_map.insert(domain.to_string(), data_file_number);
    }

    // Map from the common crawl data file to which of the domains we care about are included in this file
    let commoncrawl_file_to_domain: BTreeMap<u16, Vec<String>> = cli_args
        .domains
        .iter()
        .flat_map(|domain| {
            let (start, end) = find_prev_and_next_elements(&index_map, &domain_to_surt(domain));
            (start..=end).map(move |i| (i, domain))
        })
        .fold(BTreeMap::new(), |mut accu, (index, domain)| {
            accu.entry(index).or_default().push(domain.to_string());
            accu
        });
    // println!("{:#?}", commoncrawl_file_to_domain);

    for (idx, domains) in commoncrawl_file_to_domain.into_iter() {
        let mut url = BASEURL.to_string();
        url += &base_file;
        let url = url.replace("cdx-00000", &format!("cdx-{:0>5}", idx));
        println!("Download {}\n  to search for domains: {:?}", url, domains);

        let response = reqwest::get(&url)?;
        if !response.status().is_success() {
            bail!(
                "Error while fetching cdx-{:0>5}.gz: {}",
                idx,
                response.status()
            );
        }
        let mut content = BufReader::new(MultiGzDecoder::new(response));

        let ac = AhoCorasick::new_auto_configured(&domains);
        let mut matching_urls = String::new();

        let mut line = String::new();
        while {
            line.clear();
            content
                .read_line(&mut line)
                .expect("Failed to read data file line")
                > 0
        } {
            let json = line
                .splitn(3, ' ')
                .nth(2)
                .expect("Failed to extract the JSON part of the data file");
            let UrlContainer { url, status } =
                serde_json::from_str(json).expect("Failed to parse the JSON");
            if status != "200" {
                continue;
            }
            // Quick matcher to search if the listed domains occur anywhere in the URL
            if !ac.is_match(url.as_bytes()) {
                continue;
            }

            // Properly parse the URL and ensure the domain matches the host part and not anywhere else
            if url_has_domain_or_subdomain_of(&url, &*domains) {
                matching_urls.push_str(&url);
                matching_urls.push('\n');
            }
        }
        fs::write(&format!("urls-{:0>5}.txt.xz", idx), matching_urls)?;
    }

    Ok(())
}

fn find_prev_and_next_elements<'a, K>(map: &'a BTreeMap<K, u16>, domain: &str) -> (u16, u16)
where
    K: Ord + Borrow<str>,
    K: std::fmt::Debug,
{
    // Convert the SURT domain into two artificial entries which are strictly before and after the target domain
    let mut chars: Vec<_> = domain.chars().collect();
    let last_char = chars.len() - 1;
    // assert!(chars[last_char] != 'a' && chars[last_char] != 'z');
    chars[last_char] = char::from_u32(u32::from(chars[last_char]) - 1)
        .expect("The arithmetic on these ASCII chars should lead to other valid chars.");
    let prev_domain: String = chars.iter().collect();
    chars[last_char] = char::from_u32(u32::from(chars[last_char]) + 2)
        .expect("The arithmetic on these ASCII chars should lead to other valid chars.");
    let next_domain: String = chars.iter().collect();

    // Lookup the previous and next entry of needle and extract the index number.
    // The previous number is always bounded by 0.
    let prev = map
        .range((Bound::Unbounded, Bound::Excluded(next_domain.as_ref())))
        .map(|(_key, &index)| index)
        .next_back()
        .unwrap_or(0);
    // The next value is bounded by the largest index in the map
    let next = map
        .range((Bound::Excluded(prev_domain.as_ref()), Bound::Unbounded))
        .map(|(_key, &index)| index)
        .next()
        .unwrap_or_else(|| {
            *map.iter()
                .next_back()
                .expect("The BTreeMap must contain at least one element")
                .1
        });

    (prev, next)
}

fn domain_to_surt(domain: &str) -> String {
    domain
        .rsplit('.')
        .fold(String::with_capacity(domain.len()), |mut res, part| {
            if !res.is_empty() {
                res.push(',');
            }
            res.push_str(part);
            res
        })
}

#[test]
fn test_domain_to_surt() {
    assert_eq!("com,google", domain_to_surt("google.com"));
    assert_eq!("de", domain_to_surt("de"));
    assert_eq!("4,3,2,1", domain_to_surt("1.2.3.4"));
    assert_eq!(
        "parts,many,has,domain,this",
        domain_to_surt("this.domain.has.many.parts")
    );
}

fn url_has_domain_or_subdomain_of<S>(url: &str, domains: &[S]) -> bool
where
    S: AsRef<str>,
{
    let url_parsed = Url::parse(&url).expect("Could not parse the URL");
    for domain in domains {
        let domain = domain.as_ref();
        let host_str = url_parsed.host_str().expect("The URL has not host part");
        // Offset directly before the host of the domain starts
        let offset_before_host = host_str.len() - domain.len() - 1;
        if host_str == domain
            || (host_str.ends_with(domain)
                && &host_str[offset_before_host..=offset_before_host] == ".")
        {
            return true;
        }
    }
    false
}

#[test]
fn test_url_has_domain_or_subdomain_of() {
    assert!(url_has_domain_or_subdomain_of(
        "http://amazon.com",
        &["amazon.com"]
    ));
    assert!(url_has_domain_or_subdomain_of(
        "https://whispercast.amazon.com/robots.txt",
        &["amazon.com"]
    ));
    assert!(url_has_domain_or_subdomain_of(
        "https://www.amazon.com/Dr-Helen-Klus/e/B078Q9F7FV",
        &["amazon.com"]
    ));
    assert!(url_has_domain_or_subdomain_of(
        "https://aws.amazon.com/blogs/startups/tag/boston/",
        &["amazon.com"]
    ));

    assert!(url_has_domain_or_subdomain_of(
        "http://www.cnn.com",
        &["cnn.com"]
    ));

    // These should NOT match
    assert!(!url_has_domain_or_subdomain_of(
        "https://amazoncomamazon.com/?cat=1",
        &["amazon.com"]
    ));
}
