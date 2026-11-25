use gravity_lex::{lex, style};
fn styled(source: &str, needle: &str, id: u8) {
    let out = lex(source);
    for (at, _) in source.match_indices(needle) {
        let ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
        if needle.chars().all(ident) && (
            source[..at].chars().next_back().is_some_and(ident)
            || source[at+needle.len()..].chars().next().is_some_and(ident)) { continue; }
        assert!(out.styles[at..at+needle.len()].iter().all(|s| *s==id),
            "{needle}: {:?}", &out.styles[at..at+needle.len()]);
    }
    assert!(source.contains(needle), "fixture missing {needle}");
}
#[test]
fn php84_declarations_attributes_and_hooks() {
    let source = r#"<?php
namespace App\Domain;
use Vendor\Package\{Thing, Other as Alias};
use function Vendor\helpers\convert;
use const Vendor\VERSION;
#[\Attribute(\Attribute::TARGET_PROPERTY)]
readonly class Record extends Base implements Contract {
    public private(set) string $name;
    protected(set) int $count = 0;
    public string $label {
        get => $this->name;
        set(string $value) { $this->name = $value; }
    }
    public function convert(?Alias $input): (A&B)|null { return null; }
}
enum State: string { case Ready = 'ready'; }
$fn = static fn(int|float $x): never => throw new \Exception();
"#;
    for n in ["namespace","use","as","function","const","readonly","class","extends","implements","public","private","protected","set","get","return","enum","case","static","fn","throw","new"] {
        styled(source,n,style::KEYWORD);
    }
    for n in ["string","int","float","null","never"] { styled(source,n,style::TYPE); }
    for n in ["Record","Base","Contract","Attribute","TARGET_PROPERTY","State","Ready","Alias","A","B"] {
        // Qualified/user-defined names use the existing identifier/constant IDs,
        // not an invented semantic symbol-table resolution.
        let at = source.find(n).unwrap();
        assert!(matches!(lex(source).styles[at], style::IDENTIFIER|style::CONSTANT));
    }
    styled(source,"$value",style::VARIABLE);
    styled(source,"#[",style::OPERATOR);
}
#[test]
fn phpdoc_and_annotation_strings_are_documentation_not_code() {
    let s = r#"<?php
/**
 * @template T of \App\Model
 * @param array<string, T> $items
 * @return ?Collection<T>
 * @ORM\Column(type="string")
 * @custom-tag {$notCode} ?>
 */
/**/ $after = 1;
"#;
    for n in ["@template","@param","@return","@ORM\\Column","$items","{$notCode}","?>"] { styled(s,n,style::DOC); }
    styled(s,"/**/",style::COMMENT);
    styled(s,"$after",style::VARIABLE);
}
#[test]
fn all_number_spellings_and_operators() {
    for n in ["0","00","0777","0_777","0o755","0O7_55","0b1010","0B1_0","0xABcd","0XCA_FE","123_456",".5",".5e+2","1.","1.e2","2E-3","1_2.3_4e+5_6"] {
        styled(&format!("<?php $n = {n};"),n,style::NUMBER);
    }
    for op in ["?->","??=","===","!==","<=>","**=","<<=",">>=","...","::","=>","&","|","\\"] {
        styled(&format!("<?php {op}"),op,style::OPERATOR);
    }
}
#[test]
fn nested_and_simple_interpolation_escapes_and_backticks() {
    let s = r#"<?php $s = "{$user['nested'][0]} ${name} {$x->{call($y, ['}'])}} $simple";
$literal = '\'$notInterpolated';
$command = `echo $arg`;
"#;
    for n in ["{$user['nested'][0]}","${name}","{$x->{call($y, ['}'])}}","$simple","$arg"] { styled(s,n,style::INTERPOLATED); }
    styled(s,"$notInterpolated",style::SINGLE);
    styled(s,"echo ",style::CHARACTER);
}
#[test]
fn heredoc_nowdoc_labels_suffixes_crlf_and_prefixes() {
    for (head,id) in [("TXT",style::HEREDOC),("\"TXT\"",style::HEREDOC),("'TXT'",style::NOWDOC)] {
        let s = format!("<?php $text = <<<{head}\r\n  ?> $name\r\n  TXT_MORE\r\n  TXT, $after; ?>tail");
        styled(&s,"?> ",id);
        styled(&s,"TXT_MORE",id);
        styled(&s,"$after",style::VARIABLE);
        styled(&s,"tail",style::HTML);
        styled(&s,"$name",if id==style::NOWDOC {id} else {style::INTERPOLATED});
        for (end,_) in s.char_indices().chain(std::iter::once((s.len(),'\0'))) {
            assert_eq!(lex(&s[..end]).styles.len(),end);
        }
    }
}

#[test]
fn simple_interpolation_offsets_properties_and_escaped_delimiters() {
    let source = r#"<?php "hello $user->name $list[0] $list[-1] $map[key] $map[$key] \"still string";"#;
    for needle in ["$user->name", "$list[0]", "$list[-1]", "$map[key]", "$map[$key]"] {
        styled(source,needle,style::INTERPOLATED);
    }
    styled(source,"still string",style::DOUBLE);
    for source in ["<?php \"unterminated", "<?php /*", "<?php <<<TXT\n{$x", "<?php #[A(", "<?php 'é\\"] {
        assert_eq!(lex(source).styles.len(),source.len());
    }
}
