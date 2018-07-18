#![allow(proc_macro_derive_resolution_fallback)]
#![allow(dead_code)]

extern crate encrypted_dns;
extern crate env_logger;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate misc_utils;
extern crate rayon;
#[macro_use]
extern crate structopt;
extern crate serde;
extern crate taskmanager;
extern crate toml;

use encrypted_dns::{dnstap_to_sequence, ErrorExt};
use failure::{Error, ResultExt};
use misc_utils::fs::file_open_read;
use rayon::prelude::*;
use std::{
    ffi::{OsStr, OsString},
    fmt::{self, Debug},
    fs,
    io::{self, BufRead, BufReader, Read},
    panic::{self, RefUnwindSafe, UnwindSafe},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    thread::{self, JoinHandle},
    time::Duration,
};
use structopt::StructOpt;
use taskmanager::{models::Task, *};

lazy_static! {
    static ref DNSTAP_FILE_NAME: &'static Path = &Path::new("website-log.dnstap.xz");
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
        for fail in err.causes() {
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
    }
}

/// Run the initialization for all tasks
///
/// This parses a domain list and will create the initial list of task which we want to execute for
/// them.
#[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
fn run_init(cmd: SubCommand, config: Config) -> Result<(), Error> {
    if let SubCommand::InitTaskSet {
        domain_list: (mut domain_list_reader, domain_list_path),
        ..
    } = cmd
    {
        let mut taskmgr = TaskManager::new(&*config.get_database_path().to_string_lossy())
            .context("Cannot create TaskManager")?;

        let domains_r = BufReader::new(&mut domain_list_reader);
        let domains = domains_r
            .lines()
            .collect::<Result<Vec<String>, std::io::Error>>()
            .with_context(|_| format!("Failed to read line in {}", domain_list_path.display()))?;
        taskmgr
            .delete_all()
            .context("Empty database before filling it")?;
        taskmgr
            .add_domains(domains, config.per_domain_datasets)
            .context("Could not create tasks")?;
    } else {
        unreachable!("The run function verifies which enum variant this is.")
    }
    Ok(())
}

