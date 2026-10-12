//! 背後実装の出力を CPU の参照と比べるための、検証専用の道具の入口（spec TASK-24 の 3/8・4/8・5/8。画素の入力・比べる関数・閾値は TASK-26 の 3/6・4/6。有限でない値の規則は TASK-25 の 2/4）。
//!
//! feature `test-support` の裏にだけ公開される（置き場と feature 名の説明は `docs/parity-tool.md` の「所在」）。
//! 製品の経路（`contract`・`exec`・`upper` の非テストのコード）からは参照しない。
//! 比べ方の追加（後続の作業）は、このモジュール配下にファイルを足して `pub mod` で束ねる。

pub mod cpu_reference;
pub mod pixel;
pub mod special_values;
pub mod tolerance;
pub mod topk;
