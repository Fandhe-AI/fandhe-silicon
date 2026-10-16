#!/usr/bin/env bash
# scripts/baseline-positive-control.sh の回帰テスト（spec TASK-12 のポインタ。親 Issue #365 の 3/4）。
# 呼び出し元: `make test-baseline-positive-control` と CI の `deny` ジョブ（直接実行も可）。
# 「合成」の節は、本体と lib.sh を一時ディレクトリの scripts/ へ写し、隣に偽の検査 2 本を置いて、
# 内側の終了コードと `record:` の行から定義ごと（狭い・広い・公開 API）の判定が出ることを確かめる
# （本体は兄弟のスクリプトを DIR 経由で呼ぶため、写しが要る）。
# 「実物」の節は、実行時に一時ディレクトリへ作る合成 path crate の一時 workspace（比較基準の baseline-wgpu/ ではない）に
# 本体と実物の 2 検査を写し、実物の出力で通す。ネットワークは使わず（path 依存だけ）、リポジトリのファイルを変更しない。
# 前提: python3・cargo・git・rust-toolchain.toml の toolchain（無ければ失敗にし、skip しない）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」で行う。終了コードだけでは別の理由で落ちても
# 気付けず、検査が素通りする退行（fail-open）を見逃すため。
# 期待値を緩めない。現行の検査と食い違ったらテストでなく検査側の挙動として直す。
# 途中で止めず、最後に件数を出す。bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3
require_cmd cargo
require_cmd git

work="$(mktemp -d "${TMPDIR:-/tmp}/test-baseline-positive-control.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

report_ok() {
  passed=$((passed + 1))
  echo "ok: $1"
}

report_fail() {
  failed=$((failed + 1))
  echo "FAIL: $1: $2" >&2
  echo "--- stdout ---" >&2
  sed 's/^/    /' "${OUT_F}" >&2 || true
  echo "--- stderr ---" >&2
  sed 's/^/    /' "${ERR_F}" >&2 || true
}

# judge <名前> <期待コード> [語...]: o:<語> は標準出力に、e:<語> は標準エラーに含まれること。!<語> はどちらにも含まれないこと。
judge() {
  local name="$1" want="$2" s
  shift 2
  if [ "${rc}" -ne "${want}" ]; then
    report_fail "${name}" "expected exit ${want}, got ${rc}"
    return 0
  fi
  for s in "$@"; do
    case "${s}" in
      '!'*)
        if grep -qF -e "${s#!}" "${ERR_F}" "${OUT_F}"; then
          report_fail "${name}" "output must not contain: ${s#!}"
          return 0
        fi
        ;;
      o:*)
        if ! grep -qF -e "${s#o:}" "${OUT_F}"; then
          report_fail "${name}" "stdout lacks: ${s#o:}"
          return 0
        fi
        ;;
      e:*)
        if ! grep -qF -e "${s#e:}" "${ERR_F}"; then
          report_fail "${name}" "stderr lacks: ${s#e:}"
          return 0
        fi
        ;;
    esac
  done
  report_ok "${name}"
}

# count_is <名前> <ファイル> <固定文字列> <件数>: その語を含む行数が一致すること。
count_is() {
  local name="$1" file="$2" word="$3" want="$4" got
  got="$(grep -cF -e "${word}" "${file}" || true)"
  if [ "${got}" = "${want}" ]; then
    report_ok "${name}"
  else
    report_fail "${name}" "expected ${want} line(s) containing '${word}', got ${got}"
  fi
}

# ---- 合成（偽の検査）----
fk="${work}/fake"
mkdir -p "${fk}/scripts" "${fk}/baseline-wgpu"
cp "${DIR}/baseline-positive-control.sh" "${DIR}/lib.sh" "${fk}/scripts/"
chmod +x "${fk}/scripts/baseline-positive-control.sh"
: >"${fk}/baseline-wgpu/Cargo.toml"

