//! `cargo xtask check`: the CI matrix (.github/workflows/tests.yml), locally, from
//! a clean and cleaned again at the end.

use std::process::Command;

use crate::{Result, root};

const IOS: [&str; 2] = ["aarch64-apple-ios", "aarch64-apple-ios-sim"];

/// One step of the matrix: its header, and the command line after `cargo`.
struct Step {
    name: String,
    args: Vec<String>,
}

fn step(name: &str, args: &[&str]) -> Step {
    Step {
        name: name.to_string(),
        args: args.iter().map(|a| a.to_string()).collect(),
    }
}

pub fn run(args: &[&str]) -> Result {
    if let Some(a) = args.first() {
        return Err(format!("check: unknown argument {a}"));
    }
    preflight()?;
    let result = clean().and_then(|()| matrix());
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
    exec("cargo clean", &["clean"])
}

fn matrix() -> Result {
    let test = step(
        "test (all features)",
        &["test", "--workspace", "--all-features", "--locked"],
    );
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
        exec(&s.name, &args)?;
    }
    Ok(())
}

/// Runs `cargo` with inherited stdio, under a header line, with `ASTROMETRY_DE440` cleared: the
/// tests that need the full file skip, and the fixture's run.
fn exec(name: &str, args: &[&str]) -> Result {
    println!("==> {name}: cargo {}", args.join(" "));
    let mut cmd = Command::new("cargo");
    cmd.args(args).current_dir(root());
    cmd.env_remove("ASTROMETRY_DE440");
    let status = cmd.status().map_err(|e| format!("{name}: cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("check failed at \"{name}\": {status}"))
    }
}
