//! PDXScript formatter — expand-every-block style.
//!
//! Style (user-decided for this toolchain):
//! - every block containing a field, nested block, or comment expands: one
//!   entry per line, tab indentation, `}` on its own line;
//! - scalar-only lists stay inline while they fit (`color = { 255 0 0 }`),
//!   and empty blocks render as `{ }`;
//! - single spaces around operators; comments and blank-line grouping are
//!   preserved (blank runs collapse to one line);
//! - output is always `\n`-terminated, LF-only, BOM-free.
//!
//! Implementation is token+gap based, NOT tree based: the parser drops
//! comments and its Block/File nodes carry zero-width ranges, while the raw
//! token stream plus the whitespace gaps between tokens carry everything a
//! formatter needs (see [`trivia`]). The parser still gates the operation —
//! files with parse diagnostics are refused, because formatting an
//! error-recovered structure is destructive. Every run ends with a re-lex
//! verification ([`verify`]): output and input must yield identical token
//! and comment sequences, or the result is discarded.

mod emit;
mod fields;
mod trivia;
mod verify;

use pdxl_parser::Diagnostic;

/// Why a file could not be formatted.
#[derive(Debug)]
pub enum FmtError {
    /// The file has parse errors; formatting an error-recovered tree is
    /// destructive, so it is refused. Fix the syntax first.
    ParseDiagnostics(Vec<Diagnostic>),
    /// The file contains bytes the lexer marks invalid (some non-script
    /// `.txt` files — vanilla defines — aren't PDXScript); refused.
    Unsupported,
    /// Internal bug guard: the output did not re-lex to the input's token
    /// stream. The formatted text is discarded; the file must be left
    /// untouched.
    Verify { detail: String },
}

impl std::fmt::Display for FmtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FmtError::ParseDiagnostics(diags) => {
                write!(
                    f,
                    "file has {} parse diagnostic(s); not formatting",
                    diags.len()
                )
            }
            FmtError::Unsupported => {
                write!(
                    f,
                    "contains tokens outside the script grammar; not formatting"
                )
            }
            FmtError::Verify { detail } => {
                write!(f, "internal formatter error (output discarded): {detail}")
            }
        }
    }
}

impl std::error::Error for FmtError {}

/// Formats a whole file. Empty input formats to empty output; anything else
/// gets exactly one trailing newline.
pub fn format(filename: &str, src: &[u8]) -> Result<String, FmtError> {
    let parsed = pdxl_parser::parse(filename.to_string(), src.to_vec());
    if !parsed.diagnostics().is_empty() {
        return Err(FmtError::ParseDiagnostics(parsed.diagnostics().to_vec()));
    }
    let items = trivia::scan(src).ok_or(FmtError::Unsupported)?;
    let out = emit::emit(&items, &emit::Options::default());
    if let Some(detail) = verify::divergence(&items, &out) {
        return Err(FmtError::Verify { detail });
    }
    Ok(out)
}

/// Whether `src` is already in formatted form (byte-identical to
/// [`format`]'s output).
pub fn is_formatted(filename: &str, src: &[u8]) -> Result<bool, FmtError> {
    Ok(format(filename, src)?.as_bytes() == src)
}

