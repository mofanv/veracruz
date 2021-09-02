//! A freestanding version of the Veracruz execution engine, for offline development.
//!
//! ## About
//!
//! The WASM binary to execute, and any data sources being passed to the binary,
//! are passed with the `--binary` and `--data` flags, respectively.  A
//! runtime error is raised if the number of input data sources does not match
//! the number specified in the configuration TOML.
//!
//! To see verbose output of what is happening, set `RUST_LOG=info` before
//! executing.
//!
//! On success, the return value of the WASM program's `main` function, and the
//! result that the program wrote back to Veracruz host, will be printed to
//! stdout.
//!
//! ## Authors
//!
//! The Veracruz Development Team.
//!
//! ## Copyright
//!
//! See the file `LICENSE_MIT.markdown` in the Veracruz root directory for licensing
//! and copyright information.

use clap::{App, Arg};
use execution_engine::{execute, fs::FileSystem};
use log::*;
use std::{
    collections::HashMap,
    convert::TryFrom,
    error::Error,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    str::FromStr,
    time::Instant,
    vec::Vec,
};
use veracruz_utils::policy::principal::{ExecutionStrategy, FileRights, Principal, StandardStream};
use wasi_types::Rights;

////////////////////////////////////////////////////////////////////////////////
// Constants.
////////////////////////////////////////////////////////////////////////////////

/// About freestanding-execution-engine/Veracruz.
const ABOUT: &'static str = "Veracruz: a platform for practical secure multi-party \
                             computations.\nThis is freestanding-execution-engine, an offline \
                             counterpart of the Veracruz execution engine that is part of the \
                             Veracruz platform.  This can be used to test and develop WASM \
                             programs before deployment on the platform.";
/// The name of the application.
const APPLICATION_NAME: &'static str = "freestanding-execution-engine";
/// The authors list.
const AUTHORS: &'static str = "The Veracruz Development Team.  See the file `AUTHORS.markdown` in \
                               the Veracruz root directory for detailed authorship information.";
/// Application version number.
const VERSION: &'static str = "pre-alpha";

/// The default dump status of `stdout`, if no alternative is provided on the
/// command line.
const DEFAULT_DUMP_STDOUT: bool = false;
/// The default dump status of `stderr`, if no alternative is provided on the
/// command line.
const DEFAULT_DUMP_STDERR: bool = false;

////////////////////////////////////////////////////////////////////////////////
// Command line options and parsing.
////////////////////////////////////////////////////////////////////////////////

/// A struct capturing all of the command line options passed to the program.
struct CommandLineOptions {
    /// The list of file names passed as input data-sources.
    input_sources: Vec<String>,
    /// The list of file names passed as ouput.
    output_sources: Vec<String>,
    /// The filename passed as the WASM program to be executed.
    binary: String,
    /// The execution strategy to use when performing the computation.
    execution_strategy: ExecutionStrategy,
    /// Whether the contents of `stdout` should be dumped before exiting
    dump_stdout: bool,
    /// Whether the contents of `stderr` should be dumped before exiting
    dump_stderr: bool,
}

