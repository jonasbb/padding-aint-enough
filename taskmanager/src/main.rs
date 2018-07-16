extern crate env_logger;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
extern crate misc_utils;
#[macro_use]
extern crate structopt;
extern crate serde;
extern crate taskmanager;
extern crate toml;

use failure::{Error, ResultExt};
use misc_utils::fs::file_open_read;
use std::{
    ffi::{OsStr, OsString},
    fmt::{self, Debug},
    fs,
    io::{self, BufRead, BufReader, Read},
    panic::{self, RefUnwindSafe, UnwindSafe},
    path::{Path, PathBuf},
    process::Command,
    thread::{self, JoinHandle},
    time::Duration,
};
use structopt::StructOpt;
use taskmanager::*;

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
            .delete_all_tasks()
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

        if config.executors.is_empty() {
            bail!("You need to specify at least one executor.");
        }

        // taskmgr
        //     .get_task_for_vm(&config.executors[0])
        //     .context("Could not create tasks")?;

        let mut handles = Vec::new();

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
            handles.push(run_thread_restart(
                move || copy_vm_results(&taskmgr_),
                Some("Results Collector".to_string()),
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
fn run_thread_restart<F, R>(function: F, name: Option<String>) -> JoinHandle<()>
where
    F: Fn() -> R + UnwindSafe + RefUnwindSafe + Send + 'static,
{
    let mut builder = thread::Builder::new();
    if let Some(name) = name {
        builder = builder.name(name);
    }

    builder
        .spawn(move || loop {
            if let Err(err) = panic::catch_unwind(&function) {
                if let Some(err) = err.downcast_ref::<Error>() {
                    error!("{}", err);
                }
            }
            eprintln!("Thread stopped, restart");
            thread::sleep(Duration::new(1, 0));
        })
        .unwrap()
}

/// Perform the execution of a task in a VM
///
/// This function is responsible for all the steps related to capturing the measurement data from
/// the VMs.
fn process_tasks_vm(taskmgr: &TaskManager, executor: &Executor) {
    loop {
        if let Some(mut task) = taskmgr.get_task_for_vm(executor).unwrap() {
            println!("Process task {}", task.id());
            thread::sleep(Duration::new(2, 0));
            println!("Advance task {}", task.id());
            taskmgr.finished_task_for_vm(&mut task, &Path::new("/"));
        } else {
            println!("No tasks left");
            thread::sleep(Duration::new(2, 0));
        }
    }
}

/// Cleanup stale tasks by resetting them
fn cleanup_stale_tasks(taskmgr: &TaskManager) {
    // TODO restart counter !!!
    unimplemented!()
}

/// Copy the finished results from a VM to the global directory
fn copy_vm_results(taskmgr: &TaskManager) -> Result<(), Error> {
    let tasks = taskmgr.results_collectable()?;
    for (task, data) in tasks {
        // copy data from VM
        Command::new("scp").args(&[
            "-pr",
            &format!("{}:{}", data.executor.sshconnect, data.path_on_vm.display()),
        ]);
    }
    Ok(())
}

/// Check the VM results for consistency
fn result_sanity_checks(taskmgr: &TaskManager) {
    unimplemented!()
}

fn ensure_path_exists(path: &Path) -> io::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}
