#!/usr/bin/env bash
# doctest-error-code-check.sh の回帰テスト。
# 一時ディレクトリに依存の無い合成の crate を作り、`compile_fail,E…` のコードが
# 正しければ通り、誤っていれば落ちること（stable の素の `cargo test --doc` は誤りでも通ること）を確かめる。
# リポジトリのファイルは変更しない。ネットワーク不要（依存の無い crate だけ）。
# 呼び出し元: `make test-doctest-error-code-check`（直接実行も可。CI の `deny` ジョブも呼ぶ）。
# 判定は終了コードに標準出力・標準エラーの部分一致を添えて行い、別の理由で落ちて素通りする退行を拾う。
# 途中で止めず、最後に件数を出す。bash 3.2 互換。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"
require_cmd cargo

CHECK="${DIR}/doctest-error-code-check.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-doctest-error-code-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
passed=0
failed=0
rc=0

# ok <名前> <条件を表す終了状態 0/1>
ok() {
  if [ "$2" -eq 0 ]; then
    passed=$((passed + 1))
    echo "ok: $1"
  else
    failed=$((failed + 1))
    echo "FAILED: $1" >&2
  fi
}

# make_crate <dir> <期待のコード>: 値の移動後の使用（E0382）を起こす compile_fail の doctest を持つ crate を作る。
make_crate() {
  mkdir -p "$1/src"
  cat >"$1/Cargo.toml" <<TOML
[package]
name = "doctest-probe"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]
TOML
  cat >"$1/src/lib.rs" <<RS
//! 合成の crate。
//!
//! \`\`\`compile_fail,$2
//! let v = String::new();
//! drop(v);
//! drop(v);
//! \`\`\`
RS
}

good="${work}/good"
bad="${work}/bad"
make_crate "${good}" E0382
make_crate "${bad}" E0999

# 1. 正しいコードは通る
rc=0
"${CHECK}" --manifest-path "${good}/Cargo.toml" >"${work}/good.out" 2>&1 || rc=$?
ok "correct error code passes (rc=${rc})" "${rc}"

# 2. 誤ったコードは落ちる（照合が効いている）
rc=0
"${CHECK}" --manifest-path "${bad}/Cargo.toml" >"${work}/bad.out" 2>&1 || rc=$?
if [ "${rc}" -ne 0 ] && grep -q "E0999" "${work}/bad.out"; then
  ok "wrong error code fails with E0999 in output (rc=${rc})" 0
else
  ok "wrong error code fails with E0999 in output (rc=${rc})" 1
fi

# 3. 対照: stable の素の cargo test --doc は誤ったコードでも通る（検査が要る理由の裏づけ）
rc=0
cargo test --doc --manifest-path "${bad}/Cargo.toml" >"${work}/plain.out" 2>&1 || rc=$?
ok "plain stable doctest accepts the wrong code (control, rc=${rc})" "${rc}"

# 4. 誤用は終了コード 2
rc=0
"${CHECK}" --bogus >/dev/null 2>&1 || rc=$?
if [ "${rc}" -eq 2 ]; then ok "misuse exits 2" 0; else ok "misuse exits 2 (rc=${rc})" 1; fi

echo "passed=${passed} failed=${failed}"
[ "${failed}" -eq 0 ]
