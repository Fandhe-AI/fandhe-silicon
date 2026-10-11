//! 背後実装の出力を CPU の参照と比べるための、検証専用の道具の入口（spec TASK-24 の 3/8・4/8・5/8。画素の入力は TASK-26 の 3/6）。
//!
//! feature `test-support` の裏にだけ公開される。置き場と feature 名は未承認の仮置きで、
//! 別の置き場に決まれば `parity` 配下をそのまま移す（`docs/parity-tool.md` の Q1・Q3）。
//! 製品の経路（`contract`・`exec`・`upper` の非テストのコード）からは参照しない。
//! 比べ方の追加（後続の作業）は、このモジュール配下にファイルを足して `pub mod` で束ねる。

pub mod cpu_reference;
pub mod pixel;
pub mod tolerance;
pub mod topk;
