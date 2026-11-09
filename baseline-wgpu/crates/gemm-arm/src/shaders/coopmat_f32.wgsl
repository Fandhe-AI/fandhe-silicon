// 1 workgroup が C の 8x8 のタイル 1 枚を持つ行列積（C = A * B）。A は M x K、B は K x N、C は M x N（すべて行優先）。
// M・N・K は 8 の倍数に詰めた値を dims に渡す前提。workgroup の数は (N / 8, M / 8, 1)。
enable wgpu_cooperative_matrix;

struct Dims {
  m: u32,
  k: u32,
  n: u32,
}

@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read_write> c: array<f32>;
@group(0) @binding(3) var<uniform> dims: Dims;

@compute @workgroup_size(32, 1, 1)
fn coopmat_gemm(@builtin(workgroup_id) wg: vec3<u32>) {
  let row0 = wg.y * 8u;
  let col0 = wg.x * 8u;
  var acc = coop_mat8x8<f32, C>();
  for (var p = 0u; p < dims.k; p += 8u) {
    let ma = coopLoadT<coop_mat8x8<f32, A>>(&a[row0 * dims.k + p], dims.k);
    let mb = coopLoadT<coop_mat8x8<f32, B>>(&b[p * dims.n + col0], dims.n);
    acc = coopMultiplyAdd(ma, mb, acc);
  }
  coopStoreT(acc, &c[row0 * dims.n + col0], dims.n);
}
