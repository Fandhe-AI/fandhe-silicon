//! 背後実装の出力を CPU の参照と比べるための、検証専用の道具の入口（spec TASK-24 の 3/8・4/8・5/8。画素の入力・比べる関数・閾値は TASK-26 の 3/6・4/6。有限でない値の規則は TASK-25 の 2/4。Top-k の追加の入力は TASK-976 の 4/6。GEMM の門の入力と参照は TASK-1005 の 2/6。母数の表は TASK-1005 の 3/6。累積の順の対照は TASK-1005 の 4/6。門の run の枠は TASK-1005 の 5/6。代わりの実行の比べ方は TASK-928 の 2/8）。
//!
//! feature `test-support` の裏にだけ公開される（置き場と feature 名の説明は `docs/parity-tool.md` の「所在」）。
//! 製品の経路（`contract`・`exec`・`upper` の非テストのコード）からは参照しない。
//! 比べ方の追加（後続の作業）は、このモジュール配下にファイルを足して `pub mod` で束ねる。

pub mod binary16;
pub mod cpu_reference;
pub mod fallback_compare;
pub mod gemm_gate;
pub mod gemm_inputs;
pub mod gemm_population;
pub mod pixel;
pub mod special_values;
pub mod tolerance;
pub mod topk;
pub mod topk_inputs;