/// Formats layout and experimentally reorders fields in schema-recognized
/// structural blocks. Repeated fields retain their relative order; unknown
/// fields move after known fields but remain stable among themselves.
pub fn format_fields(
    filename: &str,
    rel_path: &str,
    src: &[u8],
    schema: &pdxl_analysis::context::ContextSchema,
) -> Result<String, FmtError> {
    let items = trivia::scan(src).ok_or(FmtError::Unsupported)?;
    let laid_out = format(filename, src)?;
    let parsed = pdxl_parser::parse(filename.to_string(), laid_out.as_bytes().to_vec());
    if !parsed.diagnostics().is_empty() {
        return Err(FmtError::ParseDiagnostics(parsed.diagnostics().to_vec()));
    }
    let (tree, _) = parsed.into_parts();
    let out = fields::reorder(&laid_out, &tree, rel_path, schema);
    if let Some(detail) = verify::inventory_divergence(&items, &out) {
        return Err(FmtError::Verify { detail });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(src: &str) -> String {
        format("test.txt", src.as_bytes()).expect("formats")
    }

    #[test]
    fn expands_dense_nested_fields() {
        assert_eq!(
            fmt(
                "option = { name = a.1 scope:scheme = { add_scheme_modifier = { type = x days = 370 } } }\n"
            ),
            "option = {\n\
             \tname = a.1\n\
             \tscope:scheme = {\n\
             \t\tadd_scheme_modifier = {\n\
             \t\t\ttype = x\n\
             \t\t\tdays = 370\n\
             \t\t}\n\
             \t}\n\
             }\n"
        );
    }

    #[test]
    fn scalar_only_lists_stay_inline() {
        assert_eq!(fmt("color = { 255 0 0 }\n"), "color = { 255 0 0 }\n");
        assert_eq!(
            fmt("events = {\n\ta.1\n\ta.2\n}\n"),
            "events = { a.1 a.2 }\n"
        );
    }

    #[test]
    fn long_scalar_lists_expand() {
        let names: Vec<String> = (0..30).map(|i| format!("name_number_{i}")).collect();
        let src = format!("male_names = {{ {} }}\n", names.join(" "));
        let out = fmt(&src);
        assert!(out.starts_with("male_names = {\n\tname_number_0\n"));
        assert!(out.ends_with("\tname_number_29\n}\n"));
    }

    #[test]
    fn empty_blocks_stay_inline() {
        assert_eq!(fmt("a = {}\nb = {\n}\n"), "a = { }\nb = { }\n");
    }

    #[test]
    fn tagged_blocks_keep_tag_inline() {
        assert_eq!(fmt("color = rgb{255 0 0}\n"), "color = rgb { 255 0 0 }\n");
    }

    #[test]
    fn operators_get_single_spaces() {
        assert_eq!(
            fmt("a=1\nb ?=  2\nc>=3\nd == 4\ne!=5\nf<6\ng<=7\nh>8\n"),
            "a = 1\nb ?= 2\nc >= 3\nd == 4\ne != 5\nf < 6\ng <= 7\nh > 8\n"
        );
    }

    #[test]
    fn comments_survive_trailing_and_own_line() {
        assert_eq!(
            fmt("# header\na = { # trailing\n\t# inner\n\tb = 1 # after value\n}\n"),
            "# header\na = { # trailing\n\t# inner\n\tb = 1 # after value\n}\n"
        );
    }

    #[test]
    fn comment_inside_block_forces_expansion() {
        assert_eq!(
            fmt("a = { 1 2 # why\n 3 }\n"),
            "a = {\n\t1\n\t2 # why\n\t3\n}\n"
        );
    }

    #[test]
    fn blank_lines_collapse_to_one() {
        assert_eq!(fmt("a = 1\n\n\n\nb = 2\n"), "a = 1\n\nb = 2\n");
        // …and never at the start of the file.
        assert_eq!(fmt("\n\na = 1\n"), "a = 1\n");
    }

    #[test]
    fn glued_chains_stay_glued() {
        assert_eq!(
            fmt("x = scope:target.var\ntitle:e_hre = { a = b }\n"),
            "x = scope:target.var\ntitle:e_hre = {\n\ta = b\n}\n"
        );
    }

    #[test]
    fn special_tokens_verbatim() {
        assert_eq!(
            fmt("a = \"quoted = text\"\nb = $PARAM$\nc = @[ 1 + x ]\nd = 1066.9.15\ne = @sv\n"),
            "a = \"quoted = text\"\nb = $PARAM$\nc = @[ 1 + x ]\nd = 1066.9.15\ne = @sv\n"
        );
    }

    #[test]
    fn crlf_and_bom_are_normalized() {
        let src = b"\xEF\xBB\xBFa = 1\r\nb = { c = 2 }\r\n";
        assert_eq!(
            format("test.txt", src).unwrap(),
            "a = 1\nb = {\n\tc = 2\n}\n"
        );
    }

    #[test]
    fn comment_only_and_empty_files() {
        assert_eq!(fmt("# just a note\n"), "# just a note\n");
        assert_eq!(fmt(""), "");
    }

    #[test]
    fn weighted_and_list_entries_one_per_line() {
        assert_eq!(
            fmt("random_events = { 100 = t.1 50 = t.2 }\n"),
            "random_events = {\n\t100 = t.1\n\t50 = t.2\n}\n"
        );
    }

    #[test]
    fn typed_definitions_keep_keyword_and_name_on_one_line() {
        assert_eq!(
            fmt("scripted_effect T4N_boost = { add_gold = 5 }\n"),
            "scripted_effect T4N_boost = {\n\tadd_gold = 5\n}\n"
        );
        // …while adjacent completed fields still split (expand-every-block)…
        assert_eq!(
            fmt("random_events = { 100 = t.1 50 = t.2 }\n"),
            "random_events = {\n\t100 = t.1\n\t50 = t.2\n}\n"
        );
        // …and a field right after a closing brace starts its own line.
        assert_eq!(
            fmt("o = { t = { x = yes } add_gold = 5 }\n"),
            "o = {\n\tt = {\n\t\tx = yes\n\t}\n\tadd_gold = 5\n}\n"
        );
    }

    #[test]
    fn schema_fields_reorder_with_comments_and_stable_repeats() {
        use pdxl_analysis::context::ClauseKind::{Config, Struct};
        use pdxl_analysis::context::{ContextSchema, Fallback, FieldSpec, ScalarKind, StructSpec};

        const VALUE: FieldSpec = FieldSpec {
            scalar: Some(ScalarKind::Setting),
            block: Some(Config),
            scope: None,
            doc: None,
            values: None,
            ref_kind: None,
            ref_alt: &[],
        };
        static SPEC: StructSpec = StructSpec {
            name: "test",
            fields: &[("first", VALUE), ("second", VALUE), ("repeat", VALUE)],
            fallback: Fallback::Ignore,
        };
        static SCHEMA: ContextSchema = ContextSchema {
            roots: &[("common/test/", Struct(&SPEC))],
            effect_structs: &[],
        };
        let src = b"x = { unknown = 0\n # second docs\n # attached\n second = 2 repeat = a first = 1 repeat = b }\n";
        let out = format_fields("x.txt", "common/test/x.txt", src, &SCHEMA).unwrap();
        assert_eq!(
            out,
            "x = {\n\
             \tfirst = 1\n\
             \t# second docs\n\
             \t# attached\n\
             \tsecond = 2\n\
             \trepeat = a\n\
             \trepeat = b\n\
             \tunknown = 0\n\
             }\n"
        );
    }

    #[test]
    fn parse_errors_are_refused() {
        let err = format("bad.txt", b"a = {\n").unwrap_err();
        assert!(matches!(err, FmtError::ParseDiagnostics(_)));
    }

    #[test]
    fn idempotent_on_everything_formatted() {
        for src in [
            "option = { name = a.1 trigger = { has_trait = brave } add_gold = 5 }\n",
            "# c\n\na = { b = { c = rgb { 1 2 3 } } } # t\n",
            "events = { a.1 a.2 }\nx = @[ 1+2 ]\n",
        ] {
            let once = fmt(src);
            assert_eq!(fmt(&once), once, "not idempotent for {src:?}");
        }
    }
}
