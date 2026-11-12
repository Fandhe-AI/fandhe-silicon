//! 実行時に受けるカーネルの一覧の検査と、一覧の 1 行からパイプラインを作る手順の部品（TASK-485 のポインタ）。
//!
//! 一覧の読み取りと形の検査（`manifest`）と MSL のソースからの入口名の抜き出し（`scan`）は device に触れない純粋な部品。
//! bin `accept-kernel` の手順の部品（`accept`）と、`specialized` の行に渡す定数の並び（`constants`）も置く。
//! Metal の device に触れるのは `accept` の macOS の cfg の内側だけ。置き場は製品の外の measure の workspace で、
//! 後続の段が一覧の各行から起動の形を組むときに、ここの型と関数を使う。

pub mod accept;
pub mod constants;
pub mod manifest;
pub mod scan;
