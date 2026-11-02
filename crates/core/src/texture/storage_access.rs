//! storage texture の束縛のアクセスの値の型の置き場（TASK-1944）。
//!
//! `texture` モジュールの一部で、公開のパスは `fandhe_silicon_core::texture::…`。
//! 値だけを扱う型で、背後実装の API は呼ばない。
//! 書く・読むの問いは [`StorageTextureAccess::writes`]・[`StorageTextureAccess::reads`] が返す。

/// storage texture の束縛のアクセス。書く・読むは `writes`・`reads` で問う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StorageTextureAccess {
    /// 書き込むだけ（TEX-78）。
    WriteOnly,
    /// 読むだけ（TEX-79）。
    ReadOnly,
    /// 読みと書き込み（TEX-80）。
    ReadWrite,
    /// atomic の操作（TEX-81）。
    Atomic,
}

impl StorageTextureAccess {
    /// 4 値を宣言の順に並べた一覧。
    pub const ALL: &'static [Self] = &[
        Self::WriteOnly,
        Self::ReadOnly,
        Self::ReadWrite,
        Self::Atomic,
    ];

    /// 書き込みを含むなら真。
    ///
    /// ```
    /// use fandhe_silicon_core::texture::StorageTextureAccess;
    /// assert!(StorageTextureAccess::WriteOnly.writes());
    /// assert!(!StorageTextureAccess::ReadOnly.writes());
    /// ```
    pub const fn writes(self) -> bool {
        match self {
            Self::WriteOnly | Self::ReadWrite | Self::Atomic => true,
            Self::ReadOnly => false,
        }
    }

    /// 読みを含むなら真。
    ///
    /// ```
    /// use fandhe_silicon_core::texture::StorageTextureAccess;
    /// assert!(StorageTextureAccess::ReadOnly.reads());
    /// assert!(!StorageTextureAccess::WriteOnly.reads());
    /// ```
    pub const fn reads(self) -> bool {
        match self {
            Self::ReadOnly | Self::ReadWrite | Self::Atomic => true,
            Self::WriteOnly => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StorageTextureAccess::{self, *};

    #[test]
    fn storage_access_all_has_four_distinct_values() {
        let all = StorageTextureAccess::ALL;
        assert_eq!(all.len(), 4);
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(all[i], all[j]);
            }
        }
        assert_eq!(all, &[WriteOnly, ReadOnly, ReadWrite, Atomic]);
    }

    #[test]
    fn storage_access_writes_reads_table() {
        let table = [
            (WriteOnly, true, false),
            (ReadOnly, false, true),
            (ReadWrite, true, true),
            (Atomic, true, true),
        ];
        for (a, w, r) in table {
            assert_eq!(a.writes(), w, "{a:?} writes");
            assert_eq!(a.reads(), r, "{a:?} reads");
        }
    }

    #[test]
    fn storage_access_every_value_reads_or_writes() {
        for a in StorageTextureAccess::ALL {
            assert!(a.writes() || a.reads(), "{a:?}");
        }
    }
}