# 偽の検査: FAKE_<名前>_RC の終了コードで終わり、FAKE_<名前>_RECORD の形で record: を出し、受け取った引数を記録する。
#   normal: 2 検査それぞれの鍵を FAKE_NARROW / FAKE_WIDE / FAKE_TYPE / FAKE_OTHER の値で出す
#   none: record: の行なし / twice: 2 行 / garbled: 値が整数でない / missing-key: 鍵が 1 つ欠ける / dupkey: 鍵が重複
cat >"${fk}/scripts/dep-graph-wgpu-check.sh" <<FAKE
#!/usr/bin/env bash
echo "\$*" >> "${work}/args-dep"
echo "scanned: fake=dep"
case "\${FAKE_DEP_RECORD:-normal}" in
  normal) echo "record: commit=- crates_total=2 workspace_members=1 narrow_total=\${FAKE_NARROW:-1} wide_total=\${FAKE_WIDE:-1} no_repository=0" ;;
  none) ;;
  twice)
    echo "record: commit=- crates_total=2 workspace_members=1 narrow_total=1 wide_total=1 no_repository=0"
    echo "record: commit=- crates_total=2 workspace_members=1 narrow_total=1 wide_total=1 no_repository=0" ;;
  garbled) echo "record: commit=- crates_total=2 workspace_members=1 narrow_total=x wide_total=1 no_repository=0" ;;
  missing-key) echo "record: commit=- crates_total=2 workspace_members=1 narrow_total=1 no_repository=0" ;;
  dupkey) echo "record: commit=- narrow_total=1 narrow_total=1 wide_total=1" ;;
esac
exit "\${FAKE_DEP_RC:-1}"
FAKE
cat >"${fk}/scripts/public-api-check.sh" <<FAKE
#!/usr/bin/env bash
echo "\$*" >> "${work}/args-api"
echo "scanned: fake=api"
case "\${FAKE_API_RECORD:-normal}" in
  normal) echo "record: commit=- documents=2 public_items_total=9 wgpu_type_refs_total=\${FAKE_TYPE:-1} wgpu_other_refs_total=\${FAKE_OTHER:-0}" ;;
  none) ;;
  twice)
    echo "record: commit=- documents=2 public_items_total=9 wgpu_type_refs_total=1 wgpu_other_refs_total=0"
    echo "record: commit=- documents=2 public_items_total=9 wgpu_type_refs_total=1 wgpu_other_refs_total=0" ;;
  garbled) echo "record: commit=- documents=2 public_items_total=9 wgpu_type_refs_total=1 wgpu_other_refs_total=-1" ;;
  missing-key) echo "record: commit=- documents=2 public_items_total=9 wgpu_type_refs_total=1" ;;
  dupkey) echo "record: commit=- wgpu_type_refs_total=1 wgpu_type_refs_total=1 wgpu_other_refs_total=0" ;;
esac
exit "\${FAKE_API_RC:-1}"
FAKE
chmod +x "${fk}/scripts/dep-graph-wgpu-check.sh" "${fk}/scripts/public-api-check.sh"

# run_fake: 環境変数を既定に戻してから、呼び出し側の指定（NAME=値 …）を足して本体を流す。
run_fake() {
  rm -f "${work}/args-dep" "${work}/args-api"
  rc=0
  env -u FAKE_DEP_RC -u FAKE_API_RC -u FAKE_DEP_RECORD -u FAKE_API_RECORD \
    -u FAKE_NARROW -u FAKE_WIDE -u FAKE_TYPE -u FAKE_OTHER \
    "$@" "${fk}/scripts/baseline-positive-control.sh" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}
run_fake_args() {
  rm -f "${work}/args-dep" "${work}/args-api"
  rc=0
  "${fk}/scripts/baseline-positive-control.sh" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

# (1) 検出がある合成の入力で成功
run_fake env FAKE_DEP_RC=1 FAKE_API_RC=1
judge "all_detected" 0 \
  "o:definition=narrow count=1 result=detected" "o:definition=wide count=1 result=detected" \
  "o:definition=public-api count=1 result=detected" "o:ok: positive control detected wgpu in baseline-wgpu (narrow, wide, public-api)" \
  "o:record: " "o:scanned: fake=dep" "o:scanned: fake=api" "!positive-control-failed" "!${work}"
count_is "all_detected_three_lines" "${OUT_F}" "result=detected" 3
args_dep="$(cat "${work}/args-dep" 2>/dev/null || true)"
args_api="$(cat "${work}/args-api" 2>/dev/null || true)"
case "${args_dep}" in
  *"--manifest-path ${fk}/baseline-wgpu/Cargo.toml"*"--allow-metadata-superset"*) report_ok "args_dep_manifest_and_superset" ;;
  *) report_fail "args_dep_manifest_and_superset" "args=${args_dep}" ;;
