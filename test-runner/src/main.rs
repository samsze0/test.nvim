use ansi_term::Colour;
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
use std::collections::HashMap;
use std::io::Read;
use std::process::Command;
use std::{env, fs::File};

#[derive(Serialize, Deserialize)]
struct TestDepedency {
    url: Option<String>,
    branch: Option<String>,
    dir: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct TestConfig {
    test_dependencies: Option<Vec<TestDepedency>>,
    test_paths: Option<Vec<String>>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env::set_var("RUST_BACKTRACE", "1");

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

    let mut file = File::open("nvim-test-runner.json")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: TestConfig = serde_json::from_str(&contents)?;

    let mut external_deps: Vec<std::path::PathBuf> = Vec::new();
    let mut local_deps: Vec<std::path::PathBuf> = Vec::new();

    if let Some(deps) = &config.test_dependencies {
        for dep in deps {
            debug!(
                "url: {}, branch: {}, dir: {}",
                dep.url.clone().unwrap_or("none".to_string()),
                dep.branch.clone().unwrap_or("none".to_string()),
                dep.dir.clone().unwrap_or("none".to_string())
            );

            match (&dep.dir, &dep.url) {
                (None, None) => {
                    warn!("Neither url or dir is specified, skipping");
                    continue;
                }
                (Some(dir), _) => {
                    if dep.url.is_some() {
                        warn!("dir takes precedence over url");
                    }

                    // Check if dir exists and is a directory
                    let path = std::path::Path::new(dir);
                    if !path.exists() {
                        warn!("Directory {} does not exist, skipping", dir);
                        continue;
                    }
                    if !path.is_dir() {
                        warn!("{} is not a directory, skipping", dir);
                        continue;
                    }

                    info!("Directory {} exists", dir);
                    local_deps.push(path.to_path_buf());
                }
                (None, Some(url)) => {
                    let maybe_dep_name = std::path::Path::new(url).file_name();
                    if maybe_dep_name.is_none() {
                        panic!("Invalid url: {}", url);
                    }
                    let dep_name = maybe_dep_name.unwrap().to_str().unwrap();

                    // Check if git is installed
                    if let Err(_) = Command::new("git").arg("--version").output() {
                        panic!("git is not installed");
                    }

                    // Check if url is a valid git repository, if so,
                    // get the HEAD commit hash
                    let url = url.clone();
                    let output = Command::new("git")
                        .arg("ls-remote")
                        .arg(&url)
                        .output()
                        .expect("Failed to execute git ls-remote");

                    if !output.status.success() {
                        panic!("URL {} is not a valid git repository", url);
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
                            // Check if repo already exists in the ".test/external-dep" directory
                            let dep_path_str = format!(".test/external-dep/{}-{}", dep_name, hash);
                            let dep_path = std::path::Path::new(&dep_path_str);
                            if dep_path.exists() {
                                debug!(
                                    "Test dependency {} already exists in {}. Skip cloning",
                                    url,
                                    dep_path.display()
                                );
                            } else {
                                info!(
                                    "Cloning repository {} with branch {}",
                                    url,
                                    dep.branch.clone().unwrap_or("HEAD".to_string())
                                );
                                println!(
                                    "{}",
                                    Colour::Yellow.paint(format!(
                                        "Cloning repo {} @ branch {}...",
                                        url,
                                        dep.branch.clone().unwrap_or("HEAD".to_string())
                                    ))
                                );

                                let mut cmd = Command::new("git");

                                cmd.arg("clone");

                                if let Some(branch) = &dep.branch {
                                    cmd.arg("--branch").arg(branch);
                                }

                                let output = cmd
                                    .arg(&url)
                                    .arg(&dep_path)
                                    .output()
                                    .expect("Failed to execute git clone");

                                if !output.status.success() {
                                    panic!("Failed to clone repository {}", url);
                                }
                            }

                            external_deps.push(dep_path.to_path_buf());
                        }
                        None => {
                            panic!("Branch {} does not exist in repository {}", ref_name, url);
                        }
                    }
                }
            }
        }
    }

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
            cmd.arg("--headless")
                .arg("--noplugin")
                // Prevent shada files from being generated or read
                .arg("--cmd")
                .arg("set shada=\"NONE\"");

            // Add plugin to runtimepath
            // Using --cmd to run vim scripts before the test file is loaded
            cmd.arg("--cmd").arg("set rtp+=.");

            // Add all external test dependencies to runtimepath
            for dep in &external_deps {
                cmd.arg("--cmd").arg(format!("set rtp+={}", dep.display()));
            }

            cmd.arg("-u").arg(test).arg("+qa");

            debug!("Running command: {:?}", cmd);

            let output = cmd.output().expect("Failed to execute command");

            if !output.status.success() {
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
        Err(format!("{} test(s) failed", num_failed_tests))?;
    }

    Ok(())
}
