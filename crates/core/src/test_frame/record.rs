//! 試験 1 回分の結果（`Outcome`）から、試験ごとの記録の部品を作る。
//!
//! 試験を束ねる呼び出し側と、書き出し（`export`）から呼ばれる純粋な関数。入力は `Outcome` だけで、
//! 環境・ファイル・標準出力・標準エラーには触れない。部品は「試験の結果の欄」だけを持つ。
//! run の欄（形の版・run の ID・時刻・環境・経路の証拠・回への参照など）は持たず、
//! それらを足して 1 件の記録に組むのは、部品を受け取る側の役目。
//! 値は検査済みの識別子と列挙のトークンだけで、環境変数の値・許可の名前・自由な文は含めない。
//! 文字列の値は [`push_json_string`] を必ず通すので、どんな `&str` が来ても出力は正しい JSON（ASCII のみ）になる。
//!
//! 部品 1 件の欄は、次の順で並ぶ。条件を満たさない欄はキーごと出さない（`null` や空文字で埋めない）。
//!
//! | 欄 | 値 | 出す条件 |
//! | --- | --- | --- |
//! | `behavior_ids` | 文字列の配列 | 常に（無ければ空の配列） |
//! | `machine_combination` | 文字列 | 常に |
//! | `machine` | 文字列 | 常に |
//! | `label` | 文字列 | 常に |
//! | `label_reason` | 文字列 | ラベルが理由のトークンを持つとき（未実施・判定不能） |
//! | `body_kind` | 文字列（`contract-test`） | 常に |
//! | `body` | オブジェクト（`{"test_id": …}`） | 常に |
//!
//! 部品の件数は、走らせた結果なら 1 件、走らせなかった結果なら宣言した組み合わせの数。
//! spec TASK-23。

use super::combination::MachineCombination;
use super::identifier::TestId;
use super::outcome::Outcome;

/// 本体の種類の値。試験ごとの記録の本体は、すべてこの種類。
pub const BODY_KIND: &str = "contract-test";

/// JSON の文字列リテラル（引用符つき）を `out` の末尾へ足す。文字列の値を出す唯一の入口。
///
/// 引用符・バックスラッシュ・制御文字（U+0000〜U+001F と U+007F）と、ASCII でない文字を
/// エスケープするので、出力は ASCII だけになる。`\n`・`\r`・`\t`・`\b`・`\f` は短い形、
/// ほかの制御文字は `\u00xx`（小文字の 16 進）、BMP の外の文字はサロゲート対の `\uxxxx\uxxxx`。
fn push_json_string(out: &mut String, s: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    fn push_u_escape(out: &mut String, unit: u16) {
        out.push_str("\\u");
        for shift in [12u32, 8, 4, 0] {
            out.push(char::from(HEX[usize::from((unit >> shift) & 0xf)]));
        }
    }
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (' '..='~').contains(&c) => out.push(c),
            c => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    push_u_escape(out, *unit);
                }
            }
        }
    }
    out.push('"');
}

/// `"key":` を足す。キーは呼び出し側の固定のリテラルだけで、実行時の値は渡さない。
fn push_key(out: &mut String, key: &'static str) {
    push_json_string(out, key);
    out.push(':');
}

fn string_value(s: &str) -> String {
    let mut out = String::new();
    push_json_string(&mut out, s);
    out
}

/// 試験ごとの記録の部品 1 件。決まった順の欄を、(欄の名前, 値の JSON の文字列) の列で持つ。
///
/// `Outcome::record_parts` だけが作る。値の文字列は、それ単体で閉じた JSON の値（引用符・括弧つき）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordPart {
    test_id: TestId,
    combination: MachineCombination,
    fields: Vec<(&'static str, String)>,
}

