use ansi_term::Colour;
use clap::Parser;
use glob::glob;
use indoc::indoc;
use log::{self, debug, error, info, warn};
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::process::Command;
use std::{collections::HashMap, io::Write};
use std::{env, fs::File};

/// Run tests for Neovim plugins
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Whether to skip checking the local clone of the external dependency is up-to-date with the remote repository
    #[arg(short, long)]
    skip_remote_check: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestDepedency {
    uri: String,
    branch: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestConfig {
    test_dependencies: Option<Vec<TestDepedency>>,
    test_paths: Option<Vec<String>>,
}

impl TestConfig {
    pub fn default() -> TestConfig {
        return TestConfig {
            test_dependencies: None,
            test_paths: None,
        };
    }
}

impl Default for TestConfig {
    fn default() -> Self {
        Self::default()
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TestDepedencyState {
    uri: String,
    hash: String,
    branch: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct State {
    test_dependencies: Vec<TestDepedencyState>,
}

impl State {
    pub fn new() -> State {
        return State {
            test_dependencies: vec![],
        };
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

fn run_test_runner() -> Result<(), Box<dyn std::error::Error>> {
    env::set_var("RUST_BACKTRACE", "1");

    let args = Args::parse();

    let current_dir = std::env::current_dir()?;

    let file_appender = FileAppender::builder()
        // Pattern: https://docs.rs/log4rs/*/log4rs/encode/pattern/index.html
        .encoder(Box::new(PatternEncoder::new(
            "[{l}] {d(%Y-%m-%d %H:%M:%S)} {m}\n",
        )))
        .build("/tmp/nvim-test-runner.log")?;

    let log_config = Config::builder()
        .appender(Appender::builder().build("file", Box::new(file_appender)))
        .build(
            Root::builder()
                .appender("file")
                .build(log::LevelFilter::Debug),
        )
        .expect("Failed to create log config");
    let _ = log4rs::init_config(log_config)?;

    log_panics::init();

    let config_path = "nvim-test-runner.json";
    let config = if let Ok(mut file) = File::open(&config_path) {
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        serde_json::from_str(&contents)?
    } else {
        println!(
            "{}",
            Colour::Yellow.paint("Config file not found, using default config")
        );
        info!("Config file not found, using default config");
        let config = TestConfig::default();
        config
    };

    // Check if state.json exists and is readable and writable, if not readable/writable, throw error
    let state_path = ".test/state.json";
    let state = if let Ok(mut file) = File::open(&state_path) {
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        serde_json::from_str(&contents)?
    } else {
        println!(
            "{}",
            Colour::Yellow.paint("State file not found, creating new state")
        );
        info!("State file not found, creating new state");
        let state = State::default();
        state
    };

    let mut new_state: State = state.clone(); // For storing the new state (and we overwrite state.json once in the end)

    let mut external_deps: Vec<std::path::PathBuf> = Vec::new();
    let mut local_deps: Vec<std::path::PathBuf> = Vec::new();

    if let Some(deps) = &config.test_dependencies {
        for dep in deps {
            debug!(
                "uri: {}, branch: {}",
                dep.uri,
                dep.branch.clone().unwrap_or("<none>".to_string()),
            );

            // Checks if url starts with "file:", if so, treat it as a local directory
            if dep.uri.starts_with("file:") {
                let path = match dep.uri.starts_with("file://") {
                    true => {
                        let abs_path = dep.uri.strip_prefix("file://").unwrap();
                        std::path::PathBuf::from(&abs_path)
                    }
                    false => {
                        let rel_path = dep.uri.strip_prefix("file:").unwrap();
                        current_dir.join(rel_path)
                    }
                };

                if !path.exists() {
                    println!(
                        "{}",
                        Colour::Yellow.paint(format!("Path {} does not exist, skipping", dep.uri))
                    );
                    warn!("Path {} does not exist, skipping", dep.uri);
                    continue;
                }
                if !path.is_dir() {
                    println!(
                        "{}",
                        Colour::Yellow.paint(format!(
                            "{} does not point to a directory, skipping",
                            dep.uri
                        ))
                    );
                    warn!("{} does not point to a directory, skipping", dep.uri);
                    continue;
                }

                info!("Path {} exists", dep.uri);
                local_deps.push(path);
                continue;
            }

            // Treating as external dependency

            let maybe_dep_name = std::path::Path::new(&dep.uri).file_name();
            if maybe_dep_name.is_none() {
                println!("{}", Colour::Red.paint(format!("Invalid uri: {}", dep.uri)));
                return Err(format!("Invalid uri: {}", dep.uri).into());
            }
            let dep_name = maybe_dep_name.unwrap().to_str().unwrap();
            let dep_path = std::path::PathBuf::from(format!(".test/external-dep/{}", dep_name));

            if !args.skip_remote_check {
                // Check if git is installed
                if let Err(_) = Command::new("git").arg("--version").output() {
                    println!("{}", Colour::Red.paint("git is not installed"));
                    return Err("git is not installed".into());
                }

                // Check if url is a valid git repository, if so,
                // get the HEAD commit hash
                let output = Command::new("git")
                    .arg("ls-remote")
                    .arg(&dep.uri)
                    .output()
                    .expect("Failed to execute git ls-remote");

                if !output.status.success() {
                    println!(
                        "{}",
                        Colour::Red.paint(format!("{} is not a valid git repository", dep.uri))
                    );
                    return Err(format!("{} is not a valid git repository", dep.uri).into());
                }

                let git_ls_remote_output = String::from_utf8_lossy(&output.stdout);
                let mut ref_name_hash_map = HashMap::new();

                for line in git_ls_remote_output.lines() {
                    let mut parts = line.split_whitespace();
                    if let (Some(hash), Some(ref_name)) = (parts.next(), parts.next()) {
                        ref_name_hash_map.insert(ref_name.to_string(), hash.to_string());
                    }
                }

                // Let ref name equals HEAD if branch is not specified, else use "refs/head/branch"
                let ref_name = match &dep.branch {
                    Some(branch) => format!("refs/heads/{}", branch),
                    None => "HEAD".to_string(),
                };
                match ref_name_hash_map.get(&ref_name) {
                    Some(hash) => {
                        // Check if state exists
                        let exists = state.test_dependencies.iter().any(|dep_state| {
                            dep_state.uri == dep.uri && dep_state.branch == dep.branch
                        });

                        let dep_path_str = format!(".test/external-dep/{}", dep_name);
                        let dep_path = std::path::Path::new(&dep_path_str);

                        if !exists && dep_path.exists() {
                            println!(
                                "{}",
                                Colour::Yellow.paint(format!(
                                    "Overwriting existing test dependency at path {}",
                                    dep_path.display()
                                ))
                            );
                            info!(
                                "Overwriting existing test dependency at path {}",
                                dep_path.display()
                            );

                            std::fs::remove_dir_all(&dep_path)?;

                            // Remove from state
                            new_state
                                .test_dependencies
                                .retain(|dep_state| dep_state.uri != dep.uri);
                        }

                        if state.test_dependencies.iter().any(|dep_state| {
                            dep_state.uri == dep.uri
                                && dep_state.branch == dep.branch
                                && dep_state.hash == *hash
                        }) {
                            external_deps.push(dep_path.to_path_buf());
                            continue;
                        }

                        println!(
                            "{}",
                            Colour::Yellow.paint(format!(
                                "Cloning repo {} @ branch {} into path {}...",
                                dep.uri,
                                dep.branch.clone().unwrap_or("HEAD".to_string()),
                                dep_path.display()
                            ))
                        );
                        info!(
                            "Cloning repository {} @ branch {} into path {}",
                            dep.uri,
                            dep.branch.clone().unwrap_or("HEAD".to_string()),
                            dep_path.display()
                        );

                        let mut cmd = Command::new("git");

                        cmd.arg("clone");

                        if let Some(branch) = &dep.branch {
                            cmd.arg("--branch").arg(branch);
                        }

                        let output = cmd
                            .arg(&dep.uri)
                            .arg(&dep_path)
                            .output()
                            .expect("Failed to execute git clone");

                        if !output.status.success() {
                            println!("{}", Colour::Red.paint("Failed to clone repository"));
                            return Err(format!(
                                "Failed to clone repository {}:\n{}",
                                dep.uri,
                                String::from_utf8_lossy(&output.stderr)
                            )
                            .into());
                        }

                        new_state.test_dependencies.push(TestDepedencyState {
                            uri: dep.uri.clone(),
                            hash: hash.clone(),
                            branch: dep.branch.clone(),
                        });
                    }
                    None => {
                        println!(
                            "{}",
                            Colour::Red.paint(format!(
                                "Branch {} does not exist in repository {}",
                                ref_name, dep.uri
                            ))
                        );
                        return Err(format!(
                            "Branch {} does not exist in repository {}",
                            ref_name, dep.uri
                        )
                        .into());
                    }
                }
            } else {
                // skip_remote_check option is off
                // Check if state exists with uri and branch
                let exists = state
                    .test_dependencies
                    .iter()
                    .any(|dep_state| dep_state.uri == dep.uri && dep_state.branch == dep.branch);
                if !exists {
                    println!(
                        "{}",
                        Colour::Red.paint(format!(
                            "State does not exist for test dependency {} @ branch {}",
                            dep.uri,
                            dep.branch.clone().unwrap_or("HEAD".to_string())
                        ))
                    );
                    return Err(format!(
                        "State does not exist for test dependency {} @ branch {}",
                        dep.uri,
                        dep.branch.clone().unwrap_or("HEAD".to_string())
                    )
                    .into());
                }
            }

            external_deps.push(dep_path.to_path_buf());
        }
    }

    // Write new_state to state.json; creating the ".texts/" directory if not already exists
    let serialized_state = serde_json::to_string(&new_state)?;
    let state_dir = std::path::Path::new(&state_path).parent().unwrap();
    std::fs::create_dir_all(state_dir)?;
    let mut file = File::create(&state_path)?;
    file.write_all(serialized_state.as_bytes())?;

    // If test_paths is not given, then default to ["tests/**/*.lua", "test/**/*.lua", "lua/tests/**/*.lua", "lua/test/**/*.lua"]
    let default_test_paths = vec![
        "tests/**/*.lua".to_string(),
        "test/**/*.lua".to_string(),
        "lua/tests/**/*.lua".to_string(),
        "lua/test/**/*.lua".to_string(),
    ];
    let test_paths = config.test_paths.unwrap_or(default_test_paths);

    for path in &test_paths {
        debug!("test path: {}", path);
    }

    let mut matched_files = Vec::new();

    for path in &test_paths {
        for entry in glob(&path)? {
            match entry {
                Ok(path) => {
                    debug!("Matched test file: {:?}", path.display());
                    matched_files.push(path);
                }
                Err(e) => error!("Error with matched file {}: {:?}", path, e),
            }
        }
    }

    let test_results: Vec<bool> = matched_files
        .par_iter()
        .map(|test| {
            debug!("Running test: {:?}", test.display());

            let mut cmd = Command::new("nvim");
            cmd.arg("--noplugin")
                .arg("--headless")
                // Disable backup and swap
                .arg("--cmd")
                .arg("set nobackup nowritebackup noswapfile")
                // Prevent shada files from being generated or read
                .arg("--cmd")
                .arg("set shada=\"NONE\"")
                // Disable viminfo
                .arg("-i")
                .arg("NONE");

            // Add plugin to runtimepath
            // Using --cmd to run vim scripts before the test file is loaded
            cmd.arg("--cmd").arg("set rtp+=.");

            debug!(
                "Adding external dependencies to runtimepath: {:?}",
                external_deps
            );

            // Add all external test dependencies to runtimepath
            for dep in &external_deps {
                cmd.arg("--cmd").arg(format!("set rtp+={}", dep.display()));
            }

            debug!("Adding local dependencies to runtimepath: {:?}", local_deps);

            for dep in &local_deps {
                cmd.arg("--cmd").arg(format!("set rtp+={}", dep.display()));
            }

            cmd.arg("-u").arg(test).arg("+qa");

            debug!("Running command: {:?}", cmd);

            let output = cmd.output().expect("Failed to execute command");

            if !output.status.success() {
                println!(
                    "{}",
                    Colour::Red.paint(format!("Failed to run test {}", test.display()))
                );
                error!("Failed to run command: {:?}", cmd);
                return false;
            }

            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.len() > 0 {
                print!(
                    "{}",
                    Colour::Red.paint(format!(
                        indoc! {"
                        x {}
                        {}
                    "},
                        test.display(),
                        stderr
                    ))
                );
                false
            } else {
                println!("{}", Colour::Blue.paint(format!("✓ {}", test.display())));
                true
            }
        })
        .collect();

    // Count the number of failed tests
    let num_failed_tests = test_results.into_iter().filter(|x| !x).count();
    if num_failed_tests > 0 {
        println!(
            "{}",
            Colour::Red.paint(format!("{} test(s) failed", num_failed_tests))
        );
        std::process::exit(1);
    }

    Ok(())
}

fn main() {
    if let Err(e) = run_test_runner() {
        println!("{}", Colour::Red.paint(format!("{}", e)));
        error!("{}", e);
        std::process::exit(2);
    }
}