#[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
fn run_exec(cmd: SubCommand, config: Config) -> Result<(), Error> {
    if let SubCommand::Run = cmd {
        let mut taskmgr = TaskManager::new(&*config.get_database_path().to_string_lossy())
            .context("Cannot create TaskManager")?;
        let config = Arc::new(config);

        if config.executors.is_empty() {
            bail!("You need to specify at least one executor.");
        }

        if !config.get_scripts_dir().exists() {
            bail!(
                "The local directory with scripts does not exist!\nMissing {}",
                config.get_scripts_dir().display()
            );
        }

        let mut handles = Vec::new();

        &config
            .executors
            .par_iter()
            .map(|executor| init_vm(executor, &config))
            .collect::<Result<(), Error>>()
            .context("Could not initialize all VMs")?;

        for executor in &config.executors {
            let executor_ = executor.clone();
            let taskmgr = taskmgr.clone();
            handles.push(run_thread_restart(
                move || process_tasks_vm(&taskmgr, &executor_),
                Some(format!("VM Executor `{}`", executor.name)),
            ));
        }

        {
            let taskmgr_ = taskmgr.clone();
            let config_ = config.clone();
            handles.push(run_thread_restart(
                move || copy_vm_results(&taskmgr_, &config_),
                Some("Results Collector".to_string()),
            ));
            let taskmgr_ = taskmgr.clone();
            let config_ = config.clone();
            handles.push(run_thread_restart(
                move || result_sanity_checks(&taskmgr_, &config_),
                Some("Sanity Check Single".to_string()),
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

#[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
fn run_debug(args: CliArgs, config: Config) -> Result<(), Error> {
    println!("{:#?}", args);
    println!("{:#?}", config);
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
                error!("{}", err);
            }
            error!(
                "Thread {} stopped, restart",
                name.as_ref().map(|s| &**s).unwrap_or("<unknown>")
            );
            thread::sleep(Duration::new(1, 0));
        })
        .unwrap()
}

/// Perform the execution of a task in a VM
///
/// This function is responsible for all the steps related to capturing the measurement data from
/// the VMs.
fn process_tasks_vm(taskmgr: &TaskManager, executor: &Executor) -> Result<(), Error> {
    loop {
        if let Some(mut task) = taskmgr.get_task_for_vm(executor)? {
            execute_or_restart_task(&mut task, taskmgr, |mut task| {
                info!("Process task {}, step VM", task.id());
                let script_path = executor
                    .working_directory
                    .join("scripts")
                    .join("record-websites.fish");
                let output_path = executor.working_directory.join(task.name());
                let logfile = output_path.join("task.log");

                // Run task on VM
                // Execute: mkdir -p OUTPUT_PATH && cd OUTPUT_PATH && fish ./SCRIPT_PATH DOMAIN >LOGFILE 2>&1
                let res = Command::new("ssh")
                    .args(&[&executor.sshconnect, "--", "mkdir", "-p"])
                    .arg(&output_path)
                    .args(&["&&", "cd"])
                    .arg(&output_path)
                    .args(&["&&", "fish"])
                    .arg(&script_path)
                    .arg(&format!("http://{}/", task.domain()))
                    .arg(">")
                    .arg(&logfile)
                    .arg("2>&1")
                    .status()
                    .with_context(|_| {
                        format!(
                            "Could not run script for task {} on VM {}",
                            task.name(),
                            executor.name,
                        )
                    })?;
                if !res.success() {
                    bail!(
                        "Could not run script for task {} on VM {}",
                        task.name(),
                        executor.name,
                    )
                }

                taskmgr.finished_task_for_vm(&mut task, &output_path)
            })?;
        } else {
            info!("No tasks left for VM");
            thread::sleep(Duration::new(10, 0));
        }
    }
}

/// Cleanup stale tasks by resetting them
fn cleanup_stale_tasks(_taskmgr: &TaskManager) {
    // TODO restart counter !!!
    unimplemented!()
}

/// Copy the finished results from a VM to the global directory
fn copy_vm_results(taskmgr: &TaskManager, config: &Config) -> Result<(), Error> {
    let local_path = config.get_collected_results_path();
    loop {
        ensure_path_exists(&local_path).context("Cannot create local path for collected results")?;
        let tasks = taskmgr.results_collectable()?;
        for (mut task, data) in tasks {
            execute_or_restart_task(&mut task, taskmgr, |mut task| {
                // copy data from VM to local directory
                scp_file(
                    &data.executor,
                    ScpDirection::RemoteToLocal,
                    &local_path,
                    &data.path_on_vm,
                )?;
                // copy data from VM to local directory
                let res = Command::new("ssh")
                    .args(&[&data.executor.sshconnect, "--", "rm", "-rf"])
                    .arg(&data.path_on_vm)
                    .status()
                    .with_context(|_| {
                        format!(
                            "Could not copy the results of {} from VM {}",
                            task.name(),
                            data.executor.name,
                        )
                    })?;
                if !res.success() {
                    bail!(
                        "Could not copy the results of {} from VM {}",
                        task.name(),
                        data.executor.name,
                    )
                }

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

                taskmgr.mark_results_collected(&mut task)
            })?;
        }
        thread::sleep(Duration::new(1, 0));
    }
}

/// Check the VM results for consistency
fn result_sanity_checks(taskmgr: &TaskManager, config: &Config) -> Result<(), Error> {
    let local_path = config.get_collected_results_path();

    loop {
        let tasks = taskmgr.results_need_sanity_check_single()?;
        for mut task in tasks {
            execute_or_restart_task(&mut task, taskmgr, |mut task| {
                // if a file is loadable, it passes all easy sanity checks
                dnstap_to_sequence(&local_path.join(task.name()).join(&*DNSTAP_FILE_NAME))
                    .with_context(|_| {
                        format!(
                            "The task {} generated invalid data and gets restarted.",
                            task.name()
                        )
                    })?;
                taskmgr.mark_results_checked_single(&mut task)
            })?;
        }
        thread::sleep(Duration::new(1, 0));
    }
}

/// Ensure the given path exists and if not create it
fn ensure_path_exists(path: &Path) -> io::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

enum ScpDirection {
    LocalToRemote,
    RemoteToLocal,
}

/// Copy files between local and remote in both directions
fn scp_file(
    executor: &Executor,
    direction: ScpDirection,
    local_path: &Path,
    remote_path: &Path,
) -> Result<(), Error> {
    let mut cmd = Command::new("scp");
    cmd.arg("-pr");
    let mut remote =
        OsString::with_capacity(executor.name.len() + remote_path.as_os_str().len() + 4);
    remote.push(&executor.name);
    remote.push(":");
    remote.push(remote_path);
    match direction {
        ScpDirection::LocalToRemote => cmd.args(&[local_path.as_os_str(), &*remote]),
        ScpDirection::RemoteToLocal => cmd.args(&[&*remote, local_path.as_os_str()]),
    };
    let res = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .context("Could not start scp")?;
    if !res.success() {
        bail!("scp did not finish successfully")
    }
    Ok(())
}

/// Make sure all necessary files are copied to the VM
fn init_vm(executor: &Executor, config: &Config) -> Result<(), Error> {
    let res = Command::new("ssh")
        .arg(&executor.sshconnect)
        .arg("--")
        .arg("mkdir")
        .arg("-p")
        .arg(&executor.working_directory)
        .status()
        .with_context(|_| format!("Could not create working dir on VM {}", executor.name,))?;
    if !res.success() {
        bail!("Could not create working dir on VM {}", executor.name,)
    }
    scp_file(
        executor,
        ScpDirection::LocalToRemote,
        &config.get_scripts_dir(),
        &executor.working_directory,
    )
}

fn execute_or_restart_task<F>(task: &mut Task, taskmgr: &TaskManager, func: F) -> Result<(), Error>
where
    F: FnOnce(&mut Task) -> Result<(), Error>,
{
    let res = func(task);
    if let Err(err) = res {
        taskmgr.restart_task(task, &err.display_causes())
    } else {
        Ok(())
    }
}

/// Compress a file with xz
fn xz(path: &Path) -> Result<(), Error> {
    // skip already compressed files
    if path.extension() == Some(OsStr::new("xz")) {
        return Ok(());
    }

    let res = Command::new("xz")
        .args(&["-9", "--force"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .context("Could not start xz")?;
    if !res.success() {
        bail!("xz did not finish successfully")
    }
    Ok(())
}
