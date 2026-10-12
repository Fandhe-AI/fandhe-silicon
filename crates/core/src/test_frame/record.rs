//! 試験 1 回分の結果（`Outcome`）から、機械可読の記録（JSON の文字列）を組み立てる。
//!
//! 試験を束ねる呼び出し側が、`judge` が返した `Outcome` を記録にしたいときに呼ぶ純粋な関数。
//! 入力は `Outcome` だけで、環境・ファイル・標準出力・標準エラーには触れない（書き出しは持たない）。
//! 値は検査済みの識別子と列挙のトークンだけで、環境変数の値・許可の名前・自由な文は含めない。
//! 文字列の値は [`push_json_string`] を必ず通すので、どんな `&str` が来ても出力は正しい JSON になる。
//!
//! 出す欄は、`Outcome` が値を持つものだけ。キーの順は固定で、出力は決まった形になる。
//!
//! | キー | 型 | 出す条件 |
//! | --- | --- | --- |
//! | `behavior_ids` | 文字列の配列 | 常に（無ければ空の配列） |
//! | `machine_combination` | 文字列 | 走らせた組み合わせがあるとき |
//! | `machine` | 文字列 | 走らせた組み合わせがあるとき |
//! | `label` | 文字列 | 常に |
//! | `label_reason` | 文字列 | ラベルが理由を持つとき |
//!
//! 条件を満たさない欄はキーごと出さない（`null` や空文字で埋めない）。

use super::outcome::Outcome;

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

impl Outcome {
    /// この結果の記録を JSON のオブジェクト 1 つ（改行なし）の文字列で返す。
    ///
    /// キーの順は `behavior_ids`・`machine_combination`・`machine`・`label`・`label_reason`。
    /// 走らせた組み合わせが無い結果（未実施）は `machine_combination`・`machine` を出さない。
    /// 理由を持たないラベル（OK・NG）は `label_reason` を出さない。
    /// 判定保留の `label_reason` は未承認の閾値の ID、未実施・判定不能は理由のトークン。
    pub fn to_record_json(&self) -> String {
        let mut out = String::from("{");
        push_key(&mut out, "behavior_ids");
        out.push('[');
        for (i, b) in self.behaviors().iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            push_json_string(&mut out, b.as_str());
        }
        out.push_str("],");
        if let Some(c) = self.ran_on() {
            push_key(&mut out, "machine_combination");
            push_json_string(&mut out, c.name());
            out.push(',');
            push_key(&mut out, "machine");
            push_json_string(&mut out, c.machine().name());
            out.push(',');
        }
        push_key(&mut out, "label");
        push_json_string(&mut out, self.label().name());
        if let Some(reason) = self.label().reason_token() {
            out.push(',');
            push_key(&mut out, "label_reason");
            push_json_string(&mut out, reason);
        }
        out.push('}');
        out
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
