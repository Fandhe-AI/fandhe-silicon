//! MSL を offline compiler で metallib にして、作業のディレクトリごと持つ module（TASK-1009 のポインタ）。macOS 向けだけ。
//!
//! 呼び出し元は `run_g7` だけ。ツールのパスと入出力のファイル名は固定で、外からパス・バイト列を受ける口は無い。
//! `unsafe` は書かない。作業のディレクトリは [`Built`] が持ち、drop で消える。

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use fandhe_silicon_core::hash::sha256_bytes;

use crate::ArmError;
use crate::coopmat::{COOPMAT_MSL, METALLIB_FILE_NAME, MSL_FILE_NAME, compile_commands};

/// offline compiler の絶対パス。
const XCRUN: &str = "/usr/bin/xcrun";

/// 読み込む metallib の長さの上限（バイト）。
const METALLIB_LIMIT: u64 = 16 * 1024 * 1024;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// 自分で作った作業のディレクトリ。drop で消す（消すのはこのディレクトリだけ）。
struct WorkDir {
    path: PathBuf,
}

impl WorkDir {
    /// `parent` の下の `name` を非再帰で作る。既にあれば失敗する。
    fn create_in(parent: &Path, name: &str) -> Result<Self, ArmError> {
        let path = parent.join(name);
        DirBuilder::new()
            .recursive(false)
            .mode(0o700)
            .create(&path)
            .map_err(|_| ArmError::WorkDirFailed)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// 作った metallib とその作業のディレクトリ。生きている間だけファイルが残る。
pub(crate) struct Built {
    _dir: WorkDir,
    metallib: PathBuf,
    sha256: [u8; 32],
}

impl Built {
    /// metallib のパス。
    pub(crate) fn metallib(&self) -> &Path {
        &self.metallib
    }

    /// metallib のバイト列の sha256。
    pub(crate) fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
}

/// 一意の名前で作業のディレクトリを作り、MSL を書いて 2 つのコマンドで metallib を作る。
pub(crate) fn build_metallib() -> Result<Built, ArmError> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let name = format!(
        "measure-metal-gemm-arm-{}-{}-{}",
        std::process::id(),
        nanos,
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let dir = WorkDir::create_in(&std::env::temp_dir(), &name)?;

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(dir.path().join(MSL_FILE_NAME))
        .map_err(|_| ArmError::WorkDirFailed)?;
    file.write_all(COOPMAT_MSL.as_bytes())
        .map_err(|_| ArmError::WorkDirFailed)?;
    drop(file);

    for args in compile_commands(dir.path()) {
        let status = Command::new(XCRUN)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| ArmError::ToolFailed)?;
        if !status.success() {
            return Err(ArmError::ToolFailed);
        }
    }

    let metallib = dir.path().join(METALLIB_FILE_NAME);
    let bytes = read_limited(&metallib, METALLIB_LIMIT)?;
    let sha256 = sha256_bytes(&bytes);
    Ok(Built {
        _dir: dir,
        metallib,
        sha256,
    })
}

/// シンボリックリンクでない通常のファイルを、`limit` バイトまでの上限つきで読む。
fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, ArmError> {
    let meta = fs::symlink_metadata(path).map_err(|_| ArmError::WorkDirFailed)?;
    if !meta.is_file() || meta.len() > limit {
        return Err(ArmError::WorkDirFailed);
    }
    let file = File::open(path).map_err(|_| ArmError::WorkDirFailed)?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ArmError::WorkDirFailed)?;
    if bytes.len() as u64 > limit {
        return Err(ArmError::WorkDirFailed);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique(tag: &str) -> String {
        format!(
            "measure-metal-gemm-arm-test-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        )
    }

    /// 作業のディレクトリは作ると在り、drop で消える。
    #[test]
    fn work_dir_exists_then_is_removed_on_drop() {
        let name = unique("drop");
        let path = std::env::temp_dir().join(&name);
        let dir = WorkDir::create_in(&std::env::temp_dir(), &name).expect("create");
        assert!(path.is_dir());
        drop(dir);
        assert!(!path.exists());
    }

    /// 同じ名前で 2 回作ると 2 回目は失敗し、1 回目のディレクトリは残る。
    #[test]
    fn work_dir_refuses_an_existing_name() {
        let name = unique("twice");
        let first = WorkDir::create_in(&std::env::temp_dir(), &name).expect("first");
        let second = WorkDir::create_in(&std::env::temp_dir(), &name);
        assert!(matches!(second, Err(ArmError::WorkDirFailed)));
        assert!(first.path().is_dir());
    }

    /// 上限を超えるファイルは読まずに拒否する。
    #[test]
    fn read_limited_rejects_oversize() {
        let name = unique("limit");
        let dir = WorkDir::create_in(&std::env::temp_dir(), &name).expect("create");
        let file = dir.path().join("x.bin");
        fs::write(&file, [0_u8; 9]).expect("write");
        assert_eq!(read_limited(&file, 8), Err(ArmError::WorkDirFailed));
        assert_eq!(read_limited(&file, 9).expect("ok").len(), 9);
    }
}
