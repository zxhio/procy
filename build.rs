use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("build_info.rs");
    let mut f = File::create(&dest_path).unwrap();

    let ci = match commit_info_from_git() {
        Some(git) => git,
        None => CommitInfo {
            hash: String::from("unknown"),
            short_hash: String::from("unknown"),
            date: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        },
    };

    let version = env::var("BUILD_VERSION").unwrap_or(String::from("unknown"));

    writeln!(f, "pub const COMMIT_HASH: &str = \"{}\";", ci.hash).unwrap();
    writeln!(
        f,
        "pub const COMMIT_SHORT_HASH: &str = \"{}\";",
        ci.short_hash
    )
    .unwrap();
    writeln!(f, "pub const COMMIT_DATE: &str = \"{}\";", ci.date).unwrap();
    writeln!(
        f,
        "pub const COMMIT_VERSION_INFO: &str = \"{} ({} {})\";",
        version, ci.short_hash, ci.date,
    )
    .unwrap();
}

struct CommitInfo {
    hash: String,
    short_hash: String,
    date: String,
}

fn commit_info_from_git() -> Option<CommitInfo> {
    if !Path::new(".git").exists() {
        return None;
    }

    let output = match std::process::Command::new("git")
        .arg("log")
        .arg("-1")
        .arg("--date=short")
        .arg("--format=%H %h %cd")
        .arg("--abbrev=9")
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return None,
    };

    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut parts = stdout.split_whitespace().map(|s| s.to_string());

    Some(CommitInfo {
        hash: parts.next()?,
        short_hash: parts.next()?,
        date: parts.next()?,
    })
}
