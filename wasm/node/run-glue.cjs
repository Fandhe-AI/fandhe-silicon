// wasm-bindgen が生成した glue（--target nodejs）を Node で読み込み、入力の各行について export を呼び、
// 観測した値のビット列を書き出すだけのスクリプト（TASK-22 の Node での照合の土台。生成の入力の行と 2 つのビルドは TASK-994）。
// 呼び出し元: scripts/wasm-node-check.sh
// （引数: scalar の glue の js または - ・入力の JSON・結果のファイル・simd128 の glue の js または - ・
//  host 側の道具の入力のディレクトリまたは - ・出力のディレクトリ）。
//
// 判定（一致・不一致・未実施）は持たない。判定は呼び出し元の bash 側（python3）にあり、
// 偽の node で差し替えた回帰テストが判定の全経路を機材なしで確かめられるようにするため。
// 固定の行の値は Number を経由せず、f32 のビット列（8 桁の 16 進）を Uint32Array と Float32Array の同じ
// ArrayBuffer のビューで受け渡す。NaN のペイロード・-0.0・±Inf を落とさないため。
// 生成の行（gemm_*）は入力の .bin を読み、出力を <出力のディレクトリ>/<行の id>.bin にバイト列のまま書く。
// 使うのは Node の組み込み（node: 付き）と引数で渡された glue だけ。npm パッケージには依存しない。
"use strict";

const fs = require("node:fs");
const nodePath = require("node:path");

// out を埋める番兵（呼び出しが書かなかった語を取り違えないため）。
const FILL_WORD = 0x7fc0dead;

function loadGlue(gluePath) {
  return require(gluePath);
}

function hexToF32(list) {
  const bits = new Uint32Array(list.length);
  for (let i = 0; i < list.length; i += 1) {
    bits[i] = Number.parseInt(list[i], 16) >>> 0;
  }
  return new Float32Array(bits.buffer);
}

function f32ToHex(arr) {
  const bits = new Uint32Array(arr.buffer, arr.byteOffset, arr.length);
  return Array.from(bits, (b) => b.toString(16).padStart(8, "0"));
}

// .bin を新しい ArrayBuffer に写して Float32Array にする（Buffer の byteOffset に依らない）。
function readF32(file) {
  const buf = fs.readFileSync(file);
  const ab = new ArrayBuffer(buf.length);
  new Uint8Array(ab).set(buf);
  return new Float32Array(ab);
}

// 呼べる export の許可表。入力由来の名前でプロパティを引かない。
// 固定の行（kind "fixed"）は出力の 16 進の列を返し、生成の行（kind "gemm"）は戻りの語を返す。
const EXPORTS = {
  roundtrip_f32: { kind: "fixed", call: (glue, row) => f32ToHex(glue.roundtrip_f32(hexToF32(row.input))) },
  detect_isa_bits: { kind: "fixed", call: (glue) => [(glue.detect_isa_bits() >>> 0).toString(16).padStart(8, "0")] },
  gemm_scalar: { kind: "gemm", call: (glue, a, b, m, k, n, out) => glue.gemm_scalar(a, b, m, k, n, out) },
  gemm_simd: { kind: "gemm", call: (glue, a, b, m, k, n, out) => glue.gemm_simd(a, b, m, k, n, out) },
};

function runGemm(entry, glue, c, id, outputs) {
  const a = readF32(nodePath.join(c.inputs, c.a_file));
  const b = readF32(nodePath.join(c.inputs, c.b_file));
  const out = new Float32Array(c.m * c.n);
  new Uint32Array(out.buffer).fill(FILL_WORD);
  const r = entry.call(glue, a, b, c.m, c.k, c.n, out) >>> 0;
  if (r !== 0) {
    return { id, error: "nonzero return" };
  }
  const name = id + ".bin";
  fs.writeFileSync(nodePath.join(outputs, name), new Uint8Array(out.buffer, out.byteOffset, out.byteLength));
  return { id, output_file: name };
}

function main(argv) {
  if (argv.length !== 6) {
    process.stderr.write(
      "usage: run-glue.cjs <scalar glue.js|-> <cases.json> <results> <simd128 glue.js|-> <inputs dir|-> <outputs dir>\n",
    );
    return 2;
  }
  const [scalarPath, casesPath, resultsPath, simdPath, inputsDir, outputsDir] = argv;
  const cases = JSON.parse(fs.readFileSync(casesPath, "utf8"));
  const glues = {
    scalar: scalarPath === "-" ? null : loadGlue(scalarPath),
    simd128: simdPath === "-" ? null : loadGlue(simdPath),
  };
  // manifest のケースは Map で引く（行の case でオブジェクトのプロパティを引かない）。
  const manifest = new Map();
  if (inputsDir !== "-") {
    const m = JSON.parse(fs.readFileSync(nodePath.join(inputsDir, "manifest.json"), "utf8"));
    for (const c of m.cases) {
      manifest.set(c.name, { ...c, inputs: inputsDir });
    }
  }
  const lines = [];
  for (const row of cases.rows) {
    const known = Object.prototype.hasOwnProperty.call(EXPORTS, row.export);
    if (!known) {
      lines.push(JSON.stringify({ id: row.id, error: "unknown export" }));
      continue;
    }
    const entry = EXPORTS[row.export];
    const glue = row.build === "simd128" ? glues.simd128 : glues.scalar;
    try {
      if (entry.kind === "gemm") {
        lines.push(JSON.stringify(runGemm(entry, glue, manifest.get(row.case), row.id, outputsDir)));
      } else {
        lines.push(JSON.stringify({ id: row.id, output: entry.call(glue, row) }));
      }
    } catch (e) {
      lines.push(JSON.stringify({ id: row.id, error: "call failed" }));
    }
  }
  fs.writeFileSync(resultsPath, lines.join("\n") + (lines.length > 0 ? "\n" : ""));
  return 0;
}

process.exitCode = main(process.argv.slice(2));
