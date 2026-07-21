//! Unit tests ported from `internal/lexer/lexer_test.go`.
//!
//! Each test mirrors a Go test of the same intent, asserting the token-kind
//! sequence (and, where the Go test does, byte ranges / slices).

use super::*;

/// Asserts the full kind sequence produced by `next_token`, then EOF.
fn assert_kinds(source: &[u8], expected: &[TokenKind]) {
    let mut lexer = Lexer::init(source);
    for (i, &want) in expected.iter().enumerate() {
        let tok = lexer
            .next_token()
            .unwrap_or_else(|| panic!("unexpected EOF at token {i}, wanted {want:?}"));
        assert_eq!(
            tok.kind, want,
            "token {i}: expected {want:?}, got {:?}",
            tok.kind
        );
    }
    assert!(
        lexer.next_token().is_none(),
        "expected EOF but got another token"
    );
}

use TokenKind::*;

#[test]
fn identifier() {
    let source = b"key1";
    let mut lexer = Lexer::init(source);
    let tok = lexer.next_token().expect("expected token");
    assert_eq!(tok.value(source), b"key1");
    assert_eq!(tok.kind, Identifier);
}

#[test]
fn key_equals_value() {
    assert_kinds(b"key = value", &[Identifier, Equal, Identifier]);
}

#[test]
fn numbers() {
    assert_kinds(b"key = 108", &[Identifier, Equal, LiteralNumber]);
}

#[test]
fn decimal_number() {
    assert_kinds(b"value = 0.1", &[Identifier, Equal, LiteralNumber]);
}

