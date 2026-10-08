//! fandhe-silicon の下-2 crate（共通の操作と代わりの実行）。
//!
//! `fandhe-silicon-core` の共通型だけに依存し、下-1 の背後実装を共通型経由で呼び出す。
//! 能力が無い場合の代わりの実行もここで扱う。
#![forbid(unsafe_code)]
