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

    if let Some(deps) = &config.test_dependencies {
        for dep in deps {
            debug!(
                "url: {}, branch: {}, dir: {}",
                dep.url.clone().unwrap_or("none".to_string()),
                dep.branch.clone().unwrap_or("none".to_string()),
                dep.dir.clone().unwrap_or("none".to_string())
            );
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

    matched_files.par_iter().for_each(|test| {
        debug!("Running test: {:?}", test.display());

        let mut cmd = Command::new("nvim");
        cmd.arg("--headless")
            .arg("--noplugin")
            .arg(format!("-u {}", test.display()))
            .arg("-c qa!");

        debug!("Running command: {:?}", cmd);

        let output = cmd.output().expect("Failed to execute command");

        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            info!(
                indoc! {"
                Successfully ran command: {:?}
                Command output:
                {}"},
                cmd, s
            );
        } else {
            let s = String::from_utf8_lossy(&output.stderr);
            warn!(
                indoc! {"
                Failed to run command: {:?}
                Command output:
                {}"},
                cmd, s
            );
        }
    });

    Ok(())
}
