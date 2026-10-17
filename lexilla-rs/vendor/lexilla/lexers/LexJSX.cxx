/** @file LexJSX.cxx
 * JSX/TSX markup regions inside JavaScript and TypeScript code.
 *
 * The surrounding code is styled by the snapshot adapter, not by this lexer:
 * bytes left at style 0 are masked out and lexed by the cpp lexer's
 * JavaScript mode (see web_templates::jsx in lapce-core). This lexer only
 * recognises markup: tag names, attribute names, attribute strings and the
 * delimiter bytes `<`, `>`, `</`, `/>`, `=` plus the `{` `}` expression
 * braces. Children text and expression interiors stay style 0 so the code
 * pass styles them.
 *
 * Tag detection is heuristic, as in every state-machine JSX highlighter
 * (babel tracks parser state via canStartJSXElement; we approximate it):
 * `<` opens markup only in expression position, never after a value-
 * producing token (identifier, number, string, `)`, `]`), and a tag header
 * commits only once it completes with `>` or `/>`. Comparisons (`a < b`),
 * generic calls (`foo<T>()`) and generic arrows (`<T,>(x: T) => x`,
 * `<T extends U>(...)`) therefore stay code. Nesting beyond maxDepth is
 * left to the code pass so minified bundles cannot exhaust the stack.
 */