#[test]
fn strings() {
    assert_kinds(br#""test string""#, &[LiteralString]);
}

#[test]
fn strings_not_terminated() {
    assert_kinds(br#""not terminated string"#, &[Invalid]);
}

#[test]
fn booleans() {
    assert_kinds(
        b"is_yes = yes\nis_no = no",
        &[
            Identifier,
            Equal,
            LiteralBoolean,
            Identifier,
            Equal,
            LiteralBoolean,
        ],
    );
}

#[test]
fn blocks() {
    assert_kinds(
        b"limit = {\n\tage > 18\n\t}",
        &[
            Identifier,
            Equal,
            LBrace,
            Identifier,
            GreaterThan,
            LiteralNumber,
            RBrace,
        ],
    );
}

#[test]
fn scope_operators() {
    assert_kinds(
        b"key1 = title:k_france.capital\nkey2 = @some_var",
        &[
            Identifier,
            Equal,
            Identifier,
            Colon,
            Identifier,
            Dot,
            Identifier, // key1
            Identifier,
            Equal,
            ScriptValue, // key2
        ],
    );
}

#[test]
fn hyphenated_identifier() {
    assert_kinds(
        b"c_anti-atlas = { x = 1 }",
        &[
            Identifier,
            Equal,
            LBrace,
            Identifier,
            Equal,
            LiteralNumber,
            RBrace,
        ],
    );
}

#[test]
fn hyphen_not_absorbed_as_minus() {
    assert_kinds(
        b"key = -1\ndiff = a - b",
        &[
            Identifier,
            Equal,
            Minus,
            LiteralNumber, // key = -1
            Identifier,
            Equal,
            Identifier,
            Minus,
            Identifier, // diff = a - b
        ],
    );
}

#[test]
fn negative_date() {
    assert_kinds(
        b"-221.1.1 = { x = 1 }",
        &[
            LiteralDate,
            Equal,
            LBrace,
            Identifier,
            Equal,
            LiteralNumber,
            RBrace,
        ],
    );
}

#[test]
fn negative_number_not_a_date() {
    assert_kinds(b"x = -0.25", &[Identifier, Equal, Minus, LiteralNumber]);
}

#[test]
fn dot_floats() {
    assert_kinds(
        b"color = { .7 0.05 1. }",
        &[
            Identifier,
            Equal,
            LBrace,
            LiteralNumber,
            LiteralNumber,
            LiteralNumber,
            RBrace,
        ],
    );
}

#[test]
fn date_literal() {
    assert_kinds(
        b"creation_date = 1099.1.1",
        &[Identifier, Equal, LiteralDate],
    );
}

#[test]
fn dot_after_identifier_is_scope_chain() {
    assert_kinds(
        b"test.0001 = { }",
        &[Identifier, Dot, LiteralNumber, Equal, LBrace, RBrace],
    );
}

#[test]
fn dot_still_scope_operator() {
    assert_kinds(
        b"x = title:k_france.capital",
        &[
            Identifier, Equal, Identifier, Colon, Identifier, Dot, Identifier,
        ],
    );
}

#[test]
fn percent_in_value() {
    assert_kinds(
        b"size = { 28% 52% }",
        &[Identifier, Equal, LBrace, Identifier, Identifier, RBrace],
    );
}

#[test]
fn percent_in_identifier() {
    assert_kinds(b"SUCCESS_% = 90", &[Identifier, Equal, LiteralNumber]);
}

#[test]
fn slash_path_value() {
    assert_kinds(
        b"reference = event:/SFX/Events/Themes/generic",
        &[Identifier, Equal, Identifier, Colon, Identifier],
    );
}

#[test]
fn unquoted_slash_path() {
    assert_kinds(
        b"name = gfx/court_scene/scene_settings",
        &[Identifier, Equal, Identifier],
    );
}

#[test]
fn script_values() {
    assert_kinds(
        b"@my_const = 0.15\nkey = @my_const\nneg = @[my_const * -1]",
        &[
            ScriptValue,
            Equal,
            LiteralNumber, // @my_const = 0.15
            Identifier,
            Equal,
            ScriptValue, // key = @my_const
            Identifier,
            Equal,
            ScriptMath, // neg = @[my_const * -1]
        ],
    );
}

#[test]
fn greater_less_equal_operators() {
    assert_kinds(
        b"age > 18\nage < 18\nage >= 18\nage <= 18",
        &[
            Identifier,
            GreaterThan,
            LiteralNumber,
            Identifier,
            LessThan,
            LiteralNumber,
            Identifier,
            GreaterEqual,
            LiteralNumber,
            Identifier,
            LessEqual,
            LiteralNumber,
        ],
    );
}

#[test]
fn different_equal_operators() {
    assert_kinds(
        b"capital = c_france\nage == 18\nage != 18\nthis ?= c_france",
        &[
            Identifier,
            Equal,
            Identifier,
            Identifier,
            EqualEqual,
            LiteralNumber,
            Identifier,
            NotEqual,
            LiteralNumber,
            Identifier,
            QuestionEqual,
            Identifier,
        ],
    );
}

#[test]
fn invalid_tokens() {
    assert_kinds(
        b"something!\nsomething?",
        &[Identifier, Invalid, Identifier, Invalid],
    );
}

#[test]
fn identifier_can_start_from_number() {
    assert_kinds(b"8_something", &[Identifier]);
}

#[test]
fn utf8_bom() {
    let mut source = UTF8_BOM.to_vec();
    source.extend_from_slice(b"key = value");
    assert_kinds(&source, &[Identifier, Equal, Identifier]);
}

#[test]
fn skipping_bom_does_not_break_value() {
    let mut source = UTF8_BOM.to_vec();
    source.extend_from_slice(b"key = value");
    let mut lexer = Lexer::init(&source);
    let tok = lexer.next_token().expect("expected token");
    assert_eq!(tok.range.start, 3, "BOM is 3 bytes");
    assert_eq!(tok.range.end, 6, "\"key\" ends at 6");
    assert_eq!(tok.value(&source), b"key");
}

#[test]
fn skip_comments() {
    assert_kinds(
        b"key = value # something commented\nkey = value",
        &[Identifier, Equal, Identifier, Identifier, Equal, Identifier],
    );
}

#[test]
fn identifier_can_contain_ampersand() {
    assert_kinds(
        b"ghw_region_finland_&_estonia = something",
        &[Identifier, Equal, Identifier],
    );
}

#[test]
fn utf8_identifier() {
    assert_kinds(
        "flag:Linnéa José".as_bytes(),
        &[Identifier, Colon, Identifier, Identifier],
    );
}

#[test]
fn utf8_identifier_values() {
    let source = "flag:Linnéa José".as_bytes();
    let mut lexer = Lexer::init(source);
    for want in ["flag", ":", "Linnéa", "José"] {
        let tok = lexer
            .next_token()
            .unwrap_or_else(|| panic!("unexpected EOF, wanted {want:?}"));
        assert_eq!(tok.value(source), want.as_bytes(), "wanted {want:?}");
    }
    assert!(lexer.next_token().is_none(), "expected EOF");
}

#[test]
fn macro_param() {
    assert_kinds(b"$CHILD$", &[MacroParam]);
    assert_kinds(b"exists = $CHILD$", &[Identifier, Equal, MacroParam]);
    assert_kinds(b"$CHILD$ = {", &[MacroParam, Equal, LBrace]);
    assert_kinds(b"$CHILD$.host", &[MacroParam, Dot, Identifier]);
    assert_kinds(b"$", &[Dollar]);
    assert_kinds(b"$$", &[Dollar, Dollar]);

    let src = b"$CHILD$";
    let tok = Lexer::init(src).next_token().unwrap();
    assert_eq!(tok.value(src), b"$CHILD$");
}

#[test]
fn utf8_string_literal() {
    assert_kinds(
        r#"flag = "Linnéa José""#.as_bytes(),
        &[Identifier, Equal, LiteralString],
    );
}

#[test]
fn utf8_string_value() {
    let source = r#"flag = "Linnéa José""#.as_bytes();
    let mut lexer = Lexer::init(source);
    lexer.next_token(); // flag
    lexer.next_token(); // =
    let tok = lexer.next_token().expect("expected token");
    assert_eq!(tok.kind, LiteralString);
    assert_eq!(tok.value(source), r#""Linnéa José""#.as_bytes());
}

#[test]
fn tokenize_skips_invalid() {
    // `tokenize` mirrors Go's `Tokenize`: invalid tokens are dropped.
    let toks = tokenize(b"something!");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].kind, Identifier);
}

#[test]
fn every_token_slice_matches_range() {
    // The source slice of a token is exactly source[start..end].
    let source = "a = { b = 1.5 c = @[x*2] d = \"s\" }".as_bytes();
    let mut lexer = Lexer::init(source);
    while let Some(tok) = lexer.next_token() {
        let slice = &source[tok.range.as_range()];
        assert_eq!(tok.value(source), slice);
    }
}

#[test]
fn token_kind_u8_roundtrip() {
    // ALL must list every variant in discriminant order, or from_u8 lies.
    for (i, kind) in TokenKind::ALL.iter().enumerate() {
        assert_eq!(*kind as usize, i, "ALL[{i}] out of order: {kind:?}");
        assert_eq!(TokenKind::from_u8(i as u8), Some(*kind));
    }
    assert_eq!(TokenKind::from_u8(TokenKind::ALL.len() as u8), None);
}