esac
case "${args_api}" in
  "--manifest-path ${fk}/baseline-wgpu/Cargo.toml") report_ok "args_api_manifest_only" ;;
  *) report_fail "args_api_manifest_only" "args=${args_api}" ;;
esac
# 件数は出すだけ。大きい値も 1 件以上と同じに扱い、特定の値とは比べない
run_fake env FAKE_DEP_RC=1 FAKE_API_RC=1 FAKE_NARROW=7 FAKE_WIDE=300 FAKE_TYPE=0 FAKE_OTHER=5
judge "counts_not_compared" 0 "o:definition=narrow count=7 " "o:definition=wide count=300 " \
  "o:definition=public-api count=5 " "o:ok: " "!positive-control-failed"
# public-api は型の参照と経路の参照の和で判定する
run_fake env FAKE_DEP_RC=1 FAKE_API_RC=1 FAKE_TYPE=0 FAKE_OTHER=1
judge "public_api_other_only" 0 "o:definition=public-api count=1 result=detected" "o:ok: "
run_fake env FAKE_DEP_RC=1 FAKE_API_RC=1 FAKE_TYPE=1 FAKE_OTHER=0
judge "public_api_type_only" 0 "o:definition=public-api count=1 result=detected" "o:ok: "

# (2) 検出 0 件の合成の入力で失敗（終了コード 1）
run_fake env FAKE_DEP_RC=0 FAKE_API_RC=0 FAKE_NARROW=0 FAKE_WIDE=0 FAKE_TYPE=0 FAKE_OTHER=0
judge "all_zero" 1 "e:definition=narrow count=0 result=no-detection" "e:definition=wide count=0 result=no-detection" \
  "e:definition=public-api count=0 result=no-detection" "!ok: positive control" "!result=detected"
count_is "all_zero_three_failures" "${ERR_F}" "result=no-detection" 3
# 違いを見分けるケース: 内側は 1 でも、片方の定義だけ 0 件なら失敗にする（終了コードだけの判定なら合格してしまう）
run_fake env FAKE_DEP_RC=1 FAKE_API_RC=1 FAKE_NARROW=1 FAKE_WIDE=0
judge "narrow_only" 1 "o:definition=narrow count=1 result=detected" "e:definition=wide count=0 result=no-detection" "!ok: positive control"
run_fake env FAKE_DEP_RC=1 FAKE_API_RC=1 FAKE_NARROW=0 FAKE_WIDE=1
judge "wide_only" 1 "e:definition=narrow count=0 result=no-detection" "o:definition=wide count=1 result=detected" "!ok: positive control"
run_fake env FAKE_DEP_RC=1 FAKE_API_RC=0 FAKE_TYPE=0 FAKE_OTHER=0
judge "public_api_zero" 1 "o:definition=narrow count=1 " "o:definition=wide count=1 " \
  "e:definition=public-api count=0 result=no-detection" "!ok: positive control"
# dep-graph が 0 件でも public-api は流れ、定義ごとの結果が出る
run_fake env FAKE_DEP_RC=0 FAKE_API_RC=1 FAKE_NARROW=0 FAKE_WIDE=0
judge "dep_zero_api_detected" 1 "e:definition=narrow count=0 " "e:definition=wide count=0 " \
  "o:definition=public-api count=1 result=detected" "!ok: positive control"
if [ -s "${work}/args-api" ]; then
  report_ok "api_still_ran_after_dep_zero"
else
  report_fail "api_still_ran_after_dep_zero" "public-api-check was not run"
fi

# (3) 読めない入力・想定外・ツール未導入は終了コード 2（0 件の失敗 = 1 とは区別される）
for mode in none twice garbled missing-key dupkey; do
  run_fake env FAKE_DEP_RC=1 FAKE_API_RC=1 FAKE_DEP_RECORD="${mode}"
  judge "dep_record_${mode}" 2 "e:check=dep-graph-wgpu-check inner_exit=1 result=unreadable-record" "!ok: positive control" "!result=no-detection"
  run_fake env FAKE_DEP_RC=1 FAKE_API_RC=1 FAKE_API_RECORD="${mode}"
  judge "api_record_${mode}" 2 "e:check=public-api-check inner_exit=1 result=unreadable-record" "!ok: positive control" "!result=no-detection"
