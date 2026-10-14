//! Fail-closed durable spend journal. Stale locks require explicit reconciliation.
use crate::agent_protocol::SpendLedger;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub struct BudgetGuard {
    file: File,
    lock: PathBuf,
    record: PathBuf,
    ledger: SpendLedger,
    poisoned: bool,
}
impl BudgetGuard {
    pub fn acquire(
        directory: &Path,
        provider_and_account: &str,
    ) -> Result<Self, String> {
        if provider_and_account.is_empty() {
            return Err("Budget account identity required".into());
        }
        fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let m = fs::symlink_metadata(directory).map_err(|e| e.to_string())?;
        if !m.is_dir() || m.file_type().is_symlink() {
            return Err("Unsafe budget directory".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        let id = format!("{:x}", Sha256::digest(provider_and_account.as_bytes()));
        let lock = directory.join(format!("{id}.lock"));
        let record = directory.join(format!("{id}.json"));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file=options.open(&lock).map_err(|_|"Budget is locked. Another process may be using it, or an interrupted run needs reconciliation.".to_string())?;
        if let Err(e) =
            write!(file, "{}", std::process::id()).and_then(|_| file.sync_all())
        {
            let _ = fs::remove_file(&lock);
            return Err(e.to_string());
        }
        let loaded = (|| match fs::symlink_metadata(&record) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(SpendLedger::default())
            }
            Err(e) => Err(e.to_string()),
            Ok(m) => {
                if !m.is_file()
                    || m.file_type().is_symlink()
                    || m.len() > 1024 * 1024
                {
                    return Err("Unsafe or oversized budget journal".into());
                }
                let mut bytes = Vec::new();
                File::open(&record)
                    .map_err(|e| e.to_string())?
                    .take(1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if bytes.len() > 1024 * 1024 {
                    return Err("Budget journal too large".into());
                }
                serde_json::from_slice(&bytes).map_err(|e| {
                    format!("Budget journal unreadable; spending blocked: {e}")
                })
            }
        })();
        match loaded {
            Ok(ledger) => Ok(Self {
                file,
                lock,
                record,
                ledger,
                poisoned: false,
            }),
            Err(e) => {
                drop(file);
                let _ = fs::remove_file(lock);
                Err(e)
            }
        }
    }
    fn commit(&mut self, ledger: SpendLedger) -> Result<(), String> {
        let bytes = serde_json::to_vec(&ledger).map_err(|e| e.to_string())?;
        if bytes.len() > 1024 * 1024 {
            return Err("Budget journal full".into());
        }
        let tmp = self.record.with_extension("pending");
        // A left-over temp file is not automatically discarded or mistaken for no spend.
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&tmp)
            .map_err(|e| format!("Budget write blocked: {e}"))?;
        let outcome = (|| {
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
            fs::rename(&tmp, &self.record).map_err(|e| e.to_string())?;
            // Linux directory sync makes rename durable before any billable IO starts.
            #[cfg(unix)]
            File::open(self.record.parent().ok_or("No journal parent")?)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
            Ok(())
        })();
        if outcome.is_err() {
            self.poisoned = true;
            return outcome;
        }
        self.ledger = ledger;
        Ok(())
    }
    pub fn reserve(
        &mut self,
        id: &str,
        max: u64,
        request_limit: u64,
        lifetime_limit: u64,
    ) -> Result<(), String> {
        if self.poisoned {
            return Err(
                "Budget journal write failed; reconciliation required".into()
            );
        }
        let mut ledger = self.ledger.clone();
        ledger.reserve(id, max, request_limit, lifetime_limit)?;
        self.commit(ledger)
    }
    pub fn settle(&mut self, id: &str, actual: Option<u64>) -> Result<(), String> {
        if self.poisoned {
            return Err(
                "Budget journal write failed; reconciliation required".into()
            );
        }
        let mut ledger = self.ledger.clone();
        let result = ledger.settle(id, actual);
        self.commit(ledger)?;
        result
    }
    pub fn charged(&self) -> u64 {
        self.ledger.charged()
    }
    pub fn reserved(&self) -> u64 {
        self.ledger.pending()
    }
}
impl Drop for BudgetGuard {
    fn drop(&mut self) {
        let _ = self.file.sync_all();
        let _ = fs::remove_file(&self.lock);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lock_and_restart_reservations() {
        let d = tempfile::tempdir().unwrap();
        let mut g = BudgetGuard::acquire(d.path(), "openai:key1").unwrap();
        assert!(BudgetGuard::acquire(d.path(), "openai:key1").is_err());
        g.reserve("a", 60, 60, 100).unwrap();
        drop(g);
        let mut g = BudgetGuard::acquire(d.path(), "openai:key1").unwrap();
        assert_eq!(g.reserved(), 60);
        assert!(g.reserve("b", 50, 50, 100).is_err());
        g.settle("a", None).unwrap();
        drop(g);
        let g = BudgetGuard::acquire(d.path(), "openai:key1").unwrap();
        assert_eq!(g.charged(), 60);
    }
    #[test]
    fn corrupt_record_fails_closed() {
        let d = tempfile::tempdir().unwrap();
        let g = BudgetGuard::acquire(d.path(), "test").unwrap();
        let p = g.record.clone();
        drop(g);
        fs::write(&p, "broken").unwrap();
        assert!(BudgetGuard::acquire(d.path(), "test").is_err());
        assert_eq!(fs::read_to_string(p).unwrap(), "broken");
    }
    #[test]
    fn accounts_partitioned() {
        let d = tempfile::tempdir().unwrap();
        let a = BudgetGuard::acquire(d.path(), "a").unwrap();
        let b = BudgetGuard::acquire(d.path(), "b").unwrap();
        assert_ne!(a.record, b.record);
    }
    #[test]
    fn overrun_persisted() {
        let d = tempfile::tempdir().unwrap();
        let mut g = BudgetGuard::acquire(d.path(), "a").unwrap();
        g.reserve("a", 5, 5, 100).unwrap();
        assert!(g.settle("a", Some(6)).is_err());
        drop(g);
        let mut g = BudgetGuard::acquire(d.path(), "a").unwrap();
        assert!(g.reserve("b", 1, 1, 100).is_err());
    }
    #[test]
    fn stale_lock_blocks() {
        let d = tempfile::tempdir().unwrap();
        let g = BudgetGuard::acquire(d.path(), "a").unwrap();
        let lock = g.lock.clone();
        drop(g);
        fs::write(lock, "old process").unwrap();
        assert!(BudgetGuard::acquire(d.path(), "a").is_err());
    }
}
