//! 比較基準の別 workspace（`baseline-wgpu/`）が単独でビルド・実行できることを確かめる疎通確認用 bin。
//!
//! 基盤の workspace からは呼ばれない。wgpu の依存はユーザー承認後に追加するため、
//! 現状は wgpu をリンクせず固定の識別子を出力して終了するだけである。

fn main() {
    println!("baseline-wgpu: skeleton (wgpu not linked yet)");
}
