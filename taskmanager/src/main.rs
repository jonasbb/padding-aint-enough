extern crate chrome;
extern crate encrypted_dns;
extern crate env_logger;
extern crate failure;
extern crate lazy_static;
extern crate log;
extern crate misc_utils;
extern crate openssl; // Needed for cross compilation
extern crate sequences;
extern crate serde;
extern crate serde_json;
extern crate structopt;
extern crate taskmanager;
extern crate tempfile;
extern crate toml;
extern crate wait_timeout;
extern crate xvfb;

mod utils;

use crate::utils::*;
use chrome::ChromeDebuggerMessage;
use encrypted_dns::{chrome_log_contains_errors, ErrorExt};
use failure::{bail, Error, ResultExt};
use lazy_static::lazy_static;
use log::{debug, error, info, warn};
use misc_utils::fs::file_open_read;
use sequences::{sequence_stats, Sequence};
use std::{
    ffi::{OsStr, OsString},
    fmt::{self, Debug},
    fs,
    io::{BufRead, BufReader, Read},
    panic::{self, RefUnwindSafe, UnwindSafe},
    path::{Path, PathBuf},
    sync::Arc,
    thread::{self, JoinHandle},
    time::Duration,
};
use structopt::StructOpt;
use taskmanager::{models::Task, AddDomainConfig, Config, TaskManager};
use tempfile::{Builder as TempDirBuilder, TempDir};
use xvfb::{ProcessStatus, Xvfb};

lazy_static! {
    static ref DNSTAP_FILE_NAME: &'static Path = &Path::new("website-log.dnstap.xz");
    static ref LOG_FILE: &'static Path = &Path::new("website-log.log.xz");
    static ref CHROME_LOG_FILE_NAME: &'static Path = &Path::new("website-log.json.xz");
}

#[derive(StructOpt)]
#[structopt(
    author = "",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
struct CliArgs {
    /// Config file for all advanced settings
    #[structopt(
        short = "c",
        long = "config",
        parse(try_from_os_str = "path_is_file_exists")
    )]
    config: PathBuf,

    #[structopt(subcommand)]
    cmd: SubCommand,
}

impl Debug for CliArgs {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("CliArgs")
            .field("config", &self.config)
            .finish()
    }
}

#[derive(StructOpt)]
enum SubCommand {
    /// Create the initial list of tasks from a domain list
    #[structopt(name = "init")]
    InitTaskSet {
        #[structopt(
            short = "d",
            long = "domain",
            parse(try_from_os_str = "path_file_exists_and_readable_open")
        )]
        domain_list: (Box<Read>, PathBuf),
    },
    /// Start executing the tasks
    #[structopt(name = "run")]
    Run,
    /// Print the CLI arguments to stdout
    #[structopt(name = "debug")]
    Debug,
    /// Create the initial list of tasks from a domain list
    #[structopt(name = "add")]
    AddRecurring {
        #[structopt(
            short = "d",
            long = "domain",
            parse(try_from_os_str = "path_file_exists_and_readable_open")
        )]
        domain_list: (Box<Read>, PathBuf),
    },
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
enum TaskStatus {
    Completed,
    Restarted,
}

fn path_is_file_exists(path: &OsStr) -> Result<PathBuf, OsString> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(format!("{} does not exist", path.display()).into());
    }
    match fs::metadata(&path) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(format!("{} does not refer to a file", path.display()).into());
            }
            Ok(path.to_path_buf())
        }
        Err(err) => Err(format!("Error for file '{}': {}", path.display(), err.to_string()).into()),
    }
}

fn path_file_exists_and_readable_open(path: &OsStr) -> Result<(Box<Read>, PathBuf), OsString> {
    let path = path_is_file_exists(path)?;
    file_open_read(&path)
        .map(|read| (read, path))
        .map_err(|err| err.to_string().into())
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

    debug!("Start loading config file");
    let config = Config::try_load_config(&cli_args.config).context("Could not load config file")?;

    match &cli_args.cmd {
        SubCommand::InitTaskSet { .. } => run_init(cli_args.cmd, config),
        SubCommand::Run => run_exec(cli_args.cmd, config),
        SubCommand::Debug => run_debug(cli_args, config),
        SubCommand::AddRecurring { .. } => run_add_recurring(cli_args.cmd, config),
    }
}

