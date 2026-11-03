#!/usr/bin/env bash
# exec-deps-check.sh の回帰テスト（spec TASK-259・BUILD-55 のポインタ。親 Issue #474 の 2/4）。
# 節の構成:
#   1. 合成の節: 一時ディレクトリに合成の crate と metadata の JSON・tree の文章を作り、--metadata-json /
#      --tree-text で渡す。(a) 依存の宣言・(b) 依存グラフ・(c) ソースの crate 名の合格・違反と、誤用の終了コード 2 を確かめる。
#      cargo・git を呼ばない。
#   2. ツールが無いとき: PATH を絞った環境で python3・cargo・git の未導入が終了コード 2 になることを確かめる。
#   3. 偽のコマンド: PATH の先頭に置いた偽の cargo / git で、引数なしの経路（cargo を流す経路）を通す。
#   4. 一時 workspace の実物: 実行時に作る空の path crate の一時 workspace に検査を写し、実物の cargo metadata /
#      cargo tree で通す。向きの違う依存を 1 つずつ足して落ちること（陽性対照）を確かめる。依存は path だけで、
#      ネットワークは使わない。
#   5. 実物のリポジトリ: crates/exec に当てた合格を 1 件。件数の値は固定しない。
# リポジトリのファイルを変更しない（5 は git の読み取りと cargo metadata / cargo tree --locked --offline だけ）。
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」。違反・誤用のケースでは `ok:` が出ないことも確かめる。
# 期待値を緩めない。現行の検査と食い違ったらテストでなく検査側の挙動として直す。
# 前提: python3・cargo・git・rust-toolchain.toml の toolchain（無ければ失敗にし、skip しない）。
# 呼び出し元: `make test-exec-deps-check`。
# 途中で止めず、最後に件数を出す。bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"
require_cmd python3
require_cmd cargo
require_cmd git

CHECK="${DIR}/exec-deps-check.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-exec-deps-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
passed=0
failed=0
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
rc=0

report_ok() {
    passed=$((passed + 1))
}

