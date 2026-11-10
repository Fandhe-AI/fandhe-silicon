//! 実行時に環境変数で受ける WGSL のソースを照らして受け取る口（spec TASK-1010 のポインタ）。
//!
//! 後続の device の経路が、ここを通ったテキストだけを受け取る。デバイスにもコンパイラにも触れない。
//! 検査は長さの上限、sha256 の照合、UTF-8、naga の解析・検証、入口と束縛の形の照合の順で、どれも失敗側に倒す。
//! 失敗の型は値を持たず、パス・sha256・ソースの断片を文言に出さない。

use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use fandhe_silicon_core::hash::sha256_bytes;
use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
use wgpu::naga::{AddressSpace, ArraySize, Module, Scalar, ShaderStage, StorageAccess, TypeInner};

/// ソースのファイルのパスを受ける環境変数の名前。
pub const KERNEL_SOURCE_ENV: &str = "FANDHE_SILICON_KERNEL_SOURCE";

/// ソースの sha256（64 桁の小文字の 16 進）を受ける環境変数の名前。
pub const KERNEL_SHA256_ENV: &str = "FANDHE_SILICON_KERNEL_SHA256";

/// 読むソースの最大バイト数。超えるファイルは照合の前に拒否する。
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;

/// 受け取る WGSL の compute の入口が持つ workgroup の大きさ。
pub const TILED_WORKGROUP_SIZE: [u32; 3] = [16, 16, 1];

/// 受け取りの失敗。どの段かだけを表し、値は含まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExternalWgslError {
    /// 2 つの値のどちらかが無い。
    Missing,
    /// sha256 が 64 桁の小文字の 16 進でない。
    InvalidDigest,
    /// 通常のファイルとして読めない。
    Unreadable,
    /// 上限を超える。
    TooLarge,
    /// sha256 が一致しない。
    DigestMismatch,
    /// UTF-8 でない。
    NotUtf8,
    /// WGSL の解析に失敗した。
    Parse,
    /// WGSL の検証に失敗した。
    Invalid,
    /// compute の入口がちょうど 1 つでない。
    EntryPoint,
    /// workgroup の大きさが合わない。
    WorkgroupSize,
    /// 束縛の数・番号・型・アクセスが合わない。
    Bindings,
}

impl fmt::Display for ExternalWgslError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Missing => "kernel source or digest value is missing",
            Self::InvalidDigest => "digest must be 64 lowercase hex digits",
            Self::Unreadable => "source is not a readable regular file",
            Self::TooLarge => "source exceeds the size limit",
            Self::DigestMismatch => "source digest does not match",
            Self::NotUtf8 => "source is not valid UTF-8",
            Self::Parse => "source failed to parse as WGSL",
            Self::Invalid => "source failed WGSL validation",
            Self::EntryPoint => "source must have exactly one compute entry point",
            Self::WorkgroupSize => "workgroup size does not match",
            Self::Bindings => "resource bindings do not match",
        })
    }
}

impl std::error::Error for ExternalWgslError {}

/// 照合を通った WGSL のテキストと、計算した sha256。
///
/// 作れるのは [`ExternalWgsl::from_lookup`] と [`ExternalWgsl::from_env`] だけで、
/// 後続の device の経路はこの値のテキストだけを受け取る。
#[derive(Clone, PartialEq, Eq)]
pub struct ExternalWgsl {
    text: String,
    sha256: [u8; 32],
}

impl fmt::Debug for ExternalWgsl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExternalWgsl").finish_non_exhaustive()
    }
}

impl ExternalWgsl {
    /// 環境変数（[`KERNEL_SOURCE_ENV`]・[`KERNEL_SHA256_ENV`]）から作る。
    pub fn from_env() -> Result<Self, ExternalWgslError> {
        Self::from_lookup(|name| std::env::var_os(name))
    }

    /// 名前から値を引く関数から作る。検査の順は固定で、最初に当たった失敗を返す。
    pub fn from_lookup<F>(lookup: F) -> Result<Self, ExternalWgslError>
    where
        F: Fn(&str) -> Option<OsString>,
    {
        let (path, digest) = match (lookup(KERNEL_SOURCE_ENV), lookup(KERNEL_SHA256_ENV)) {
            (Some(path), Some(digest)) => (path, digest),
            _ => return Err(ExternalWgslError::Missing),
        };
        let expected = parse_digest(&digest)?;
        let bytes = read_capped(Path::new(&path))?;
        if bytes.len() > MAX_SOURCE_BYTES {
            return Err(ExternalWgslError::TooLarge);
        }
        let actual = sha256_bytes(&bytes);
        if actual != expected {
            return Err(ExternalWgslError::DigestMismatch);
        }
        let text = String::from_utf8(bytes).map_err(|_| ExternalWgslError::NotUtf8)?;
        check_shape(&text)?;
        Ok(Self {
            text,
            sha256: actual,
        })
    }

