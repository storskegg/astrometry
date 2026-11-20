//! `cargo xtask check [--no-jpl]`: the CI matrix (.github/workflows/tests.yml), locally, from
//! a clean and cleaned again at the end.

use std::path::Path;
use std::process::Command;

use crate::{Result, root};

const DE440_URL: &str = "https://ssd.jpl.nasa.gov/ftp/eph/planets/bsp/de440.bsp";
const IOS: [&str; 2] = ["aarch64-apple-ios", "aarch64-apple-ios-sim"];

/// One step of the matrix: its header, and the command line after `cargo`.
struct Step {
    name: String,
    args: Vec<String>,
    /// Whether it gets `ASTROMETRY_DE440`.
    jpl: bool,
}

fn step(name: &str, args: &[&str]) -> Step {
    Step {
        name: name.to_string(),
        args: args.iter().map(|a| a.to_string()).collect(),
        jpl: false,
    }
}

pub fn run(args: &[&str]) -> Result {
    let mut jpl = true;
    for &a in args {
        match a {
            "--no-jpl" => jpl = false,
            _ => return Err(format!("check: unknown argument {a}")),
        }
    }
    preflight()?;
    let de440 = root().join("local/jpl/de440.bsp");
    let result = clean().and_then(|()| {
        if jpl {
            ensure_de440(&de440)?;
        }
        matrix(jpl.then_some(de440.as_path()))
    });
    // The final clean runs whatever happened.
    let cleaned = clean();
    result.and(cleaned)
}

/// Fails early, naming the `rustup` command, if a target or toolchain is missing.
fn preflight() -> Result {
    let list = |args: &[&str]| -> Result<String> {
        let out = Command::new("rustup")
            .args(args)
            .output()
            .map_err(|e| format!("rustup: {e}"))?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let toolchains = list(&["toolchain", "list"])?;
    if !toolchains.lines().any(|l| l.starts_with("1.99")) {
        return Err("the MSRV toolchain is missing: run `rustup toolchain install 1.99`".into());
    }
    let targets = list(&["target", "list", "--installed"])?;
    for t in ["wasm32-unknown-unknown", IOS[0], IOS[1]] {
        if !targets.lines().any(|l| l.trim() == t) {
            return Err(format!(
                "target {t} is missing: run `rustup target add {t}`"
            ));
        }
    }
    Ok(())
}

fn clean() -> Result {
    exec("cargo clean", &["clean"], None)
}

fn ensure_de440(path: &Path) -> Result {
    if path.exists() {
        return Ok(());
    }
    println!("==> download de440.bsp");
    std::fs::create_dir_all(path.parent().expect("a file path"))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let status = Command::new("curl")
        .args(["-sS", "--fail", "-L", "-o"])
        .arg(path)
        .arg(DE440_URL)
        .status()
        .map_err(|e| format!("curl: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        // Don't leave a partial file that would pass for the real one.
        let _ = std::fs::remove_file(path);
        Err(format!("download de440.bsp: {DE440_URL}: {status}"))
    }
}

fn matrix(de440: Option<&Path>) -> Result {
    let mut test = step(
        "test (all features)",
        &["test", "--workspace", "--all-features", "--locked"],
    );
    test.jpl = true;
    let mut steps = vec![
        step("fmt", &["fmt", "--all", "--check"]),
        step(
            "clippy (all features)",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--locked",
                "--",
                "-D",
                "warnings",
            ],
        ),
        step(
            "clippy (no default features)",
            &[
                "clippy",
                "-p",
                "astrometry",
                "-p",
                "horae",
                "--all-targets",
                "--no-default-features",
                "--locked",
                "--",
                "-D",
                "warnings",
            ],
        ),
        step(
            "check horae for wasm32",
            &[
                "check",
                "-p",
                "horae",
                "--no-default-features",
                "--target",
                "wasm32-unknown-unknown",
                "--locked",
            ],
        ),
        test,
        step(
            "test slice (no default features)",
            &[
                "test",
                "-p",
                "astrometry",
                "--no-default-features",
                "--lib",
                "--locked",
                "slice::tests",
            ],
        ),
        step(
            "MSRV 1.99",
            &[
                "+1.99",
                "check",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--locked",
            ],
        ),
    ];
    for t in IOS {
        for (label, flag) in [
            ("default features", None),
            ("no default features", Some("--no-default-features")),
            ("all features", Some("--all-features")),
        ] {
            let mut args = vec!["build", "-p", "astrometry", "-p", "horae", "--target", t];
            args.extend(flag);
            args.push("--locked");
            steps.push(step(&format!("build for {t}, {label}"), &args));
        }
    }
    for s in steps {
        let args: Vec<&str> = s.args.iter().map(String::as_str).collect();
        exec(&s.name, &args, if s.jpl { de440 } else { None })?;
    }
    Ok(())
}

/// Runs `cargo` with inherited stdio, under a header line; `de440` sets `ASTROMETRY_DE440`,
/// and without it the variable is cleared.
fn exec(name: &str, args: &[&str], de440: Option<&Path>) -> Result {
    println!("==> {name}: cargo {}", args.join(" "));
    let mut cmd = Command::new("cargo");
    cmd.args(args).current_dir(root());
    match de440 {
        Some(p) => cmd.env("ASTROMETRY_DE440", p),
        None => cmd.env_remove("ASTROMETRY_DE440"),
    };
    let status = cmd.status().map_err(|e| format!("{name}: cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("check failed at \"{name}\": {status}"))
    }
}
