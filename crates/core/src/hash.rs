//! 外部依存なしの SHA-256（FIPS 180-4）。
//!
//! 純粋な関数だけを持ち、プロセス・ファイル・時刻・環境変数・スレッドに触れない。
//! 上位の層や `contract` から同一性の識別に呼ばれる想定（TASK-471・ART-28 のポインタ）。
//! 秘密の比較・MAC・パスワードの用途は対象外で、定数時間の性質は保証しない。

/// 丸め定数（FIPS 180-4 の K）。
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// 初期のハッシュ値（FIPS 180-4 の H(0)）。
const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// 64 バイトの塊 1 つを状態へ畳み込む。`sha256_bytes` だけから呼ばれる。
fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    let (words, _) = block.as_chunks::<4>();
    for (slot, word) in w.iter_mut().zip(words) {
        *slot = u32::from_be_bytes(*word);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for (k, wi) in K.iter().zip(w.iter()) {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(*k)
            .wrapping_add(*wi);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *s = s.wrapping_add(v);
    }
}

/// `data` の SHA-256 を 32 バイトで返す。
///
/// `sha256_bytes(b"")` は `e3b0c442…b855`（16 進）になる。
///
/// # 入力の長さ
///
/// 長さの欄は 2^64 の剰余で詰める。2^61 バイト以上の入力は FIPS 180-4 の定義域の外。
///
/// ```
/// let d = fandhe_silicon_core::hash::sha256_bytes(b"");
/// assert_eq!(d[..4], [0xe3, 0xb0, 0xc4, 0x42]);
/// ```
pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut state = H0;
    let (blocks, rem) = data.as_chunks::<64>();
    for block in blocks {
        compress(&mut state, block);
    }
    // 余り（0〜63 バイト）+ 0x80 + 0 詰め + 8 バイトの長さ。余りが 56 以上なら 2 ブロック。
    let mut tail = [0u8; 128];
    tail[..rem.len()].copy_from_slice(rem);
    tail[rem.len()] = 0x80;
    let total = if rem.len() < 56 { 64 } else { 128 };
    let bit_len = ((data.len() as u128) * 8) as u64;
    tail[total - 8..total].copy_from_slice(&bit_len.to_be_bytes());
    let (tail_blocks, _) = tail[..total].as_chunks::<64>();
    for block in tail_blocks {
        compress(&mut state, block);
    }
    let mut out = [0u8; 32];
    let (out_words, _) = out.as_chunks_mut::<4>();
    for (dst, word) in out_words.iter_mut().zip(state) {
        *dst = word.to_be_bytes();
    }
    out
}

/// `sha256_bytes` のダイジェストの先頭 8 バイトを big-endian で読んだ値を返す。
///
/// ```
/// assert_eq!(fandhe_silicon_core::hash::sha256_u64(b"abc"), 0xba7816bf8f01cfea);
/// ```
pub fn sha256_u64(data: &[u8]) -> u64 {
    let [b0, b1, b2, b3, b4, b5, b6, b7, ..] = sha256_bytes(data);
    u64::from_be_bytes([b0, b1, b2, b3, b4, b5, b6, b7])
}
