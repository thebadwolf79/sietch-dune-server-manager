//! Append-only operation log file with simple size-based rotation.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Manager};

const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;
const LOG_FILE_NAME: &str = "operation.log";
const ROLLED_FILE_NAME: &str = "operation.log.1";

#[derive(Debug, Serialize)]
struct LogLine<'a> {
    ts: String,
    level: &'a str,
    scope: &'a str,
    message: &'a str,
}

/// JSON-line append-only sink for operation logs.
pub struct LogFile {
    dir: PathBuf,
    path: PathBuf,
    file: Mutex<File>,
}

impl LogFile {
    /// Resolves the app's local log directory, creates it if missing, and
    /// opens `operation.log` for append. Errors out only if the directory
    /// cannot be created or the file cannot be opened.
    pub fn new(app: &AppHandle) -> std::io::Result<Self> {
        let dir = app
            .path()
            .app_log_dir()
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))?;
        fs::create_dir_all(&dir)?;
        let path = dir.join(LOG_FILE_NAME);
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            dir,
            path,
            file: Mutex::new(file),
        })
    }

    /// Returns the directory the log file lives in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Appends a single JSON-line entry. Errors are swallowed by callers
    /// because the live in-memory log view is the source of truth.
    pub fn append(&self, level: &str, scope: &str, message: &str) -> std::io::Result<()> {
        let line = LogLine {
            ts: iso_timestamp(),
            level,
            scope,
            message,
        };
        let mut text = serde_json::to_string(&line)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))?;
        text.push('\n');
        let mut file = self
            .file
            .lock()
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))?;
        file.write_all(text.as_bytes())?;
        self.maybe_rotate_locked(&mut file)?;
        Ok(())
    }

    fn maybe_rotate_locked(&self, file: &mut File) -> std::io::Result<()> {
        let len = file.metadata()?.len();
        if len < MAX_LOG_BYTES {
            return Ok(());
        }
        // Drop the file handle before renaming on Windows.
        drop(std::mem::replace(
            file,
            OpenOptions::new().read(true).open(&self.path)?,
        ));
        let rolled = self.dir.join(ROLLED_FILE_NAME);
        let _ = fs::remove_file(&rolled);
        fs::rename(&self.path, &rolled)?;
        *file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        Ok(())
    }
}

fn iso_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let millis = now.subsec_millis();
    // Minimal ISO-8601 UTC formatter without bringing in chrono.
    let days_from_epoch = (secs / 86_400) as i64;
    let (year, month, day) = civil_from_days(days_from_epoch);
    let seconds_in_day = secs % 86_400;
    let hour = (seconds_in_day / 3600) as u32;
    let minute = ((seconds_in_day / 60) % 60) as u32;
    let second = (seconds_in_day % 60) as u32;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// Converts days-since-1970 to a (year, month, day) Gregorian triple.
/// Based on Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 {
        (mp + 3) as u32
    } else {
        (mp - 9) as u32
    };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