impl RecordPart {
    /// 欄の列。順は固定で、条件を満たさない欄は含まない。
    pub fn fields(&self) -> &[(&'static str, String)] {
        &self.fields
    }

    /// 部品だけを 1 つの JSON オブジェクトにした文字列（1 行。改行なし。ASCII のみ）。
    pub fn to_json(&self) -> String {
        let mut out = String::from("{");
        for (i, (key, value)) in self.fields.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            push_key(&mut out, key);
            out.push_str(value);
        }
        out.push('}');
        out
    }

    /// 試験の ID。
    pub fn test_id(&self) -> &TestId {
        &self.test_id
    }

    /// この部品が表す機材の組み合わせ。
    pub fn combination(&self) -> MachineCombination {
        self.combination
    }
}

impl Outcome {
    /// この結果の記録の部品を返す。
    ///
    /// 走らせた結果は走らせた組み合わせの 1 件、走らせなかった結果は宣言した組み合わせごとに 1 件
    /// （宣言の順）。どの部品も同じラベル・理由・ビヘイビア項目の ID・試験の ID を持つ。
    pub fn record_parts(&self) -> Vec<RecordPart> {
        let combinations: Vec<MachineCombination> = match self.ran_on() {
            Some(c) => vec![c],
            None => self.declared_combinations().to_vec(),
        };
        combinations.into_iter().map(|c| self.part_for(c)).collect()
    }

    fn part_for(&self, c: MachineCombination) -> RecordPart {
        let mut ids = String::from("[");
        for (i, b) in self.behaviors().iter().enumerate() {
            if i > 0 {
                ids.push(',');
            }
            push_json_string(&mut ids, b.as_str());
        }
        ids.push(']');
        let mut fields: Vec<(&'static str, String)> = vec![
            ("behavior_ids", ids),
            ("machine_combination", string_value(c.name())),
            ("machine", string_value(c.machine().name())),
            ("label", string_value(self.label().name())),
        ];
        if let Some(reason) = self.label().reason_name() {
            fields.push(("label_reason", string_value(reason)));
        }
        fields.push(("body_kind", string_value(BODY_KIND)));
        let mut body = String::from("{");
        push_key(&mut body, "test_id");
        push_json_string(&mut body, self.id().as_str());
        body.push('}');
        fields.push(("body", body));
        RecordPart {
            test_id: self.id().clone(),
            combination: c,
            fields,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::push_json_string;

    fn escaped(s: &str) -> String {
        let mut out = String::new();
        push_json_string(&mut out, s);
        out
    }

    /// 期待値の組み立て: 各 16 進 4 桁を `\u` エスケープにして引用符で囲む。
    fn quoted_u(units: &[&str]) -> String {
        let mut s = String::from("\"");
        for u in units {
            s.push('\\');
            s.push('u');
            s.push_str(u);
        }
        s.push('"');
        s
    }

    #[test]
    fn plain_ascii_is_only_quoted() {
        assert_eq!(escaped(""), "\"\"");
        assert_eq!(escaped("abc-XYZ_09 ~"), "\"abc-XYZ_09 ~\"");
    }

    #[test]
    fn quote_and_backslash_are_escaped() {
        assert_eq!(escaped("a\"b\\c"), r#""a\"b\\c""#);
    }

    #[test]
    fn control_characters_use_short_or_u_escapes() {
        assert_eq!(escaped("\n\r\t\u{8}\u{c}"), r#""\n\r\t\b\f""#);
        assert_eq!(
            escaped("\u{0}\u{1}\u{1f}\u{7f}"),
            r#""\u0000\u0001\u001f\u007f""#
        );
    }

    #[test]
    fn non_ascii_is_escaped_and_astral_uses_surrogate_pairs() {
        assert_eq!(escaped("\u{e9}"), quoted_u(&["00e9"]));
        assert_eq!(escaped("\u{65e5}"), quoted_u(&["65e5"]));
        assert_eq!(escaped("\u{2028}"), quoted_u(&["2028"]));
        assert_eq!(escaped("\u{1F600}"), quoted_u(&["d83d", "de00"]));
        assert!(escaped("\u{e9}\u{65e5}\u{1F600}\u{0}\"").is_ascii());
    }
}
