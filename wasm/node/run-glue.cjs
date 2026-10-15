// wasm-bindgen が生成した glue（--target nodejs）を Node で読み込み、入力の各行について export を呼び、
// 観測した f32 のビット列を書き出すだけのスクリプト（TASK-22 の Node での照合の土台）。
// 呼び出し元: scripts/wasm-node-check.sh（引数: glue の js のパス・入力の JSON・結果のファイル）。
//
// 判定（一致・不一致・未実施）は持たない。判定は呼び出し元の bash 側（python3）にあり、
// 偽の node で差し替えた回帰テストが判定の全経路を機材なしで確かめられるようにするため。
// 値は Number を経由せず、f32 のビット列（8 桁の 16 進）を Uint32Array と Float32Array の同じ
// ArrayBuffer のビューで受け渡す。NaN のペイロード・-0.0・±Inf を落とさないため。
// 使うのは Node の組み込み（node: 付き）と引数で渡された glue だけ。npm パッケージには依存しない。
"use strict";

const fs = require("node:fs");

// 呼べる export の許可表。入力由来の名前でプロパティを引かない。
const EXPORTS = {
  roundtrip_f32: (glue, f32) => glue.roundtrip_f32(f32),
};

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

function main(argv) {
  if (argv.length !== 3) {
    process.stderr.write("usage: run-glue.cjs <glue.js> <cases.json> <results>\n");
    return 2;
  }
  const [gluePath, casesPath, resultsPath] = argv;
  const glue = require(gluePath);
  const cases = JSON.parse(fs.readFileSync(casesPath, "utf8"));
  const lines = [];
  for (const row of cases.rows) {
    const known = Object.prototype.hasOwnProperty.call(EXPORTS, row.export);
    if (!known) {
      lines.push(JSON.stringify({ id: row.id, error: "unknown export" }));
      continue;
    }
    try {
      const out = EXPORTS[row.export](glue, hexToF32(row.input));
      lines.push(JSON.stringify({ id: row.id, output: f32ToHex(out) }));
    } catch (e) {
      lines.push(JSON.stringify({ id: row.id, error: "call failed" }));
    }
  }
  fs.writeFileSync(resultsPath, lines.join("\n") + (lines.length > 0 ? "\n" : ""));
  return 0;
}

process.exitCode = main(process.argv.slice(2));
