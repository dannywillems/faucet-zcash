//! Single-writer lock for the wallet database.
//!
//! The wallet SQLite database must be used by at most one signer process at a
//! time (concurrent writers corrupt SQLite and the note-tracking state). When
//! the database is mounted into multiple containers, only one may hold the
//! wallet; the others must fail fast and let the orchestrator restart them until
//! the holder releases the lock.
//!
//! This uses an advisory `flock(2)` on a sibling lock file (`<db>.lock`). flock
//! is held for as long as the file descriptor is open, so the kernel releases it
//! automatically when the process exits (cleanly OR on crash). There is no stale
//! lock left behind: the lock file may remain on disk, but the lock itself is a
//! kernel state on the open descriptor, not the file's existence. Dropping
//! [`WalletLock`] (or exiting the process) releases it.

use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;

/// Holds the wallet database lock for the lifetime of the value. The lock is
/// released when this is dropped or when the process exits.
#[derive(Debug)]
#[must_use = "the lock is released as soon as this value is dropped"]
pub struct WalletLock {
    // Held only to keep the descriptor (and thus the flock) alive.
    _file: File,
    path: String,
}

impl WalletLock {
    /// The lock file path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Drop for WalletLock {
    fn drop(&mut self) {
        // Closing the descriptor releases the flock; do it explicitly so the
        // lock is given up the moment the process no longer needs the wallet.
        let fd = self._file.as_raw_fd();
        // Safety: fd is a valid open descriptor owned by `_file`.
        unsafe {
            libc::flock(fd, libc::LOCK_UN);
        }
    }
}

/// Why acquiring the wallet lock failed.
#[derive(Debug)]
pub enum LockError {
    /// Another process currently holds the wallet lock.
    Held(String),
    /// The lock file could not be opened or locked for another reason.
    Io(String),
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Held(p) => {
                write!(
                    f,
                    "wallet database is already locked by another process ({p})"
                )
            }
            Self::Io(e) => write!(f, "could not acquire wallet lock: {e}"),
        }
    }
}

/// Try to take an exclusive, non-blocking lock guarding `db_path`.
///
/// Returns [`LockError::Held`] if another process holds it (the caller should
/// exit non-zero so the orchestrator retries), or [`LockError::Io`] on any other
/// failure.
pub fn acquire(db_path: &str) -> Result<WalletLock, LockError> {
    let lock_path = format!("{db_path}.lock");
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| LockError::Io(format!("open {lock_path}: {e}")))?;

    let fd = file.as_raw_fd();
    // LOCK_EX (exclusive) | LOCK_NB (do not block; fail immediately if held).
    // Safety: fd is a valid open descriptor owned by `file`.
    let rc = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        return match err.raw_os_error() {
            Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN => {
                Err(LockError::Held(lock_path))
            }
            _ => Err(LockError::Io(format!("flock {lock_path}: {err}"))),
        };
    }

    Ok(WalletLock {
        _file: file,
        path: lock_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_is_rejected_then_released_on_drop() {
        let dir = std::env::temp_dir();
        let db = dir.join(format!("faucet-dblock-test-{}.db", std::process::id()));
        let db = db.to_str().unwrap().to_string();

        let first = acquire(&db).expect("first lock should succeed");
        // A second attempt in-process also contends on the same lock file and
        // must be rejected while the first is held.
        match acquire(&db) {
            Err(LockError::Held(_)) => {}
            other => panic!("expected Held, got {other:?}"),
        }
        drop(first);

        // Once released, it can be taken again.
        let again = acquire(&db).expect("lock should be free after drop");
        drop(again);
        let _ = std::fs::remove_file(format!("{db}.lock"));
    }
}