done
# 内側の終了コードが 2・想定外
run_fake env FAKE_DEP_RC=1 FAKE_API_RC=2
judge "api_error" 2 "e:check=public-api-check inner_exit=2 result=error" "!ok: positive control"
run_fake env FAKE_DEP_RC=2 FAKE_API_RC=1
judge "dep_error" 2 "e:check=dep-graph-wgpu-check inner_exit=2 result=error" "!ok: positive control"
run_fake env FAKE_DEP_RC=137 FAKE_API_RC=1
judge "dep_unexpected" 2 "e:inner_exit=137 result=error" "!ok: positive control"
# 2 は 1（0 件）に勝つ
run_fake env FAKE_DEP_RC=0 FAKE_API_RC=2 FAKE_NARROW=0 FAKE_WIDE=0
judge "error_beats_zero" 2 "e:result=no-detection" "e:result=error" "!ok: positive control"
# 内側の終了コードと件数の食い違い
run_fake env FAKE_DEP_RC=1 FAKE_API_RC=1 FAKE_NARROW=0 FAKE_WIDE=0
judge "inner_one_but_all_zero" 2 "e:check=dep-graph-wgpu-check inner_exit=1 result=inconsistent" "!ok: positive control"
run_fake env FAKE_DEP_RC=1 FAKE_API_RC=1 FAKE_TYPE=0 FAKE_OTHER=0
judge "inner_one_but_api_zero" 2 "e:check=public-api-check inner_exit=1 result=inconsistent" "!ok: positive control"
run_fake env FAKE_DEP_RC=0 FAKE_API_RC=0 FAKE_NARROW=1 FAKE_WIDE=0 FAKE_TYPE=0 FAKE_OTHER=0
judge "inner_zero_but_count" 2 "e:check=dep-graph-wgpu-check inner_exit=0 result=inconsistent" "!ok: positive control"
# 未知の引数
run_fake_args --bogus
judge "unknown_arg" 2 "e:usage:" "!ok: positive control"
if [ -s "${work}/args-dep" ] || [ -s "${work}/args-api" ]; then
  report_fail "unknown_arg_runs_nothing" "inner checks must not run"
else
  report_ok "unknown_arg_runs_nothing"
fi

# ツール未導入: 絞った PATH に必要なコマンドだけを置き、cargo だけを外す
bash_bin="$(command -v bash)"
py_real="$(python3 -I -c 'import sys; print(sys.executable)')"
nocargo="${work}/nocargo-bin"
mkdir -p "${nocargo}"
for tool in dirname basename mktemp cat rm git; do
  ln -s "$(command -v "${tool}")" "${nocargo}/${tool}"
done
ln -s "${py_real}" "${nocargo}/python3"
rm -f "${work}/args-dep" "${work}/args-api"
rc=0
PATH="${nocargo}" "${bash_bin}" "${fk}/scripts/baseline-positive-control.sh" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "no_cargo" 2 "e:required command not found on PATH: cargo" "!ok: positive control" "!result=no-detection" "!${work}/nocargo-bin"
if [ -s "${work}/args-dep" ] || [ -s "${work}/args-api" ]; then
  report_fail "no_cargo_runs_nothing" "inner checks must not run"
else
  report_ok "no_cargo_runs_nothing"
fi

# ---- 実物（一時 workspace の実物の出力）----
# ${rw}/ws を製品の workspace に見立て、その中の baseline-wgpu/ に合成 path crate の workspace を置く。
rw="${work}/real"
mkdir -p "${rw}/ws/scripts" "${rw}/ws/baseline-wgpu/crates/api/src" "${rw}/ext"
cat >"${rw}/ws/baseline-wgpu/Cargo.toml" <<'TOML'
[workspace]
members = ["crates/api"]
resolver = "2"
TOML
for f in baseline-positive-control.sh dep-graph-wgpu-check.sh public-api-check.sh lib.sh; do
  cp "${DIR}/${f}" "${rw}/ws/scripts/"
done
chmod +x "${rw}/ws/scripts/"*.sh
if [ -f "${DIR}/../rust-toolchain.toml" ]; then
  cp "${DIR}/../rust-toolchain.toml" "${rw}/ws/rust-toolchain.toml"
fi

