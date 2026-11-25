//! A conservative conventions overlay on existing PHP/Blade byte styles.
//! No filesystem, project detection, parsing or PHP execution belongs here.
use crate::style;
use std::ops::Range;

pub const FACADE: u8 = 40;
pub const METHOD: u8 = 41;
pub const HELPER: u8 = 42;
pub const CONVENTION: u8 = 43;
pub const CONTEXT_STRING: u8 = 44;

pub struct Vocabulary<'a> {
    pub facades: &'a str,
    pub route_methods: &'a str,
    pub artisan: &'a str,
    pub helpers: &'a str,
    pub eloquent: &'a str,
}
#[derive(Clone)]
struct Token { range: Range<usize>, kind: u8 }
fn member(s: &str, list: &str) -> bool { list.split_ascii_whitespace().any(|word|word==s) }
fn code_identifier(s:u8) -> bool { matches!(s,style::IDENTIFIER|style::CONSTANT|style::FUNCTION|style::TYPE|style::KEYWORD) }

/// The caller owns project detection and supplies existing framework word lists.
/// Reject a mismatched byte buffer rather than creating partial highlighting.
pub fn overlay(source:&str, styles:&mut [u8], vocab:&Vocabulary<'_>) -> bool {
    if source.len()!=styles.len() {return false;}
    let mut tokens=Vec::new();
    let mut start=0;
    while start<styles.len() {
        let kind=styles[start];let mut end=start+1;
        while end<styles.len() && styles[end]==kind {end+=1;}
        if kind==style::OPERATOR {
            // Keep multi-byte member operators but separate punctuation.
            let mut at=start;
            while at<end {
                let width=if source[at..end].starts_with("?->") {3} else if source[at..end].starts_with("->") || source[at..end].starts_with("::") {2} else {source[at..].chars().next().unwrap().len_utf8()};
                tokens.push(Token{range:at..at+width,kind});at+=width;
            }
        } else if !matches!(kind,style::DEFAULT|style::COMMENT|style::LINE_COMMENT|style::DOC) {
            tokens.push(Token{range:start..end,kind});
        }
        start=end;
    }
    let text=|index:usize| -> &str { &source[tokens[index].range.clone()] };
    let conventions="Model Authenticatable ServiceProvider Controller FormRequest Middleware Job ShouldQueue Dispatchable InteractsWithQueue Queueable SerializesModels Event Listener Notification Notifiable Policy Gate HasFactory SoftDeletes HasMany HasOne BelongsTo BelongsToMany MorphTo MorphMany MorphOne HasManyThrough HasOneThrough";
    let methods="dispatch dispatchSync dispatchAfterResponse listen subscribe fire notify notifyNow via toMail toDatabase toArray authorize authorizeForUser allows denies check before after forUser policy define bind singleton scoped instance make makeWith resolve boot register provides when needs give commands loadRoutesFrom loadViewsFrom publishes hasMany hasOne belongsTo belongsToMany morphTo morphMany morphOne hasManyThrough hasOneThrough";
    for (index,token) in tokens.iter().enumerate() {
        if !code_identifier(token.kind) {continue;}
        let word=text(index);
        let prev=index.checked_sub(1).map(text).unwrap_or("");
        let next=if index+1<tokens.len(){text(index+1)}else{""};
        let member_call=matches!(prev,"::"|"->"|"?->") && next=="(";
        let scope=if member(word,vocab.facades) && (next=="::" || prev=="\\") {Some(FACADE)}
        else if member_call && (member(word,vocab.route_methods)||member(word,vocab.eloquent)||member(word,methods)) {Some(METHOD)}
        else if next=="(" && !matches!(prev,"::"|"->"|"?->"|"function"|"new"|"\\") && member(word,vocab.helpers) {Some(HELPER)}
        else if member(word,conventions) && (matches!(prev,"extends"|"implements"|"use"|"new"|"\\"|"|"|"?"|":"|",") || next=="::") {Some(CONVENTION)}
        else {None};
        if let Some(scope)=scope {styles[token.range.clone()].fill(scope);}
    }
    // Literal command/middleware values only in a nearby matching call context.
    // A stack bounds nested calls and arrays without trying to parse PHP.
    let mut calls:Vec<Option<&str>>=Vec::new();
    for(index,token)in tokens.iter().enumerate() {
        let word=text(index);
        if token.kind==style::OPERATOR && word=="(" {
            let name=index.checked_sub(1).filter(|i|code_identifier(tokens[*i].kind)).map(text);
            calls.push(name);
        } else if token.kind==style::OPERATOR && word==")" {calls.pop();}
        else if matches!(token.kind,style::OPEN|style::CLOSE|style::HTML|style::HTML_TAG) {calls.clear();}
        else if matches!(token.kind,style::SINGLE|style::DOUBLE) && word.len()>=2 && (word.starts_with('\'')&&word.ends_with('\'') || word.starts_with('"')&&word.ends_with('"')) {
            let value=&word[1..word.len()-1];
            let call=calls.last().and_then(|v|*v).unwrap_or("");
            let middleware=matches!(call,"middleware"|"withoutMiddleware") && member(value.split(':').next().unwrap_or(""),"auth guest verified throttle can signed bindings cache.headers password.confirm precognitive auth.basic auth.session");
            let command=matches!(call,"call"|"queue") && member(value,vocab.artisan) && index>=3 && tokens[..index].iter().rev().take(5).any(|t|&source[t.range.clone()]=="Artisan");
            if middleware||command {styles[token.range.clone()].fill(CONTEXT_STRING);}
        }
    }
    true
}
