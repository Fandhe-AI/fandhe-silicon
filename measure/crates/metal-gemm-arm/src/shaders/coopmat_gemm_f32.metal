// 門の 1 セルを 1 回投入する cooperative matrix の f32 GEMM（TASK-1009 のポインタ）。
// C = A × B、行優先。M・K・N が 8 の倍数のときの形。
// 束縛: buffer 0 = A、buffer 1 = B、buffer 2 = C、buffer 3 = 次元 { m, k, n }。
// 1 つの threadgroup（32 スレッド = 1 simdgroup）が C の 8×8 を 1 枚書く。
#include <metal_stdlib>
#include <metal_simdgroup_matrix>
using namespace metal;

struct Dims {
    uint m;
    uint k;
    uint n;
};

kernel void coopmat_gemm_f32(
    device const float* a [[buffer(0)]],
    device const float* b [[buffer(1)]],
    device float* c [[buffer(2)]],
    constant Dims& dims [[buffer(3)]],
    uint2 tgid [[threadgroup_position_in_grid]]
) {
    uint row0 = tgid.y * 8;
    uint col0 = tgid.x * 8;
    if (row0 >= dims.m || col0 >= dims.n) {
        return;
    }
    simdgroup_float8x8 acc = make_filled_simdgroup_matrix<float, 8, 8>(0.0f);
    for (uint p = 0; p < dims.k; p += 8) {
        simdgroup_float8x8 ma;
        simdgroup_float8x8 mb;
        simdgroup_load(ma, a + row0 * dims.k + p, dims.k);
        simdgroup_load(mb, b + p * dims.n + col0, dims.n);
        simdgroup_multiply_accumulate(acc, ma, mb, acc);
    }
    simdgroup_store(acc, c + row0 * dims.n + col0, dims.n);
}
