use duct::cmd;
use glob::glob;
use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
};

fn main() -> anyhow::Result<()> {
    let html_mode = std::env::args().any(|a| a == "--html");
    coverage(html_mode)
}

const TARGET_DIR: &str = "../build/coverage/";

pub fn coverage(html_mode: bool) -> anyhow::Result<()> {
    if html_mode {
        fs_extra::dir::remove(format!("{}/html", TARGET_DIR)).map_err(anyhow::Error::msg)?;
    }
    create_dir_all(TARGET_DIR)?;

    println!("=== running coverage ===");
    cmd!("cargo", "test", "--lib", "-q")
        .env("CARGO_INCREMENTAL", "0")
        .env("RUSTFLAGS", "-Cinstrument-coverage")
        .env("LLVM_PROFILE_FILE", "cargo-test-%p-%m.profraw")
        .run()?;
    println!("ok.");

    println!("=== generating report ===");

    let (fmt, file) = if html_mode {
        ("html", TARGET_DIR.to_string())
    } else {
        ("lcov", format!("{}/tests.lcov", TARGET_DIR))
    };

    cmd!(
        "grcov",
        ".",
        "--binary-path",
        "../build/cargo/debug/deps",
        "-s",
        "..",
        "-t",
        fmt,
        "--ignore-not-existing",
        "--ignore",
        "rust/ffi/*",
        "--ignore",
        "../*",
        "--ignore",
        "/*",
        "--ignore",
        "rust/xtask/*",
        "--ignore",
        "*/src/tests/*",
        "-o",
        file.as_str(),
    )
    .run()?;
    println!("ok.");

    println!("=== cleaning up ===");
    clean_files("**/*.profraw")?;
    println!("ok.");
    println!("report location: {file}");

    Ok(())
}

pub fn clean_files(pattern: &str) -> anyhow::Result<()> {
    let files: Result<Vec<PathBuf>, _> = glob(pattern)?.collect();
    files?.iter().try_for_each(remove_file)
}

pub fn remove_file<P>(path: P) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    fs_extra::file::remove(path).map_err(anyhow::Error::msg)
}
