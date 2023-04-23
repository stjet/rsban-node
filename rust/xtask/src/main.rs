use std::{fs::create_dir_all, path::{PathBuf, Path}};
use duct::cmd;
use glob::glob;

fn main() -> anyhow::Result<()>{
    let html_mode = std::env::args().any(|a| a == "--html");
    coverage(html_mode)
}

pub fn coverage(html_mode: bool) -> anyhow::Result<()> {
    let target_dir = "../build/coverage";
    fs_extra::dir::remove(target_dir).map_err(anyhow::Error::msg)?;
    create_dir_all(target_dir)?;

    println!("=== running coverage ===");
    cmd!("cargo", "test", "--lib", "-q")
        .env("CARGO_INCREMENTAL", "0")
        .env("RUSTFLAGS", "-Cinstrument-coverage")
        .env("LLVM_PROFILE_FILE", "cargo-test-%p-%m.profraw")
        .run()?;
    println!("ok.");

    println!("=== generating report ===");

    let (fmt, file) = if html_mode {
        ("html", format!("{}/html", target_dir))
    } else {
        ("lcov", format!("{}/tests.lcov", target_dir))
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