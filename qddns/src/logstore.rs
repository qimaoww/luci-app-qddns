use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::error::{Error, Result};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const LOG_DIR_MODE: u32 = 0o750;
const LOG_FILE_MODE: u32 = 0o640;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: String,
    pub scope: String,
    pub message: String,
}

pub fn append_log(base_dir: &str, scope: &str, entry: &LogEntry) -> Result<()> {
    let scope = validate_log_scope(scope)?;
    fs::create_dir_all(base_dir)?;
    set_dir_mode(base_dir, LOG_DIR_MODE)?;
    let path = PathBuf::from(base_dir).join(format!("{scope}.log"));
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    set_file_mode(&file, LOG_FILE_MODE)?;
    writeln!(
        file,
        "{}\t{}\t{}\t{}",
        entry.timestamp, entry.level, entry.scope, entry.message
    )
    .map_err(|err| Error::new(err.to_string()))?;
    Ok(())
}

pub fn read_logs(base_dir: &str, scope: Option<&str>, limit: usize) -> Result<Vec<LogEntry>> {
    let scope = validate_log_scope(scope.unwrap_or("system"))?;
    let path = PathBuf::from(base_dir).join(format!("{scope}.log"));
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut content = String::new();
    OpenOptions::new()
        .read(true)
        .open(path)?
        .read_to_string(&mut content)?;

    let mut lines = content.lines().collect::<Vec<_>>();
    if lines.len() > limit {
        lines = lines.split_off(lines.len() - limit);
    }

    let mut entries = Vec::new();
    for line in lines {
        let mut parts = line.splitn(4, '\t');
        entries.push(LogEntry {
            timestamp: parts
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .ok_or_else(|| Error::new("invalid log timestamp"))?,
            level: parts.next().unwrap_or("").to_string(),
            scope: parts.next().unwrap_or("").to_string(),
            message: parts.next().unwrap_or("").to_string(),
        });
    }

    Ok(entries)
}

pub fn ensure_valid_log_scope(scope: &str) -> Result<()> {
    validate_log_scope(scope).map(|_| ())
}

fn validate_log_scope(scope: &str) -> Result<&str> {
    if scope.is_empty() {
        return Err(Error::new("log scope must not be empty"));
    }

    if scope
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Ok(scope);
    }

    Err(Error::new(format!("invalid log scope '{scope}'")))
}

#[cfg(unix)]
fn set_dir_mode(path: &str, mode: u32) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_dir_mode(_path: &str, _mode: u32) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_file_mode(file: &std::fs::File, mode: u32) -> Result<()> {
    file.set_permissions(fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_file_mode(_file: &std::fs::File, _mode: u32) -> Result<()> {
    Ok(())
}