    /// 照合を通ったテキスト。
    pub fn text(&self) -> &str {
        &self.text
    }

    /// テキストの sha256。
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
}

/// 64 桁の小文字の 16 進を 32 バイトへ直す。
fn parse_digest(value: &OsString) -> Result<[u8; 32], ExternalWgslError> {
    let text = value.to_str().ok_or(ExternalWgslError::InvalidDigest)?;
    let bytes = text.as_bytes();
    if bytes.len() != 64 {
        return Err(ExternalWgslError::InvalidDigest);
    }
    let nibble = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let mut out = [0u8; 32];
    for (index, slot) in out.iter_mut().enumerate() {
        let hi = bytes.get(index * 2).copied().and_then(nibble);
        let lo = bytes.get(index * 2 + 1).copied().and_then(nibble);
        match (hi, lo) {
            (Some(hi), Some(lo)) => *slot = (hi << 4) | lo,
            _ => return Err(ExternalWgslError::InvalidDigest),
        }
    }
    Ok(out)
}

/// 通常のファイルを上限 + 1 バイトまで読む。開いた後のファイルでも通常のファイルかを確かめ直す。
fn read_capped(path: &Path) -> Result<Vec<u8>, ExternalWgslError> {
    let meta = fs::metadata(path).map_err(|_| ExternalWgslError::Unreadable)?;
    if !meta.is_file() {
        return Err(ExternalWgslError::Unreadable);
    }
    let file = File::open(path).map_err(|_| ExternalWgslError::Unreadable)?;
    let opened = file.metadata().map_err(|_| ExternalWgslError::Unreadable)?;
    if !opened.is_file() {
        return Err(ExternalWgslError::Unreadable);
    }
    let mut buf = Vec::new();
    file.take(MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|_| ExternalWgslError::Unreadable)?;
    Ok(buf)
}

/// 解析・検証を通し、入口と束縛の形を照らす。入口の名前は比べない。
fn check_shape(text: &str) -> Result<(), ExternalWgslError> {
    let module = wgpu::naga::front::wgsl::parse_str(text).map_err(|_| ExternalWgslError::Parse)?;
    Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .map_err(|_| ExternalWgslError::Invalid)?;
    match module.entry_points.as_slice() {
        [entry] if entry.stage == ShaderStage::Compute => {
            if entry.workgroup_size != TILED_WORKGROUP_SIZE {
                return Err(ExternalWgslError::WorkgroupSize);
            }
        }
        _ => return Err(ExternalWgslError::EntryPoint),
    }
    check_bindings(&module)
}

/// `binding` を持つ global が、group 0 の 0・1・2・3 の 4 つだけで、型とアクセスが合うことを照らす。
fn check_bindings(module: &Module) -> Result<(), ExternalWgslError> {
    let read = StorageAccess::LOAD;
    let read_write = StorageAccess::LOAD | StorageAccess::STORE;
    let mut seen = [false; 4];
    for (_, global) in module.global_variables.iter() {
        let Some(binding) = &global.binding else {
            continue;
        };
        if binding.group != 0 {
            return Err(ExternalWgslError::Bindings);
        }
        let index = usize::try_from(binding.binding).map_err(|_| ExternalWgslError::Bindings)?;
        let slot = seen.get_mut(index).ok_or(ExternalWgslError::Bindings)?;
        if *slot {
            return Err(ExternalWgslError::Bindings);
        }
        *slot = true;
        let ty = &module.types[global.ty].inner;
        let matches = match (index, global.space) {
            (0 | 1, AddressSpace::Storage { access }) => access == read && is_f32_array(module, ty),
            (2, AddressSpace::Storage { access }) => {
                access == read_write && is_f32_array(module, ty)
            }
            (3, AddressSpace::Uniform) => matches!(ty, TypeInner::Struct { span: 16, .. }),
            _ => false,
        };
        if !matches {
            return Err(ExternalWgslError::Bindings);
        }
    }
    if seen.iter().all(|s| *s) {
        Ok(())
    } else {
        Err(ExternalWgslError::Bindings)
    }
}

/// 要素が f32 で大きさが実行時に決まる配列か。
fn is_f32_array(module: &Module, ty: &TypeInner) -> bool {
    match ty {
        TypeInner::Array {
            base,
            size: ArraySize::Dynamic,
            ..
        } => module.types[*base].inner == TypeInner::Scalar(Scalar::F32),
        _ => false,
    }
}