/// Parses the command line options, building a `CommandLineOptions` struct out
/// of them.  If required options are not present, or if any options are
/// malformed, this will abort the program.
fn parse_command_line() -> Result<CommandLineOptions, Box<dyn Error>> {

    let default_dump_stdout = DEFAULT_DUMP_STDOUT.to_string();
    let default_dump_stderr = DEFAULT_DUMP_STDERR.to_string();

    let matches = App::new(APPLICATION_NAME)
        .version(VERSION)
        .author(AUTHORS)
        .about(ABOUT)
        .arg(
            Arg::with_name("input")
                .short("i")
                .long("input-source")
                .value_name("FILES OR DIRECTORIES")
                .help("Space-separated paths to the input data source files or directories on disk.")
                .multiple(true),
        )
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output-source")
                .value_name("FILES")
                .help("Space-separated paths to the output data source files on disk.")
                .multiple(true),
        )
        .arg(
            Arg::with_name("program")
                .short("p")
                .long("program")
                .value_name("FILE")
                .help("Path to the WASM binary on disk.")
                .multiple(false)
                .required(true),
        )
        .arg(
            Arg::with_name("execution-strategy")
                .short("x")
                .long("execution-strategy")
                .value_name("interp | jit")
                .help(
                    "Selects the execution strategy to use: interpretation or JIT (defaults to \
                     interpretation).",
                )
                .required(false)
                .multiple(false)
                .default_value("interp"),
        )
        .arg(
            Arg::with_name("dump-stdout")
                .short("d")
                .long("dump-stdout")
                .help("Whether the contents of stdout should be dumped before exiting")
                .required(true)
                .value_name("BOOLEAN")
                .default_value(&default_dump_stdout),
        )
        .arg(
            Arg::with_name("dump-stderr")
                .short("e")
                .long("dump-stderr")
                .help("Whether the contents of stderr should be dumped before exiting")
                .required(true)
                .value_name("BOOLEAN")
                .default_value(&default_dump_stderr),
        )
        .get_matches();

    info!("Parsed command line.");

    let execution_strategy = if let Some(strategy) = matches.value_of("execution-strategy") {
        if strategy == "interp" {
            info!("Selecting interpretation as the execution strategy.");
            ExecutionStrategy::Interpretation
        } else if strategy == "jit" {
            info!("Selecting JITting as the execution strategy.");
            ExecutionStrategy::JIT
        } else {
            return Err(format!(
                "Expecting 'interp' or 'jit' as selectable execution strategies, but found {}",
                strategy
            )
            .into());
        }
    } else {
        return Err("Default 'interp' value is not loaded correctly".into());
    };

    let binary = if let Some(binary) = matches.value_of("program") {
        info!("Using '{}' as our WASM executable.", binary);
        binary.to_string()
    } else {
        return Err("No binary file provided.".into());
    };
    let input_sources = if let Some(data) = matches.values_of("input") {
        let input_sources: Vec<String> = data.map(|e| e.to_string()).collect();
        info!(
            "Selected {} data sources as input to computation.",
            input_sources.len()
        );
        input_sources
    } else {
        Vec::new()
    };

    let output_sources = if let Some(data) = matches.values_of("output") {
        let output_sources: Vec<String> = data.map(|e| e.to_string()).collect();
        info!(
            "Selected {} data sources as input to computation.",
            output_sources.len()
        );
        output_sources
    } else {
        Vec::new()
    };

    let dump_stdout = if let Some(dump_stdout) = matches.value_of("dump-stdout") {
        if let Ok(dump_stdout) = bool::from_str(dump_stdout) {
            if dump_stdout { info!("stdout configured to be dumped before exiting."); }
            dump_stdout
        } else {
            return Err(format!("Expecting a boolean, but found {}", dump_stdout)
            .into());
        }
    } else {
        return Err("Default 'dump-stdout' value is not loaded correctly".into());
    };

    let dump_stderr = if let Some(dump_stderr) = matches.value_of("dump-stderr") {
        if let Ok(dump_stderr) = bool::from_str(dump_stderr) {
            if dump_stderr { info!("stderr configured to be dumped before exiting."); }
            dump_stderr
        } else {
            return Err(format!("Expecting a boolean, but found {}", dump_stderr)
            .into());
        }
    } else {
        return Err("Default 'dump-stderr' value is not loaded correctly".into());
    };

    Ok(CommandLineOptions {
        input_sources,
        output_sources,
        binary,
        execution_strategy,
        dump_stdout,
        dump_stderr,
    })
}

/// Reads a WASM file from disk (actually, will read any file, but we only need
/// it for WASM here) and return a collection of bytes corresponding to that
/// file.  Will abort the program if anything goes wrong.
fn load_file(file_path: &str) -> Result<(String, Vec<u8>), Box<dyn Error>> {
    info!("Opening file '{}' for reading.", file_path);

    let mut file = File::open(file_path)?;
    let mut contents = Vec::new();

    file.read_to_end(&mut contents)?;

    Ok((
        Path::new(file_path)
            .file_name()
            .ok_or(format!("Failed to obtain file name on {}", file_path))?
            .to_str()
            .ok_or(format!(
                "Failed to convert file name to string on {}",
                file_path
            ))?
            .to_string(),
        contents,
    ))
}

