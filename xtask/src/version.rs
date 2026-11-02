//! `cargo xtask version`: the next calendar version, `YYYY.WW.PATCH`.

use crate::{Result, root};
use jiff::civil::Date;
use std::fs;
use std::process::Command;

/// The ISO 8601 week-year and week of a date.
fn iso_week(date: Date) -> (i16, i8) {
    let iso = date.iso_week_date();
    (iso.year(), iso.week())
}

/// The release number of a `vYYYY.WW.N` tag for this week, or `None` for any other tag
/// (another week, a pre-release, something else).
fn release_patch(tag: &str, year: i16, week: i8) -> Option<u32> {
    let rest = tag.strip_prefix(&format!("v{year}.{week}."))?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    rest.parse().ok()
}

/// The version for a date, given the repository's existing tags.
fn next_version(date: Date, tags: &[&str]) -> String {
    let (year, week) = iso_week(date);
    let patch = tags
        .iter()
        .filter_map(|t| release_patch(t, year, week))
        .max()
        .map_or(0, |n| n + 1);
    format!("{year}.{week}.{patch}")
}

/// Sets the quoted value after `key` on the one line that starts with `line_start`.
fn set_in_line(text: &str, line_start: &str, key: &str, value: &str) -> Result<String> {
    let matching: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with(line_start) && l.contains(key))
        .collect();
    let [line] = matching.as_slice() else {
        return Err(format!(
            "expected exactly one line starting `{line_start}` with `{key}`, found {}",
            matching.len()
        ));
    };
    let start = line.find(key).expect("checked above") + key.len();
    let len = line[start..]
        .find('"')
        .ok_or_else(|| format!("unterminated value in `{line}`"))?;
    let new_line = format!("{}{value}{}", &line[..start], &line[start + len..]);
    Ok(text.replacen(line, &new_line, 1))
}

fn edit(path: &str, f: impl FnOnce(String) -> Result<String>) -> Result {
    let path = root().join(path);
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let new = f(text).map_err(|e| format!("{}: {e}", path.display()))?;
    fs::write(&path, new).map_err(|e| format!("{}: {e}", path.display()))
}

/// Sets the version in the manifests and the README, and refreshes `Cargo.lock`.
fn write(version: &str) -> Result {
    let key = "version = \"";
    edit("Cargo.toml", |t| {
        set_in_line(&t, "version = ", key, version)
    })?;
    edit("horae/Cargo.toml", |t| {
        let t = set_in_line(&t, "version = ", key, version)?;
        set_in_line(&t, "astrometry = ", key, version)
    })?;
    edit("README.md", |t| {
        let tag = format!("v{version}");
        let t = set_in_line(&t, "astrometry = { git", "tag = \"", &tag)?;
        set_in_line(&t, "horae = { git", "tag = \"", &tag)
    })?;
    let status = Command::new("cargo")
        .args(["update", "-p", "astrometry", "-p", "horae", "--offline"])
        .current_dir(root())
        .status()
        .map_err(|e| format!("cargo update: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo update failed: {status}"))
    }
}

/// The repository's existing tags for a week.
fn week_tags(date: Date) -> Result<Vec<String>> {
    let (year, week) = iso_week(date);
    let out = Command::new("git")
        .args(["tag", "--list", &format!("v{year}.{week}.*")])
        .current_dir(root())
        .output()
        .map_err(|e| format!("git tag: {e}"))?;
    if !out.status.success() {
        return Err(format!("git tag failed: {}", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

/// Prints the next version for `DATE` (default: today, UTC); with `--write`, applies it.
pub fn run(args: &[&str]) -> Result {
    let mut date = None;
    let mut write_it = false;
    for &arg in args {
        match arg {
            "--write" => write_it = true,
            _ if date.is_none() => {
                date = Some(
                    arg.parse::<Date>()
                        .map_err(|e| format!("bad date `{arg}` (want YYYY-MM-DD): {e}"))?,
                );
            }
            _ => return Err(format!("unexpected argument `{arg}`")),
        }
    }
    let date = date.unwrap_or_else(|| {
        jiff::Timestamp::now()
            .to_zoned(jiff::tz::TimeZone::UTC)
            .date()
    });
    let tags = week_tags(date)?;
    let tags: Vec<&str> = tags.iter().map(String::as_str).collect();
    let version = next_version(date, &tags);
    println!("{version}");
    if write_it {
        write(&version)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(date: &str, tags: &[&str]) -> String {
        next_version(date.parse().unwrap(), tags)
    }

    #[test]
    fn year_and_week_follow_iso_8601() {
        assert_eq!(version("2026-11-09", &[]), "2026.46.0");
        assert_eq!(version("2027-01-01", &[]), "2026.53.0");
        assert_eq!(version("2027-01-04", &[]), "2027.1.0");
        assert_eq!(version("2026-12-28", &[]), "2026.53.0");
    }

    #[test]
    fn patch_follows_the_weeks_releases() {
        let d = "2026-11-09";
        assert_eq!(version(d, &[]), "2026.46.0");
        assert_eq!(version(d, &["v2026.46.0"]), "2026.46.1");
        assert_eq!(version(d, &["v2026.46.0", "v2026.46.1"]), "2026.46.2");
        assert_eq!(version(d, &["v2026.46.1", "v2026.46.0"]), "2026.46.2");
    }

    #[test]
    fn pre_releases_and_other_weeks_do_not_count() {
        let d = "2026-11-09";
        assert_eq!(version(d, &["v2026.46.2-rc.1"]), "2026.46.0");
        assert_eq!(version(d, &["v2026.46.0", "v2026.46.2-rc.1"]), "2026.46.1");
        assert_eq!(
            version(d, &["v2026.45.3", "v2025.46.4", "v2026.4.5"]),
            "2026.46.0"
        );
        assert_eq!(
            version(d, &["v1.6.0", "v2026.46.x", "v2026.46."]),
            "2026.46.0"
        );
    }

    #[test]
    fn a_line_is_set_in_place() {
        let t = "[package]\nversion = \"1.6.0\"\nserde = { version = \"1\" }\n";
        assert_eq!(
            set_in_line(t, "version = ", "version = \"", "2026.46.0").unwrap(),
            "[package]\nversion = \"2026.46.0\"\nserde = { version = \"1\" }\n"
        );
    }

    #[test]
    fn a_missing_or_repeated_line_is_an_error() {
        assert!(set_in_line("a = 1\n", "version = ", "version = \"", "x").is_err());
        let twice = "version = \"1\"\nversion = \"2\"\n";
        assert!(set_in_line(twice, "version = ", "version = \"", "x").is_err());
    }
}