/// Run the initialization for all tasks
///
/// This parses a domain list and will create the initial list of task which we want to execute for
/// them.
#[allow(clippy::needless_pass_by_value)]
fn run_init(cmd: SubCommand, config: Config) -> Result<(), Error> {
    if let SubCommand::InitTaskSet {
        domain_list: (mut domain_list_reader, domain_list_path),
        ..
    } = cmd
    {
        let taskmgr = TaskManager::new(&*config.get_database_path().to_string_lossy())
            .context("Cannot create TaskManager")?;
        taskmgr
            .run_migrations()
            .context("Error while executing migrations")?;

        debug!("Read domains file");
        let domains_r = BufReader::new(&mut domain_list_reader);
        let domains = domains_r
            .lines()
            .collect::<Result<Vec<String>, std::io::Error>>()
            .with_context(|_| format!("Failed to read line in {}", domain_list_path.display()))?;
        info!("Empty old database entries");
        taskmgr
            .delete_all()
            .context("Empty database before filling it")?;
        info!("Add new database entries");
        taskmgr
            .add_domains(
                domains
                    .into_iter()
                    .map(|domain| AddDomainConfig::new(domain, 0, 0, config.per_domain_datasets)),
                config.initial_priority,
            )
            .context("Could not create tasks")?;
    } else {
        unreachable!("The run function verifies which enum variant this is.")
    }
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn run_exec(cmd: SubCommand, config: Config) -> Result<(), Error> {
    if let SubCommand::Run = cmd {
        let taskmgr = TaskManager::new(&*config.get_database_path().to_string_lossy())
            .context("Cannot create TaskManager")?;
        let config = Arc::new(config);

        if config.num_executors == 0 {
            bail!("You need to specify at least one executor.");
        }

        ensure_docker_image_exists(&config.docker_image).context("Check for docker image")?;

        init_global_environment(&config).context("Could not setup the global environment")?;

        let mut handles = Vec::new();

        for i in 0..config.num_executors {
            let taskmgr_ = taskmgr.clone();
            let config_ = config.clone();
            handles.push(run_thread_restart(
                move || process_tasks_docker(&taskmgr_, &config_),
                Some(format!("Docker Executor {}", i)),
            ));
        }

        {
            let config_ = config.clone();
            handles.push(run_thread_restart(
                move || background_update_unbound_cache_dump(&config_),
                Some("Update Unbound Cache".to_string()),
            ));
            let taskmgr_ = taskmgr.clone();
            let config_ = config.clone();
            handles.push(run_thread_restart(
                move || result_sanity_checks(&taskmgr_, &config_),
                Some("Sanity Check Single".to_string()),
            ));
            let taskmgr_ = taskmgr.clone();
            let config_ = config.clone();
            handles.push(run_thread_restart(
                move || result_sanity_checks_domain(&taskmgr_, &config_),
                Some("Sanity Check Domain".to_string()),
            ));
            let taskmgr_ = taskmgr.clone();
            handles.push(run_thread_restart(
                move || cleanup_stale_tasks(&taskmgr_),
                Some("Cleanup stale tasks".to_string()),
            ));
        }

        for handle in handles {
            // TODO make nice
            let _ = handle.join();
        }
    } else {
        unreachable!("The run function verifies which enum variant this is.")
    }
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn run_debug(args: CliArgs, config: Config) -> Result<(), Error> {
    println!("{:#?}", args);
    println!("{:#?}", config);
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn run_add_recurring(cmd: SubCommand, config: Config) -> Result<(), Error> {
    if let SubCommand::AddRecurring {
        domain_list: (mut domain_list_reader, domain_list_path),
        ..
    } = cmd
    {
        let taskmgr = TaskManager::new(&*config.get_database_path().to_string_lossy())
            .context("Cannot create TaskManager")?;

        debug!("Read domains file");
        let domains = BufReader::new(&mut domain_list_reader)
            .lines()
            .collect::<Result<Vec<String>, std::io::Error>>()
            .with_context(|_| format!("Failed to read line in {}", domain_list_path.display()))?;

        let domain_state = taskmgr
            .get_domain_state(&domains)
            .context("Failed to retrieve the domainstate")?;
        taskmgr
            .add_domains(
                domain_state.into_iter().map(|dc| {
                    dc.into_add_domain_config(config.per_domain_datasets_repeated_measurements)
                }),
                0,
            )
            .context("Failed to add repeated domains tasks")?;
    } else {
        unreachable!("The run function verifies which enum variant this is.")
    }
    Ok(())
}

/// Make function execution in threads persistent
///
/// This is a small wrapper around `thread::spawn`, which ensures that if a thread panics or the
/// function returns it is restarted.
fn run_thread_restart<F>(function: F, name: Option<String>) -> JoinHandle<()>
where
    F: UnwindSafe + RefUnwindSafe + Send + 'static,
    F: Fn() -> Result<(), Error>,
{
    let mut builder = thread::Builder::new();
    if let Some(name) = &name {
        builder = builder.name(name.clone());
    }

    builder
        .spawn(move || loop {
            let res = panic::catch_unwind(&function);
            if let Ok(Err(err)) = res {
                error!("{}", err.display_causes());
            }
            error!(
                "Thread {} stopped, restart",
                name.as_ref().map(|s| &**s).unwrap_or("<unknown>")
            );
            thread::sleep(Duration::new(10, 0));
        })
        .unwrap()
}

/// Perform the execution of a task in a VM
///
/// This function is responsible for all the steps related to capturing the measurement data from
/// the VMs.
fn process_tasks_docker(taskmgr: &TaskManager, config: &Config) -> Result<(), Error> {
    let mut xvfb = None;
    loop {
        if let Some(mut task) = taskmgr.get_task_for_vm()? {
            if xvfb.is_none() {
                debug!("{}: Start new xvfb", task.name());
                xvfb = Some(Xvfb::new().context("Failed to spawn virtual frame buffer")?);
            }

            let taskstatus = execute_or_restart_task(&mut task, taskmgr, |mut task| {
                let xvfb = xvfb.as_ref().expect("xvfb must always exist and be Some()");
                let tmp_dir = TempDirBuilder::new().prefix("docker").tempdir()?;
                info!(
                    "Process task {} ({}), step Docker, tmp dir {}",
                    task.name(),
                    task.id(),
                    tmp_dir.path().display()
                );

                debug!("{}: Copy initial files to mount point", task.name());
                // Write all the required files to the mount point
                fs::copy(config.get_cache_file(), tmp_dir.path().join("cache.dump"))
                    .with_context(|_| format!("{}: Failed to copy cache.dump", task.name()))?;
                fs::write(
                    tmp_dir.path().join("domain"),
                    &format!("http://{}", task.domain()),
                )
                .with_context(|_| format!("{}: Failed to create file `domain`", task.name()))?;
                fs::write(
                    tmp_dir.path().join("display"),
                    &xvfb.get_display().to_string(),
                )
                .with_context(|_| format!("{}: Failed to create file `display`", task.name()))?;

                debug!("{}: Run docker container", task.name());
                let _status = docker_run(
                    &config.docker_image,
                    tmp_dir.path(),
                    None,
                    Duration::new(60, 0),
                )
                .with_context(|_| format!("{}: Failed to start the measuremetns", task.name()))?;
                debug!("{}: Copy files from mount point to local back", task.name());
                let local_path: PathBuf = config.get_collected_results_path().join(task.name());
                ensure_path_exists(&local_path)?;

                for fname in &[&*DNSTAP_FILE_NAME, &*LOG_FILE, &*CHROME_LOG_FILE_NAME] {
                    let fname = fname.with_extension("");
                    fs::copy(tmp_dir.path().join(&fname), local_path.join(&fname)).with_context(
                        |_| {
                            format!(
                                "{}: Failed to copy back file {}",
                                task.name(),
                                fname.display()
                            )
                        },
                    )?;
                }
                tmp_dir.close()?;

                debug!("Finished task {} ({})", task.name(), task.id());
                taskmgr.finished_task_for_vm(&mut task)
            })?;

            // Perform tests to see if we need to restart xvfb
            if taskstatus != TaskStatus::Completed {
                debug!("Kill xvfb due to error in task execution");
                xvfb = None;
            }
            let is_xvfb_alive = xvfb.as_mut().map(|xvfb| match xvfb.process_status() {
                ProcessStatus::Alive => true,
                _ => false,
            });
            if is_xvfb_alive != Some(true) {
                debug!("Reset xvfb, as it is no longer alive");
                xvfb = None;
            }
        } else {
            info!("No tasks left for Docker");
            thread::sleep(Duration::new(10, 0));
        }
    }
}

/// Cleanup stale tasks by resetting them
fn cleanup_stale_tasks(taskmgr: &TaskManager) -> Result<(), Error> {
    loop {
        let tasks = taskmgr
            .get_stale_tasks()
            .context("Failed to get stale tasks")?;
        for mut task in tasks {
            taskmgr.restart_task(&mut task, &"Restart stale task")?;
        }

        // run every 30 minutes
        thread::sleep(Duration::new(30 * 60, 0));
    }
}

/// Check the VM results for consistency
fn result_sanity_checks(taskmgr: &TaskManager, config: &Config) -> Result<(), Error> {
    let local_path = config.get_collected_results_path();

    loop {
        let tasks = taskmgr.results_need_sanity_check_single()?;
        for mut task in tasks {
            execute_or_restart_task(&mut task, taskmgr, |mut task| {
                // compress files to save space
                for entry in fs::read_dir(local_path.join(task.name()))? {
                    let entry = entry?;
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_file() {
                            let path = entry.path();
                            xz(&*path).with_context(|_| {
                                format!("Failed to compress {}", path.display())
                            })?;
                        }
                    }
                }

                // if a file is loadable, it passes all easy sanity checks
                Sequence::from_path(&local_path.join(task.name()).join(&*DNSTAP_FILE_NAME))
                    .with_context(|_| {
                        format!(
                            "The task {} generated invalid data and gets restarted.",
                            task.name()
                        )
                    })?;

                let chrome_log = local_path.join(task.name()).join(&*CHROME_LOG_FILE_NAME);
                // open Chrome file and parse it
                let mut rdr = file_open_read(&chrome_log)
                    .with_context(|_| format!("Failed to read {}", chrome_log.display()))?;
                let mut content = String::new();
                rdr.read_to_string(&mut content)
                    .with_context(|_| format!("Error while reading '{}'", chrome_log.display()))?;
                let msgs: Vec<ChromeDebuggerMessage> = serde_json::from_str(&content)
                    .with_context(|_| {
                        format!("Error while deserializing '{}'", chrome_log.display())
                    })?;
                if let Some(err_reason) = chrome_log_contains_errors(&msgs) {
                    bail!(
                        "Fail task {} ({}) due to chrome log: {}",
                        task.name(),
                        task.id(),
                        err_reason
                    );
                }

                taskmgr.mark_results_checked_single(&mut task)
            })?;
        }
        thread::sleep(Duration::new(10, 0));
    }
}

/// Make sure all necessary files are copied to the VM
fn init_global_environment(config: &Config) -> Result<(), Error> {
    // Create global tmp directory
    update_unbound_cache_dump(config)
}

fn execute_or_restart_task<F>(
    task: &mut Task,
    taskmgr: &TaskManager,
    func: F,
) -> Result<TaskStatus, Error>
where
    F: FnOnce(&mut Task) -> Result<(), Error>,
{
    let res = func(task);
    if let Err(err) = res {
        warn!("{}", err.display_causes());
        taskmgr.restart_task(task, &err.display_causes())?;
        Ok(TaskStatus::Restarted)
    } else {
        Ok(TaskStatus::Completed)
    }
}

/// Check the VM results for consistency
fn result_sanity_checks_domain(taskmgr: &TaskManager, config: &Config) -> Result<(), Error> {
    let local_path = config.get_collected_results_path();
    let results_path = config.get_results_path();

    loop {
        ensure_path_exists(&results_path)?;

        let tasks = taskmgr.results_need_sanity_check_domain()?;
        if tasks.is_none() {
            info!("No tasks for sanity check domains");
            thread::sleep(Duration::new(10, 0));
            continue;
        }
        // we just checked that tasks is not None
        let mut tasks = tasks.unwrap();
        info!("Sanity check domains: '{}'", tasks[0].name());

        let sequences: Vec<_> = tasks
            .iter()
            .map(|task| {
                Sequence::from_path(&local_path.join(task.name()).join(&*DNSTAP_FILE_NAME))
                    .expect("Loading a DNSTAP file cannot fail, as we checked that before.")
            })
            .collect();

        let mark_domain_good = |tasks: &mut Vec<Task>| -> Result<(), Error> {
            info!("Sanity check domain: Marked Good: '{}'", tasks[0].name());
            //everything is fine, advance the tasks to next stage
            for task in &*tasks {
                let outdir = results_path.join(task.domain());
                ensure_path_exists(&outdir)?;

                let old_task_dir = local_path.join(task.name());

                for (filename, new_file_ext) in &[
                    (&*DNSTAP_FILE_NAME, "dnstap.xz"),
                    (&*LOG_FILE, "log.xz"),
                    (&*CHROME_LOG_FILE_NAME, "json.xz"),
                ] {
                    let src = old_task_dir.join(filename);
                    let dst = results_path.join(task.domain()).join(format!(
                        "{}.{}",
                        task.name(),
                        new_file_ext
                    ));
                    fs::rename(&src, &dst).with_context(|_| {
                        format!("Failed to move {} to {}", src.display(), dst.display())
                    })?;
                }
                fs::remove_dir(&old_task_dir).with_context(|_| {
                    format!(
                        "Could not remove old task directory {}",
                        old_task_dir.display()
                    )
                })?;
            }

            taskmgr
                .mark_results_checked_domain(tasks)
                .context("Failed to mark domain tasks as finished.")?;
            Ok(())
        };

        let (_, median_distances, _, avg_median) = sequence_stats(&sequences, &sequences);

        let is_bad_dist = |dist| {
            // absolute difference is too much
            ((dist as isize) - (avg_median as isize)).abs() > config.max_allowed_dist_difference_abs as isize ||
            // dist is at least X times larger than avg_avg
            dist as f32 > (avg_median as f32 * config.max_allowed_dist_difference) ||
            // dist is at least X time smaller than avg_avg
            (dist as f32) < (avg_median as f32 / config.max_allowed_dist_difference)
        };

        // if there is only a single bad value, only restart that
        // if there are multiple bad values, restart whole domain

        if avg_median <= 10 {
            mark_domain_good(&mut tasks)?;
            continue;
        }

        // Only do this for the initial measurement with groupid 0 or for measurements of sufficient size.
        // It does not make sense to do this for the repeated measurements with a single or two requests each.
        if tasks[0].groupid() == 0 || tasks[0].groupsize() >= 10 {
            match median_distances
                .iter()
                .filter(|dist| is_bad_dist(**dist))
                .count()
            {
                0 => {
                    mark_domain_good(&mut tasks)?;
                }
                1 => {
                    // restart the single bad task
                    let (dist, mut task) = median_distances
                        .iter()
                        .zip(tasks.iter_mut())
                        .find(|(dist, _task)| is_bad_dist(**dist))
                        .expect("There is exactly one task");
                    info!(
                        "Restart task {} because of distance difference",
                        task.name()
                    );
                    taskmgr
                        .restart_task(
                            &mut task,
                            &format!(
                                "The task's distance is {} while the average distance is only {}",
                                dist, avg_median
                            ),
                        )
                        .context("Cannot restart single bad task")?;
                }
                n => {
                    // restart all tasks
                    info!(
                        "Restart task of domain {} groupid {} because of distance difference",
                        tasks[0].domain(),
                        tasks[0].groupid(),
                    );
                    taskmgr
                        .restart_tasks(
                            &mut *tasks,
                            &format!(
                                "{} out of {} differ by too much from the average distance",
                                n, config.per_domain_datasets
                            ),
                        )
                        .context("Cannot restart bad domain")?;
                }
            }
        } else {
            // we always need to take a decision
            // so mark them as good
            mark_domain_good(&mut tasks)?;
        }
    }
}

fn update_unbound_cache_dump(config: &Config) -> Result<(), Error> {
    let tmp_dir = TempDir::new()?;
    // Copy the prefetching list to the mount point
    fs::copy(
        config.get_prefetch_file(),
        tmp_dir.path().join("prefetch-domains.txt"),
    )
    .context("Prefetch file missing")?;
    info!(
        "Start Unbound refresh in Docker with tmp dir '{}'",
        tmp_dir.path().display()
    );
    let status = docker_run(
        &config.docker_image,
        tmp_dir.path(),
        Some("/usr/bin/create-cache-dump.fish"),
        Duration::new(120, 0),
    )
    .context("Failed to run docker image to create a cache dump")?;
    if !status.success() {
        bail!("Creating the unbound cache dump failed");
    }

    // Copy the file from the temporary directory to the working directory
    // Do not copy it to the final destination yet, this should be atomic
    fs::copy(
        tmp_dir.path().join("cache.dump.new"),
        config.get_cache_file().with_extension("tmp"),
    )
    .context("The new cache.dump.new file is missing")?;
    fs::rename(
        config.get_cache_file().with_extension("tmp"),
        config.get_cache_file(),
    )?;
    tmp_dir.close()?;
    Ok(())
}

fn background_update_unbound_cache_dump(config: &Config) -> Result<(), Error> {
    loop {
        update_unbound_cache_dump(config)?;
        thread::sleep(Duration::from_secs(u64::from(config.refresh_cache_seconds)));
    }
}