/// Loads the specified data sources, as provided on the command line, for
/// reading and massages them into metadata frames, ready for
/// the computation.  May abort the program if something goes wrong when reading
/// any data source.
fn load_input_sources(
    cmdline: &CommandLineOptions, vfs: Arc<Mutex<FileSystem>>,
) -> Result<(), Box<dyn Error>> {
    for (id, file_path) in cmdline.input_sources.iter().enumerate() {
        info!(
            "Loading data source '{}' with id {} for reading.",
            file_path, id
        );
        let mut file = File::open(file_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        vfs.lock()
            .map_err(|e| format!("Failed to lock vfs, error: {:?}", e))?
            .write_file_by_absolute_path(&Principal::InternalSuperUser, &Path::new("/").join(file_path), &buffer, false)?;

        info!("Loading '{}' into vfs.", file_path);
    }
    Ok(())
}

/// Entry: reads the static configuration and the command line parameters,
/// parsing both and then starts provisioning the Veracruz host state, before
/// invoking the entry point.
fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let cmdline = parse_command_line()?;
    info!("Command line read successfully.");

    let (prog_file_name, program) = load_file(&cmdline.binary)?;
    let prog_file_abs_path = Path::new("/").join(prog_file_name.clone());

    let mut right_table = HashMap::new();
    let mut file_table = HashMap::new();
    let read_right = Rights::PATH_OPEN | Rights::FD_READ | Rights::FD_SEEK;
    let write_right = Rights::PATH_OPEN
        | Rights::FD_WRITE
        | Rights::FD_SEEK
        | Rights::PATH_CREATE_FILE
        | Rights::PATH_FILESTAT_SET_SIZE;

    // Set up standard streams table
    let std_streams_table = vec![
        StandardStream::Stdin(FileRights::new(String::from("stdin"), u64::from(read_right) as u32)),
        StandardStream::Stdout(FileRights::new(String::from("stdout"), u64::from(write_right) as u32)),
        StandardStream::Stderr(FileRights::new(String::from("stderr"), u64::from(write_right) as u32)),
    ];

    // Manually create the Right table for the VFS.
    // Add read and readdir permissions to root dir
    file_table.insert(Path::new("/").to_path_buf(), read_right | Rights::FD_READDIR | Rights::PATH_CREATE_DIRECTORY);
    // Add read permission to program
    file_table.insert(prog_file_abs_path.clone(), read_right);
    // Add read permission to input file
    for file_path in cmdline.input_sources.iter() {
        // NOTE: inject the root path.
        file_table.insert(Path::new("/").join(file_path), read_right);
    }
    for std_stream in &std_streams_table {
        let (path, rights) = match std_stream {
            StandardStream::Stdin(file_rights) => (file_rights.file_name(), file_rights.rights()),
            StandardStream::Stdout(file_rights) => (file_rights.file_name(), file_rights.rights()),
            StandardStream::Stderr(file_rights) => (file_rights.file_name(), file_rights.rights()),
        };
        let rights = Rights::try_from(*rights as u64)
            .map_err(|e| format!("Failed to convert u64 to Rights: {:?}", e))?;
        file_table.insert(PathBuf::from(path), rights);
    }
    // Add write permission to output file
    for file_path in cmdline.output_sources.iter() {
        // NOTE: inject the root path.
        file_table.insert(Path::new("/").join(file_path), write_right);
    }
    right_table.insert(Principal::Program(prog_file_abs_path.to_str().ok_or("Failed to convert program path to a string.")?.to_string()), file_table);
    info!("The final right tables: {:?}",right_table);

    let vfs = Arc::new(Mutex::new(FileSystem::new(right_table, &std_streams_table)?));
    vfs.lock()
        .map_err(|e| format!("Failed to lock vfs, error: {:?}", e))?
        .write_file_by_absolute_path(
            &Principal::InternalSuperUser,
            &prog_file_abs_path,
            &program,
            false,
        )?;
    info!("WASM program {} loaded into VFS.", prog_file_name);

    load_input_sources(&cmdline, vfs.clone())?;
    info!("Data sources loaded.");

    info!("Invoking main.");
    let main_time = Instant::now();
    let return_code = execute(&cmdline.execution_strategy, vfs.clone(), prog_file_abs_path.to_str().ok_or("Failed to convert program path to a string.")?)?;
    info!("return code: {:?}", return_code);
    info!("time: {} micro seconds", main_time.elapsed().as_micros());

    // Dump contents of stdout
    if cmdline.dump_stdout {
        let buf = vfs.lock()
            .map_err(|e| format!("Failed to lock vfs, error: {:?}", e))?
            .read_file_by_absolute_path(
                &Principal::InternalSuperUser,
                "/stdout",
            )?;
        let stdout_dump = std::str::from_utf8(&buf)
            .map_err(|e| format!("Failed to convert byte stream to UTF-8 string: {:?}", e))?;
        print!("---- stdout dump ----\n{}---- stdout dump end ----\n", stdout_dump);
    }

    // Dump contents of stderr
    if cmdline.dump_stderr {
        let buf = vfs.lock()
            .map_err(|e| format!("Failed to lock vfs, error: {:?}", e))?
            .read_file_by_absolute_path(
                &Principal::InternalSuperUser,
                "/stderr",
            )?;
        let stderr_dump = std::str::from_utf8(&buf)
            .map_err(|e| format!("Failed to convert byte stream to UTF-8 string: {:?}", e))?;
        eprint!("---- stderr dump ----\n{}---- stderr dump end ----\n", stderr_dump);
    }

    for file_path in cmdline.output_sources.iter() {
        let output = vfs.lock()
            .map_err(|e| format!("Failed to lock vfs, error: {:?}", e))?
            .read_file_by_absolute_path(
                &Principal::InternalSuperUser,
                Path::new("/").join(file_path)
            );
        info!("{}: {:?}", file_path, output);
        //TODO REMOVE
        let output : String = pinecone::from_bytes(&output?).unwrap();
        info!("{}: {:?}",file_path, output);
    }
    Ok(())
}
