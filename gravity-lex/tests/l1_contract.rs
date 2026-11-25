use gravity_lex::{lex, style};

#[test]
fn style_abi_matches_the_native_php_renderer() {
    let native = include_str!("../../lexilla-rs/vendor/lexilla/lexers/LexPHP.cxx");
    let names = [
        ("DEFAULT", style::DEFAULT), ("COMMENT", style::COMMENT),
        ("COMMENTLINE", style::LINE_COMMENT), ("DOCCOMMENT", style::DOC),
        ("VARIABLE", style::VARIABLE), ("STRING", style::SINGLE),
        ("STRING_VARIABLE", style::INTERPOLATED), ("STRING_DQ", style::DOUBLE),
        ("HEREDOC", style::HEREDOC), ("NOWDOC", style::NOWDOC),
        ("CHARACTER", style::CHARACTER), ("OPERATOR", style::OPERATOR),
        ("NUMBER", style::NUMBER), ("WORD", style::KEYWORD),
        ("WORD2", style::FUNCTION), ("WORD3", style::TYPE),
        ("WORD4", style::CONSTANT), ("IDENTIFIER", style::IDENTIFIER),
        ("LEXERROR", style::ERROR), ("OPEN_TAG", style::OPEN),
        ("CLOSE_TAG", style::CLOSE), ("INLINE_HTML", style::HTML),
    ];
    for (name, id) in names {
        let name = format!("SCE_PHP_{name}");
        let line = native.lines().find(|line| {
            let mut words = line.split_whitespace();
            words.next() == Some("#define") && words.next() == Some(name.as_str())
        }).expect("native PHP style definition");
        let value: u8 = line.split_whitespace().nth(2).unwrap().parse().unwrap();
        assert_eq!(id, value, "{name}");
    }
}

#[test]
fn every_scalar_is_total_and_byte_aligned() {
    // Every Unicode scalar as input, including controls, astral and noncharacters.
    // Invalid UTF-8 is outside the public &str contract.
    let mut input = String::from("<?php ");
    for code in 0..=0x10ffff {
        if let Some(ch) = char::from_u32(code) {
            input.truncate(6);
            input.push(ch);
            let out = lex(&input);
            assert_eq!(out.styles.len(), input.len());
            assert!(out.styles.iter().all(|id| *id <= style::HTML));
            assert!(out.styles[6..].iter().all(|id| *id == out.styles[6]));
        }
    }
}

#[test]
fn basic_token_contract_preserves_every_byte() {
    let source = "<?php // comment\n/* block */ $λ = VALUE + 12; echo ($λ); ?>html";
    let output = lex(source);
    for (word, id) in [
        ("<?php",style::OPEN), ("// comment",style::LINE_COMMENT),
        ("/* block */",style::COMMENT), ("$λ",style::VARIABLE),
        ("VALUE",style::CONSTANT), ("+",style::OPERATOR),
        ("12",style::NUMBER), ("echo",style::KEYWORD),
        ("(",style::OPERATOR), (")",style::OPERATOR),
        (";",style::OPERATOR), ("?>",style::CLOSE), ("html",style::HTML),
    ] {
        let at = source.find(word).unwrap();
        assert!(output.styles[at..at+word.len()].iter().all(|s| *s==id), "{word}");
    }
}