report_fail() {
    failed=$((failed + 1))
    echo "FAIL: $1: $2" >&2
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

# count_is <名前> <ファイル> <固定文字列> <件数>: 固定文字列を含む行数が一致すること。
count_is() {
    local name="$1" file="$2" word="$3" want="$4" got
    got="$(grep -cF -e "${word}" "${file}" || true)"
    if [ "${got}" = "${want}" ]; then
        report_ok "${name}"
    else
        report_fail "${name}" "expected ${want} line(s) containing '${word}', got ${got}"
    fi
}

# ---- 1. 合成の節 ----
# 差し替え用の入力。runsyn が読み、実行のあとに既定へ戻す。@D@ はケースのディレクトリに置換する。
OK_DEP='{"name":"fandhe-silicon-core","kind":null,"path":"@D@/crates/core"}'
DEPS="${OK_DEP}"
TREE_EXTRA=""
TREE_RAW=""
MD_RAW=""
DUP_EXEC=0
EXEC_REL="exec"

# mkcase <名前>: ${work}/<名前> に crates/exec（Cargo.toml と src/lib.rs）と crates/core を作る。
mkcase() {
    local d="${work}/$1"
    mkdir -p "${d}/crates/exec/src" "${d}/crates/core"
    printf '[package]\nname = "e"\nversion = "0.1.0"\n' >"${d}/crates/exec/Cargo.toml"
    printf '%s\n' '// fixture' >"${d}/crates/exec/src/lib.rs"
}

# runsyn <名前> <期待コード> [語...]: mkcase 済みのケースの metadata と tree を作って --metadata-json / --tree-text で流し、judge する。
runsyn() {
    local n="$1" want="$2" d="${work}/$1" members pkgs dup="" deps
    shift 2
    members='["c","e","k","u","w"'
    if [ "${DUP_EXEC}" = 1 ]; then
        members="${members},\"e2\""
        dup=",{\"id\":\"e2\",\"name\":\"fandhe-silicon-exec\",\"manifest_path\":\"${d}/crates/exec/Cargo.toml\",\"dependencies\":[]}"
    fi
    members="${members}]"
    deps="${DEPS//@D@/${d}}"
    pkgs="{\"id\":\"c\",\"name\":\"fandhe-silicon-core\",\"manifest_path\":\"${d}/crates/core/Cargo.toml\",\"dependencies\":[]}"
    pkgs="${pkgs},{\"id\":\"e\",\"name\":\"fandhe-silicon-exec\",\"manifest_path\":\"${d}/crates/${EXEC_REL}/Cargo.toml\",\"dependencies\":[${deps}]}"
    pkgs="${pkgs},{\"id\":\"k\",\"name\":\"fandhe-silicon-contract\",\"manifest_path\":\"${d}/crates/contract/Cargo.toml\",\"dependencies\":[]}"
    pkgs="${pkgs},{\"id\":\"u\",\"name\":\"fandhe-silicon-upper\",\"manifest_path\":\"${d}/crates/upper/Cargo.toml\",\"dependencies\":[]}"
    pkgs="${pkgs},{\"id\":\"w\",\"name\":\"fandhe-silicon-wasm-abi\",\"manifest_path\":\"${d}/crates/wasm-abi/Cargo.toml\",\"dependencies\":[]}${dup}"
    if [ -n "${MD_RAW}" ]; then
        printf '%s' "${MD_RAW}" >"${d}/md.json"
    else
        printf '{"workspace_root":"%s","workspace_members":%s,"packages":[%s]}' "${d}" "${members}" "${pkgs}" >"${d}/md.json"
    fi
    if [ -n "${TREE_RAW}" ]; then
        printf '%s\n' "${TREE_RAW//@D@/${d}}" >"${d}/tree.txt"
    else
        {
            printf '0fandhe-silicon-exec v0.1.0 (%s/crates/exec)\n' "${d}"
            printf '1fandhe-silicon-core v0.1.0 (%s/crates/core)\n' "${d}"
            printf '1fandhe-silicon-core v0.1.0 (%s/crates/core) (*)\n' "${d}"
            if [ -n "${TREE_EXTRA}" ]; then
                printf '%s\n' "${TREE_EXTRA}"
            fi
        } >"${d}/tree.txt"
    fi
    rc=0
    "${CHECK}" --metadata-json "${d}/md.json" --tree-text "${d}/tree.txt" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
    DEPS="${OK_DEP}"
    TREE_EXTRA=""
    TREE_RAW=""
    MD_RAW=""
    DUP_EXEC=0
    EXEC_REL="exec"
    judge "${n}" "${want}" "!${work}" "$@"
}

# syn <名前> <期待コード> [語...]: mkcase + runsyn。
syn() {
    mkcase "$1"
    runsyn "$@"
}

# raw <名前> <期待コード> <引数...>: 引数をそのまま渡して流す（誤用の確認用）。
raw() {
    local n="$1" want="$2"
    shift 2
    rc=0
    "${CHECK}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
    judge "${n}" "${want}" "!${work}" "!ok: " "!record: "
}

# 合格
syn ok 0 "o:ok: " "o:record: commit=- " "!violation" "!error:"

# (a) 宣言の違反: 種類 × 名前
for kind in normal build dev; do
    if [ "${kind}" = normal ]; then kj=null; else kj="\"${kind}\""; fi
    for nm in fandhe-silicon-contract fandhe-silicon-upper fandhe-silicon-wasm-abi \
        objc2 objc2-foundation objc2-metal objc2-core-foundation ash cudarc plain-dummy; do
        DEPS="${OK_DEP},{\"name\":\"${nm}\",\"kind\":${kj},\"path\":null}"
        syn "decl_${kind}_${nm}" 1 "e:violation: check=dep-allow kind=${kind} name=${nm}" "e:FAILED: 1 violation(s)" "!ok: "
    done
done
DEPS="${OK_DEP},{\"name\":\"fandhe-silicon-contract\",\"kind\":null,\"rename\":\"alias\",\"path\":\"@D@/crates/contract\"}"
syn decl_renamed 1 "e:violation: check=dep-allow kind=normal name=fandhe-silicon-contract" "!ok: "
DEPS="${OK_DEP},{\"name\":\"ash\",\"kind\":null,\"target\":\"cfg(target_os = \\\"macos\\\")\",\"path\":null}"
syn decl_target 1 "e:violation: check=dep-allow kind=normal name=ash" "!ok: "
DEPS="${OK_DEP},{\"name\":\"ash\",\"kind\":null,\"optional\":true,\"path\":null}"
syn decl_optional 1 "e:violation: check=dep-allow kind=normal name=ash" "!ok: "
for kind in normal build dev; do
    if [ "${kind}" = normal ]; then kj=null; else kj="\"${kind}\""; fi
    DEPS="{\"name\":\"fandhe-silicon-core\",\"kind\":${kj},\"path\":null}"
    syn "decl_source_registry_${kind}" 1 "e:violation: check=dep-source kind=${kind} name=fandhe-silicon-core" "!check=dep-allow" "!ok: "
    DEPS="{\"name\":\"fandhe-silicon-core\",\"kind\":${kj},\"path\":\"@D@/crates/other\"}"
    syn "decl_source_other_${kind}" 1 "e:violation: check=dep-source kind=${kind} name=fandhe-silicon-core" "!check=dep-allow" "!ok: "
done

# (b) 依存グラフの違反と合格
TREE_EXTRA='1plain-dummy v0.1.0'
syn tree_direct_other 1 "e:violation: check=tree-direct name=plain-dummy" "!check=tree-forbidden" "!ok: "
TREE_EXTRA='1fandhe-silicon-contract v0.1.0 (/x)'
syn tree_direct_contract 1 "e:violation: check=tree-direct name=fandhe-silicon-contract" \
    "e:violation: check=tree-forbidden depth=1 name=fandhe-silicon-contract" "!ok: "
TREE_EXTRA='2fandhe-silicon-upper v0.1.0 (/x)'
syn tree_depth2_upper 1 "e:violation: check=tree-forbidden depth=2 name=fandhe-silicon-upper" "!check=tree-direct" "!ok: "
for nm in objc2 objc2-foundation objc2-metal objc2-core-foundation ash cudarc fandhe-silicon-wasm-abi; do
    TREE_EXTRA="3${nm} v0.1.0"
    syn "tree_depth3_${nm}" 1 "e:violation: check=tree-forbidden depth=3 name=${nm}" "!ok: "
done
TREE_EXTRA='2some-external v0.1.0'
syn tree_depth2_external_ok 0 "o:ok: " "!violation"
TREE_RAW='0fandhe-silicon-core v0.1.0 (@D@/crates/core)'
syn tree_root_not_target 2 "e:the dependency tree root is not the target crate" "!ok: " "!record: "
TREE_RAW='this is not a tree line'
syn tree_unparsable 2 "e:cannot parse a dependency tree line" "!ok: " "!record: "
TREE_RAW='
'
syn tree_empty 2 "e:the dependency tree is empty" "!ok: " "!record: "

# (c) ソースの crate 名
mkcase src_use
printf '%s\n' 'use fandhe_silicon_contract::x;' >"${work}/src_use/crates/exec/src/x.rs"
runsyn src_use 1 "e:violation: check=source-name file=src/x.rs line=1 name=fandhe-silicon-contract" \
    "!check=dep-allow" "!check=tree-" "!ok: "
mkcase src_path
mkdir -p "${work}/src_path/crates/exec/tests"
printf '%s\n' '// c' 'fn f() { ash::f(); }' >"${work}/src_path/crates/exec/tests/t.rs"
runsyn src_path 1 "e:violation: check=source-name file=tests/t.rs line=2 name=ash" "!ok: "
mkcase src_raw
printf '%s\n' 'fn main() {}' 'use r#cudarc as c;' >"${work}/src_raw/crates/exec/build.rs"
runsyn src_raw 1 "e:violation: check=source-name file=build.rs line=2 name=cudarc" "!ok: "
mkcase src_not_counted
printf '%s\n' '//! ash' '/// fandhe_silicon_contract' '/* cudarc */' 'const A: &str = "ash";' \
    'const B: &str = r#"ash"#;' >"${work}/src_not_counted/crates/exec/src/y.rs"
runsyn src_not_counted 0 "o:ok: " "!violation"

# 誤用
raw args_metadata_only 2 --metadata-json "${work}/ok/md.json"
raw args_tree_only 2 --tree-text "${work}/ok/tree.txt"
raw args_duplicate 2 --metadata-json "${work}/ok/md.json" --metadata-json "${work}/ok/md.json" --tree-text "${work}/ok/tree.txt"
raw args_duplicate_eq 2 --metadata-json "${work}/ok/md.json" --metadata-json="${work}/ok/md.json" --tree-text "${work}/ok/tree.txt"
raw args_duplicate_tree 2 --metadata-json "${work}/ok/md.json" --tree-text "${work}/ok/tree.txt" --tree-text "${work}/ok/tree.txt"
raw args_unknown 2 --no-such-option
raw args_help 2 -h
rc=0
"${CHECK}" --metadata-json "${work}/ok/md.json" --metadata-json "${work}/ok/md.json" --tree-text "${work}/ok/tree.txt" \
    >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge args_duplicate_msg 2 "e:duplicate option" "!ok: "
MD_RAW='{not json'
syn md_not_json 2 "e:metadata input is not JSON" "!ok: " "!record: "
MD_RAW='[]'
syn md_not_object 2 "e:metadata top level is not an object" "!ok: " "!record: "
EXEC_REL="elsewhere"
syn exec_missing 2 "e:not found exactly once" "!ok: " "!record: " "!scanned:"
DUP_EXEC=1
syn exec_twice 2 "e:not found exactly once" "!ok: " "!record: " "!scanned:"
mkcase no_rs
rm -f "${work}/no_rs/crates/exec/src/lib.rs"
runsyn no_rs 2 "e:no source files found" "!ok: " "!record: "
mkcase link_file
ln -s "${work}/ok/md.json" "${work}/link_file/crates/exec/src/l.rs"
runsyn link_file 2 "e:symbolic link in the crate directory" "!ok: " "!record: "
mkcase link_dir
ln -s "${work}/ok" "${work}/link_dir/crates/exec/src/ld"
runsyn link_dir 2 "e:symbolic link in the crate directory" "!ok: " "!record: "
mkcase non_utf8
printf '\377\376\n' >"${work}/non_utf8/crates/exec/src/b.rs"
runsyn non_utf8 2 "e:cannot read input file" "!ok: " "!record: "
mkcase open_comment
printf '%s\n' '/* never closed' >"${work}/open_comment/crates/exec/src/c.rs"
runsyn open_comment 2 "e:unterminated block comment" "!ok: " "!record: "
mkcase open_string
printf '%s\n' 'const A: &str = "abc;' >"${work}/open_string/crates/exec/src/s.rs"
runsyn open_string 2 "e:unterminated string literal" "!ok: " "!record: "

# ---- 2. ツール未導入（成功にせず終了コード 2）----
bash_bin="$(command -v bash)"
dirname_bin="$(command -v dirname)"
git_bin="$(command -v git)"
py_real="$(python3 -I -c 'import sys; print(sys.executable)')"

nopy="${work}/nopy"
mkdir -p "${nopy}"
ln -s "${dirname_bin}" "${nopy}/dirname"
rc=0
PATH="${nopy}" "${bash_bin}" "${CHECK}" --metadata-json "${work}/ok/md.json" --tree-text "${work}/ok/tree.txt" \
    >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge no_python3 2 "e:required command not found on PATH: python3" "!ok: " "!record: " "!scanned:"

nocargo="${work}/nocargo"
mkdir -p "${nocargo}"
ln -s "${dirname_bin}" "${nocargo}/dirname"
ln -s "${py_real}" "${nocargo}/python3"
ln -s "${git_bin}" "${nocargo}/git"
rc=0
PATH="${nocargo}" "${bash_bin}" "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge no_cargo 2 "e:error: cannot run cargo" "!ok: " "!record: " "!scanned:"

nogit="${work}/nogit"
mkdir -p "${nogit}"
ln -s "${dirname_bin}" "${nogit}/dirname"
ln -s "${py_real}" "${nogit}/python3"
rc=0
PATH="${nogit}" "${bash_bin}" "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge no_git 2 "e:error: cannot resolve the source commit" "!ok: " "!record: " "!scanned:"

# ---- 3. 偽の cargo / git（引数なしの経路。PATH の先頭に置く）----
fakebin="${work}/fakebin"
mkdir -p "${fakebin}"
cat >"${fakebin}/cargo" <<'FAKE'
#!/usr/bin/env bash
mode="${FAKE_CARGO_MODE:-ok}"
if [ -n "${FAKE_ARGS_LOG:-}" ]; then
  echo "$*" >>"${FAKE_ARGS_LOG}"
fi
case "$1" in
  --version)
    if [ "${mode}" = "fail" ]; then
      exit 1
    fi
    echo "cargo 0.0.0 (fake)"
    ;;
  metadata)
    if [ "${mode}" = "metafail" ]; then
      exit 1
    fi
    if [ "${mode}" = "badjson" ]; then
      echo "{not json"
    else
      cat "${FAKE_MD}"
    fi
    ;;
  tree)
    if [ "${mode}" = "treefail" ]; then
      exit 1
    fi
    if [ "${mode}" = "treegarbage" ]; then
      echo "garbage line"
    else
      cat "${FAKE_TREE}"
    fi
    ;;
  *) exit 1 ;;