#include <algorithm>
#include <cassert>
#include <cctype>
#include <string>
#include <tuple>
#include <vector>
#include "ILexer.h"
#include "Scintilla.h"
#include "SciLexer.h"
#include "LexerModule.h"
#include "LexAccessor.h"
#include "DefaultLexer.h"
#include "WordList.h"
using namespace Scintilla;
using namespace Lexilla;
namespace {
const LexicalClass classes[] = {
    {0,"code","default","JavaScript/TypeScript code (styled by the adapter's cpp pass)"},
    {1,"tag","tag","Element and fragment tag names"},
    {2,"attribute","attribute","Attribute names"},
    {3,"string","literal string","Attribute string values"},
};
bool identStart(unsigned char c){ return c>=128 || std::isalpha(c) || c=='_' || c=='$'; }
bool identChar(unsigned char c){ return c>=128 || std::isalnum(c) || c=='_' || c=='$'; }
bool tagChar(unsigned char c){ return identChar(c) || c=='-' || c=='.' || c==':'; }
// Keywords after which an expression - and therefore markup - may follow.
bool exprKeyword(const std::string &w){
    static const char *const words[]={"return","throw","case","delete","typeof","void",
        "yield","await","else","do","in","of","instanceof","new","extends","as","satisfies"};
    for(const char *k:words) if(w==k) return true;
    return false;
}
// A first attribute that is a TypeScript type keyword means `<` opened a
// generic parameter list (`<T extends U>`), not markup.
bool tsTypeKeyword(const std::string &w){
    return w=="extends" || w=="keyof" || w=="infer" || w=="readonly";
}
constexpr size_t maxDepth=200;
struct Scanner {
    const std::string &t;
    std::string &st;
    void paint(size_t a,size_t b,int s){
        std::fill(st.begin()+std::min(a,st.size()), st.begin()+std::min(b,st.size()), static_cast<char>(s));
    }
    size_t stringEnd(size_t p){
        char q=t[p]; size_t e=p+1;
        while(e<t.size() && t[e]!=q){ if(t[e]=='\\') ++e; if(e<t.size()) ++e; }
        return e<t.size()?e+1:e;
    }
    size_t regexEnd(size_t p){
        size_t e=p+1; bool cls=false;
        while(e<t.size() && t[e]!='\n'){
            char c=t[e];
            if(c=='\\'){ e+=2; continue; }
            if(c=='[') cls=true;
            else if(c==']') cls=false;
            else if(c=='/' && !cls){ ++e; while(e<t.size() && identChar(t[e])) ++e; return e; }
            ++e;
        }
        return e;
    }
    size_t templateEnd(size_t p, size_t depth){
        size_t e=p+1;
        while(e<t.size()){
            if(t[e]=='\\'){ e+=2; continue; }
            if(t[e]=='`') return e+1;
            if(t[e]=='$' && e+1<t.size() && t[e+1]=='{' && depth<=maxDepth){
                e=js(e+2,true,depth+1);
                if(e<t.size()) ++e; // consume the closing `}`
                continue;
            }
            ++e;
        }
        return e;
    }
    // JavaScript context. Paints nested markup only; comments, strings and
    // code stay style 0 for the adapter's cpp pass. With braceStop, returns
    // positioned on the `}` that closes the enclosing `{`.
    size_t js(size_t p, bool braceStop, size_t depth){
        bool expr=true; // next token may start an expression
        int braces=0;
        while(p<t.size()){
            unsigned char c=t[p];
            if(std::isspace(c)){ ++p; continue; }
            if(c=='/' && p+1<t.size() && t[p+1]=='/'){ size_t e=t.find('\n',p+2); p=(e==std::string::npos)?t.size():e; continue; }
            if(c=='/' && p+1<t.size() && t[p+1]=='*'){ size_t e=t.find("*/",p+2); p=(e==std::string::npos)?t.size():e+2; continue; }
            if(c=='\'' || c=='"'){ p=stringEnd(p); expr=false; continue; }
            if(c=='`'){ p=templateEnd(p,depth); expr=false; continue; }
            if(c=='/'){ if(expr){ p=regexEnd(p); expr=false; } else { ++p; expr=true; } continue; }
            if(identStart(c)){
                size_t e=p+1; while(e<t.size() && identChar(t[e])) ++e;
                expr=exprKeyword(t.substr(p,e-p)); p=e; continue;
            }
            if(std::isdigit(c)){ size_t e=p+1; while(e<t.size() && (identChar(t[e])||t[e]=='.')) ++e; p=e; expr=false; continue; }
            if(c=='<' && expr && p+1<t.size() && (identStart(t[p+1]) || t[p+1]=='>')){
                size_t end=markup(p,depth+1);
                if(end>p){ p=end; expr=false; continue; }
            }
            if(c=='}'){ if(braceStop && braces==0) return p; if(braces>0) --braces; ++p; expr=true; continue; }
            if(c=='{'){ ++braces; ++p; expr=true; continue; }
            if(c==')' || c==']' || c=='.'){ ++p; expr=false; continue; }
            ++p; expr=true; // remaining operators and punctuation
        }
        return p;
    }
    // One element or fragment whose `<` is at p. Returns the position after
    // its closing tag (or self-close), or p unchanged when the bytes after
    // `<` are not markup. Header spans are speculative: they are painted
    // only once the header completes, so rejected headers leave no styles.
    size_t markup(size_t p, size_t depth){
        if(depth>maxDepth || p+1>=t.size() || t[p]!='<') return p;
        std::vector<std::tuple<size_t,size_t,int>> pending;
        size_t i=p+1;
        if(t[i]=='>'){ ++i; } // fragment open `<>`
        else {
            size_t s=i;
            if(!identStart(t[i])) return p;
            while(i<t.size() && tagChar(t[i])) ++i;
            // After the name JSX requires whitespace, `/` or `>`; `(` or `,`
            // here means a TypeScript generic list, not markup.
            if(i<t.size() && !(std::isspace(static_cast<unsigned char>(t[i])) || t[i]=='/' || t[i]=='>')) return p;
            pending.emplace_back(s,i,1);
            for(;;){
                while(i<t.size() && std::isspace(static_cast<unsigned char>(t[i]))) ++i;
                if(i>=t.size()){
                    // Tag header cut off by EOF while typing: the position was
                    // already validated as an expression start, so commit.
                    for(const auto &sp:pending) paint(std::get<0>(sp),std::get<1>(sp),std::get<2>(sp));
                    return i;
                }
                if(t[i]=='>'){ ++i; break; }
                if(t[i]=='/' && i+1<t.size() && t[i+1]=='>'){
                    i+=2;
                    for(const auto &sp:pending) paint(std::get<0>(sp),std::get<1>(sp),std::get<2>(sp));
                    return i;
                }
                if(t[i]=='{'){ // spread or expression attribute
                    size_t e=js(i+1,true,depth+1);
                    i=(e<t.size())?e+1:e;
                    continue;
                }
                if(!identStart(t[i])) return p;
                size_t as=i;
                while(i<t.size() && (identChar(t[i]) || t[i]=='-' || t[i]==':')) ++i;
                if(tsTypeKeyword(t.substr(as,i-as))) return p;
                pending.emplace_back(as,i,2);
                while(i<t.size() && std::isspace(static_cast<unsigned char>(t[i]))) ++i;
                if(i<t.size() && t[i]=='='){
                    ++i;
                    while(i<t.size() && std::isspace(static_cast<unsigned char>(t[i]))) ++i;
                    if(i<t.size() && (t[i]=='"' || t[i]=='\'')){
                        size_t e=stringEnd(i); pending.emplace_back(i,e,3); i=e;
                    } else if(i<t.size() && t[i]=='{'){
                        size_t e=js(i+1,true,depth+1);
                        i=(e<t.size())?e+1:e;
                    } else return p;
                }
            }
        }
        for(const auto &sp:pending) paint(std::get<0>(sp),std::get<1>(sp),std::get<2>(sp));
        // Children: text stays style 0, `{expr}` and nested elements are handled.
        while(i<t.size()){
            if(t[i]=='{'){
                size_t e=js(i+1,true,depth+1);
                i=(e<t.size())?e+1:e;
                continue;
            }
            if(t[i]=='<'){
                if(i+1<t.size() && t[i+1]=='/'){
                    size_t cs=i; i+=2;
                    size_t ns=i;
                    while(i<t.size() && tagChar(t[i])) ++i;
                    size_t ne=i;
                    while(i<t.size() && std::isspace(static_cast<unsigned char>(t[i]))) ++i;
                    if(i<t.size() && t[i]=='>') ++i;
                    if(ne>ns) paint(ns,ne,1);
                    (void)cs;
                    return i; // first closing tag ends this element (tolerant of mismatches)
                }
                size_t e=markup(i,depth+1);
                if(e>i){ i=e; continue; }
            }
            ++i;
        }
        return i; // unclosed while typing: keep the spans painted so far
    }
};
class LexerJSX final : public DefaultLexer {
public:
    // 147 follows the fork's own ids (php 143, laravel 144, blade 145, twig 146).
    LexerJSX(): DefaultLexer("jsx",147,classes,std::size(classes)) {}
    void SCI_METHOD Lex(Sci_PositionU start, Sci_Position length, int, IDocument *doc) override {
        // The editor requests full snapshots. Rebuild region state on partial
        // calls too, then publish only the requested byte range.
        std::string text(doc->Length(),'\0');
        doc->GetCharRange(text.data(),0,doc->Length());
        std::string styles(text.size(),0);
        Scanner scan{text,styles};
        scan.js(0,false,0);
        size_t count=std::min(static_cast<size_t>(length), text.size()-std::min(static_cast<size_t>(start),text.size()));
        doc->StartStyling(start); doc->SetStyles(count,styles.data()+start);
    }
    static ILexer5 *Factory(){ return new LexerJSX(); }
};
}
extern const LexerModule lmJsx(147,LexerJSX::Factory,"jsx",nullptr);
