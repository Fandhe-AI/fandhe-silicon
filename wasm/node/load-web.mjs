// wasm-bindgen の `--target web` の出力（wasm_glue.js と wasm_glue_bg.wasm）を Node の ESM で読み込み、
// 決まった 7 名の export の有無と detect_isa_bits の戻り値を 1 行の JSON で書き出す（TASK-248 のポインタ）。
// 呼び出し元: scripts/wasm-web-load-check.sh（組ごとに別のプロセスで 1 回ずつ流す）。判定は持たない（判定は呼び出し元の python3）。
//
// 使い方: node load-web.mjs <組のディレクトリ> <結果のファイル>
// 終了コード: 2 は引数の誤用だけ。それ以外は常に 0（結果のファイルを書けないときだけ未捕捉で非 0）。
// 結果の形は次の 3 つだけ。例外の文・パスは書き出さない。
//   {"init":"failed"}
//   {"init":"ok","exports":[...]}
//   {"init":"ok","exports":[...,"detect_isa_bits"],"detect_isa_bits":<u32 | "call-failed">}
// 読むのは固定の 2 名だけで、同じディレクトリのほかの項目は見ない。detect_isa_bits 以外の export は呼ばない。
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

// 許可表（宣言点はここ 1 か所）。入力由来の名前でプロパティを引かない。
const EXPORTS = [
  "gemm_scalar",
  "gemm_simd",
  "topk_scalar",
  "topk_simd",
  "elementwise_scalar",
  "elementwise_simd",
  "detect_isa_bits",
];

async function observe(dir) {
  let ns;
  try {
    ns = await import(pathToFileURL(path.join(dir, "wasm_glue.js")).href);
    const bytes = fs.readFileSync(path.join(dir, "wasm_glue_bg.wasm"));
    if (typeof ns.initSync !== "function") {
      return { init: "failed" };
    }
    ns.initSync({ module: bytes });
  } catch {
    return { init: "failed" };
  }
  const found = EXPORTS.filter((name) => typeof ns[name] === "function");
  const result = { init: "ok", exports: found };
  if (found.includes("detect_isa_bits")) {
    try {
      const v = ns.detect_isa_bits();
      result.detect_isa_bits = typeof v === "number" ? v >>> 0 : "call-failed";
    } catch {
      result.detect_isa_bits = "call-failed";
    }
  }
  return result;
}

if (process.argv.length !== 4) {
  process.stderr.write("usage: load-web.mjs <combo-dir> <result-file>\n");
  process.exit(2);
}
const [dir, out] = process.argv.slice(2);
const result = await observe(dir);
fs.writeFileSync(out, JSON.stringify(result) + "\n");