esac
FAKE
cat >"${fakebin}/git" <<'FAKE'
#!/usr/bin/env bash
case "${FAKE_GIT_MODE:-ok}" in
  fail) exit 1 ;;
  garbage)
    echo "/secret/host/path/not-a-sha"
    exit 0
    ;;
esac
case "$1" in
  rev-parse) echo "0123456789abcdef0123456789abcdef01234567" ;;
  status)
    if [ "${FAKE_GIT_MODE:-ok}" = "dirty" ]; then
      echo " M tracked.rs"
    fi
    ;;
  *) exit 1 ;;
esac
FAKE
chmod +x "${fakebin}/cargo" "${fakebin}/git"

# run_fake <cargo モード> <git モード>: 合成の節の ok ケースの metadata と tree を偽の cargo に返させる。
args_log="${work}/args.log"
run_fake() {
    rc=0
    rm -f "${args_log}"
    PATH="${fakebin}:${PATH}" FAKE_CARGO_MODE="$1" FAKE_GIT_MODE="$2" FAKE_ARGS_LOG="${args_log}" \
        FAKE_MD="${work}/ok/md.json" FAKE_TREE="${work}/ok/tree.txt" \
        "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

run_fake ok ok
judge fake_ok 0 "o:record: commit=0123456789abcdef0123456789abcdef01234567 " "o:ok: " \
    "!-dirty" "!violation" "!error:" "!${work}"
count_is fake_ok_metadata "${args_log}" "metadata --locked --offline --no-deps --format-version 1 --all-features" 1
count_is fake_ok_tree "${args_log}" "tree --locked --offline -p fandhe-silicon-exec --all-features -e normal,build,dev --target all --prefix depth" 1
count_is fake_ok_version "${args_log}" "--version" 1
run_fake ok dirty
judge fake_dirty 0 "o:commit=0123456789abcdef0123456789abcdef01234567-dirty " "o:ok: " "!violation"
run_fake fail ok
judge fake_cargo_version_fails 2 "e:error: cargo --version failed" "!ok: " "!record: "
run_fake metafail ok
judge fake_metadata_fails 2 "e:error: cargo metadata failed" "!ok: " "!record: "
run_fake badjson ok
judge fake_metadata_not_json 2 "e:error: cargo metadata output is not JSON" "!ok: " "!record: "
run_fake treefail ok
judge fake_tree_fails 2 "e:error: cargo tree failed" "!ok: " "!record: "
run_fake treegarbage ok
judge fake_tree_garbage 2 "e:cannot parse a dependency tree line" "!ok: " "!record: "
run_fake ok fail
judge fake_git_fails 2 "e:error: cannot resolve the source commit" "!ok: " "!record: " "!scanned:"
run_fake ok garbage
judge fake_git_garbage 2 "e:error: cannot resolve the source commit" "!/secret/host/path" "!ok: " "!record: "

# ---- 4. 一時 workspace の実物（実物の cargo metadata / cargo tree。引数なしの経路）----
rw="${work}/real"
mkdir -p "${rw}/ws/scripts" "${rw}/ext"
for c in core exec contract upper wasm-abi; do
    mkdir -p "${rw}/ws/crates/${c}/src"
    printf '[package]\nname = "fandhe-silicon-%s"\nversion = "0.1.0"\nedition = "2021"\n' "${c}" >"${rw}/ws/crates/${c}/Cargo.toml"
    printf '%s\n' '// fixture' >"${rw}/ws/crates/${c}/src/lib.rs"
done
cat >"${rw}/ws/Cargo.toml" <<'TOML'
[workspace]
members = ["crates/core", "crates/exec", "crates/contract", "crates/upper", "crates/wasm-abi"]
resolver = "2"
TOML
cp "${DIR}/exec-deps-check.sh" "${DIR}/lib.sh" "${rw}/ws/scripts/"
chmod +x "${rw}/ws/scripts/exec-deps-check.sh"
if [ -f "${DIR}/../rust-toolchain.toml" ]; then
    cp "${DIR}/../rust-toolchain.toml" "${rw}/ws/rust-toolchain.toml"
fi

# mk_ext <名前>: ${rw}/ext/<名前> に空の合成 crate を作る（ws の外。path 依存だけで取得は要らない）。
mk_ext() {
    mkdir -p "${rw}/ext/$1/src"
    printf '[package]\nname = "%s"\nversion = "0.1.0"\nedition = "2021"\n' "$1" >"${rw}/ext/$1/Cargo.toml"
    printf '%s\n' '// fixture' >"${rw}/ext/$1/src/lib.rs"
}
mk_ext ash
mk_ext objc2-metal
mk_ext plain-dummy

# real_git: 開発者の全体設定とフックを効かせない git。
real_git() {
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${rw}/ws" \
        -c core.hooksPath=/dev/null -c user.name=test -c user.email=test@example.invalid "$@"
}
# real_commit: ロックを作り直してコミットする（--locked で流すため）。
real_commit() {
    (cd "${rw}/ws" && cargo generate-lockfile --offline) >/dev/null 2>"${ERR_F}" \
        && real_git add -A >/dev/null 2>>"${ERR_F}" \
        && real_git commit -q --allow-empty -m fixture >/dev/null 2>>"${ERR_F}"
}
# real_state <exec の Cargo.toml の追記> [core の Cargo.toml の追記]: exec と core の Cargo.toml と exec のソースを
# 既定から組み直してコミットする。追記は `[dependencies.<名前>]` のような表の見出しから書く。
RS_LIB='// fixture'
RS_SRC_X=""
real_state() {
    rm -f "${rw}/ws/crates/exec/src/x.rs"
    {
        printf '[package]\nname = "fandhe-silicon-exec"\nversion = "0.1.0"\nedition = "2021"\n\n'
        printf '[dependencies]\nfandhe-silicon-core = { path = "../core" }\n\n'
        printf '[dev-dependencies]\nfandhe-silicon-core = { path = "../core" }\n\n'
        printf '%s\n' "${1:-}"
    } >"${rw}/ws/crates/exec/Cargo.toml"
    {
        printf '[package]\nname = "fandhe-silicon-core"\nversion = "0.1.0"\nedition = "2021"\n\n'
        printf '%s\n' "${2:-}"
    } >"${rw}/ws/crates/core/Cargo.toml"
    printf '%s\n' "${RS_LIB}" >"${rw}/ws/crates/exec/src/lib.rs"
    if [ -n "${RS_SRC_X}" ]; then
        printf '%s\n' "${RS_SRC_X}" >"${rw}/ws/crates/exec/src/x.rs"
    fi
    RS_LIB='// fixture'
    RS_SRC_X=""
    real_commit
}
# run_real: 写した検査を引数なしで流す。共有の target ディレクトリへ書かせない。
run_real() {
    rc=0
    (cd "${rw}/ws" && env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR ./scripts/exec-deps-check.sh) \
        >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}
# real_case <名前> <期待コード> [語...]: run_real + judge。ホストの一時パスが出ないことも確かめる。
real_case() {
    local n="$1" w="$2"
    shift 2
    run_real
    judge "${n}" "${w}" "!${rw}" "$@"
}
# real_bad <名前> <exec の追記> <語...>: 状態を組み直して違反（終了コード 1）を確かめる。
real_bad() {
    local n="$1" add="$2"
    shift 2
    if real_state "${add}"; then
        real_case "${n}" 1 "!ok: " "!error:" "$@"
    else
        report_fail "${n}_setup" "cannot prepare"
    fi
}

if real_git init -q >/dev/null 2>"${ERR_F}" && real_state ""; then
    real_case real_clean 0 "o:ok: " "!-dirty" "!violation" "!error:"
    if grep -Eq '^record: commit=[0-9a-f]{40,64} ' "${OUT_F}"; then
        report_ok real_clean_commit_is_clean_sha
    else
        report_fail real_clean_commit_is_clean_sha "commit must be a full hex sha without -dirty"
    fi
    count_is real_clean_three_scanned "${OUT_F}" "scanned: " 3

    # 追跡済みのファイルを触ると -dirty が付く
    printf '%s\n' '// touched' >>"${rw}/ws/crates/exec/src/lib.rs"
    real_case real_dirty 0 "o:ok: " "!violation"
    if grep -Eq '^record: commit=[0-9a-f]{40,64}-dirty ' "${OUT_F}"; then
        report_ok real_dirty_suffix
    else
        report_fail real_dirty_suffix "commit must end with -dirty"
    fi
    real_git checkout -- . >/dev/null 2>&1 || true

    # 向きの違う依存を 1 つ足すと落ちる（陽性対照）
    real_bad real_contract_normal '[dependencies.fandhe-silicon-contract]
path = "../contract"' \
        "e:violation: check=dep-allow kind=normal name=fandhe-silicon-contract" \
        "e:violation: check=tree-direct name=fandhe-silicon-contract" \
        "e:violation: check=tree-forbidden depth=1 name=fandhe-silicon-contract"
    real_bad real_contract_dev '[dev-dependencies.fandhe-silicon-contract]
path = "../contract"' \
        "e:violation: check=dep-allow kind=dev name=fandhe-silicon-contract" \
        "e:violation: check=tree-direct name=fandhe-silicon-contract" \
        "e:violation: check=tree-forbidden depth=1 name=fandhe-silicon-contract"
    real_bad real_chip_build '[build-dependencies.ash]
path = "../../../ext/ash"' \
        "e:violation: check=dep-allow kind=build name=ash" \
        "e:violation: check=tree-direct name=ash" \
        "e:violation: check=tree-forbidden depth=1 name=ash"
    real_bad real_upper_normal '[dependencies.fandhe-silicon-upper]
path = "../upper"' \
        "e:violation: check=dep-allow kind=normal name=fandhe-silicon-upper" \
        "e:violation: check=tree-direct name=fandhe-silicon-upper" \
        "e:violation: check=tree-forbidden depth=1 name=fandhe-silicon-upper"
    real_bad real_chip_target_macos "[target.'cfg(target_os = \"macos\")'.dependencies.objc2-metal]
path = \"../../../ext/objc2-metal\"" \
        "e:violation: check=dep-allow kind=normal name=objc2-metal" \
        "e:violation: check=tree-direct name=objc2-metal" \
        "e:violation: check=tree-forbidden depth=1 name=objc2-metal"
    real_bad real_other_dev '[dev-dependencies.plain-dummy]
path = "../../../ext/plain-dummy"' \
        "e:violation: check=dep-allow kind=dev name=plain-dummy" \
        "e:violation: check=tree-direct name=plain-dummy" "!check=tree-forbidden"

    # core 側に足した依存は exec の宣言には出ず、tree の深さ 2 で落ちる
    if real_state "" '[dependencies.ash]
path = "../../../ext/ash"'; then
        real_case real_chip_in_core 1 "e:violation: check=tree-forbidden depth=2 name=ash" \
            "!check=dep-allow" "!check=tree-direct" "!ok: " "!error:"
    else
        report_fail real_chip_in_core_setup "cannot prepare"
    fi

    # 宣言を変えずにソースだけで落ちる
    RS_LIB='use fandhe_silicon_contract;'
    if real_state ""; then
        real_case real_source_only 1 "e:violation: check=source-name file=src/lib.rs line=1 name=fandhe-silicon-contract" \
            "!check=dep-allow" "!check=tree-" "!ok: " "!error:"
    else
        report_fail real_source_only_setup "cannot prepare"
    fi

    # 実物の cargo metadata / cargo tree の出力をファイルに落として渡しても同じ判定になる（commit は `-`）。
    # real_files <名前> <期待コード> [語...]
    real_files() {
        local n="$1" w="$2"
        shift 2
        if (cd "${rw}/ws" \
            && cargo metadata --locked --offline --no-deps --format-version 1 --all-features >"${rw}/md.json" \
            && cargo tree --locked --offline -p fandhe-silicon-exec --all-features -e normal,build,dev \
                --target all --prefix depth >"${rw}/tree.txt") 2>"${ERR_F}"; then
            rc=0
            "${rw}/ws/scripts/exec-deps-check.sh" --metadata-json "${rw}/md.json" --tree-text "${rw}/tree.txt" \
                >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
            judge "${n}" "${w}" "o:record: commit=- " "!${rw}" "$@"
        else
            report_fail "${n}" "cannot produce cargo output"
        fi
    }
    if real_state ""; then
        real_files real_files_ok 0 "o:ok: " "!violation"
    else
        report_fail real_files_ok_setup "cannot prepare"
    fi
    if real_state '[dependencies.fandhe-silicon-contract]
path = "../contract"'; then
        real_files real_files_bad 1 "e:violation: check=dep-allow kind=normal name=fandhe-silicon-contract" "!ok: "
    else
        report_fail real_files_bad_setup "cannot prepare"
    fi
else
    report_fail real_workspace_setup "cannot prepare the temporary workspace"
fi

# ---- 5. 実物のリポジトリ ----
# 本リポの crates/exec に引数なしで当てる。起きることは git の読み取り（rev-parse・status）と
# cargo metadata / cargo tree --locked --offline だけで、リポジトリのファイルは書かない。
rc=0
"${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge real_repo 0 "o:ok: " "o:record: " "!violation" "!error:" "!${ROOT_DIR}"

echo "passed=${passed} failed=${failed}"
[ "${failed}" -eq 0 ]