# mk_ext <名前> <repository>: ${rw}/ext/<名前> に合成 crate を作る。
mk_ext() {
  mkdir -p "${rw}/ext/$1/src"
  printf '[package]\nname = "%s"\nversion = "0.1.0"\nedition = "2021"\nrepository = "%s"\n' "$1" "$2" >"${rw}/ext/$1/Cargo.toml"
  printf '%s\n' 'pub struct Handle;' >"${rw}/ext/$1/src/lib.rs"
}
mk_ext plain-dummy "https://example.invalid/plain"
mk_ext wgpu "https://github.com/gfx-rs/wgpu-dummy"
# 名前は無害で、repository だけが広い定義に当たる crate
mk_ext gfx-lookalike "https://github.com/gfx-rs/lookalike"

real_git() {
  GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${rw}/ws" \
    -c core.hooksPath=/dev/null -c user.name=test -c user.email=test@example.invalid "$@"
}
# real_app <依存の節> <lib.rs の中身>: api crate を書き換え、ロックを作り直してコミットする。
real_app() {
  {
    printf '[package]\nname = "api"\nversion = "0.1.0"\nedition = "2021"\nrepository = "https://example.invalid/api"\n\n'
    printf '%s\n' "$1"
  } >"${rw}/ws/baseline-wgpu/crates/api/Cargo.toml"
  printf '%s\n' "$2" >"${rw}/ws/baseline-wgpu/crates/api/src/lib.rs"
  (cd "${rw}/ws" && cargo generate-lockfile --offline --manifest-path baseline-wgpu/Cargo.toml) >/dev/null 2>"${ERR_F}" \
    && real_git add -A >/dev/null 2>>"${ERR_F}" \
    && { real_git commit -q -m fixture >/dev/null 2>>"${ERR_F}" || true; }
}
# run_real: 写した本体を引数なしで流す。共有の target ディレクトリへ書かせない。
run_real() {
  rc=0
  (cd "${rw}/ws" && env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR CARGO_NET_OFFLINE=true \
    ./scripts/baseline-positive-control.sh) >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

if real_git init -q >/dev/null 2>"${ERR_F}"; then
  # (a) wgpu に依存し、公開面にも出す → 3 定義とも検出
  if real_app '[dependencies]
wgpu = { path = "../../../../ext/wgpu" }' 'pub fn handle() -> wgpu::Handle { wgpu::Handle }'; then
    run_real
    judge "real_all_detected" 0 "o:definition=narrow count=" "o:definition=wide count=" "o:definition=public-api count=" \
      "o:ok: positive control detected wgpu in baseline-wgpu (narrow, wide, public-api)" "o:record: " "!positive-control-failed"
    count_is "real_all_detected_three_lines" "${OUT_F}" "result=detected" 3
  else
    report_fail "real_all_detected_setup" "cannot prepare"
  fi
  # (b) 無害な依存だけ → 3 定義とも 0 件で失敗
  if real_app '[dependencies]
plain-dummy = { path = "../../../../ext/plain-dummy" }' 'pub fn handle() -> plain_dummy::Handle { plain_dummy::Handle }'; then
    run_real
    judge "real_none" 1 "e:definition=narrow count=0 result=no-detection" "e:definition=wide count=0 result=no-detection" \
      "e:definition=public-api count=0 result=no-detection" "!ok: positive control"
    count_is "real_none_three_failures" "${ERR_F}" "result=no-detection" 3
  else
    report_fail "real_none_setup" "cannot prepare"
  fi
  # (c) repository だけが広い定義に当たる依存 → 広い定義だけ検出で失敗（定義ごとに判定している証拠）
  if real_app '[dependencies]
gfx-lookalike = { path = "../../../../ext/gfx-lookalike" }' 'pub struct Own;'; then
    run_real
    judge "real_wide_only" 1 "e:definition=narrow count=0 result=no-detection" "o:definition=wide count=1 result=detected" \
      "e:definition=public-api count=0 result=no-detection" "!ok: positive control"
  else
    report_fail "real_wide_only_setup" "cannot prepare"
  fi
  # (d) wgpu に依存するが公開面には出さない → 公開 API だけ 0 件で失敗
  if real_app '[dependencies]
wgpu = { path = "../../../../ext/wgpu" }' 'pub struct Own;'; then
    run_real
    judge "real_not_exposed" 1 "o:definition=narrow count=" "o:definition=wide count=" \
      "e:definition=public-api count=0 result=no-detection" "!ok: positive control"
  else
    report_fail "real_not_exposed_setup" "cannot prepare"
  fi
else
  report_fail "real_fixture_setup" "cannot prepare the temporary workspace"
fi

echo "passed: ${passed}, failed: ${failed}"
[ "${failed}" -eq 0 ]
