/** @file LexTwig.cxx
 * Twig template regions. HTML/script/style composition is performed by the
 * snapshot adapter with byte-preserving masks, not by copying PHP scanners.
 * Independently implemented against Twig 3 delimiter behaviour.
 */
#include <algorithm>
#include <cassert>
#include <cctype>
#include <cstring>
#include <string>
#include <map>
#include <functional>
#include <string_view>
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
    {0,"text","default","Host HTML"},
    {1,"delimiter","operator","Twig region delimiters"},
    {2,"comment","comment","Twig comment"},
    {3,"string","literal string","Twig string"},
    {4,"number","literal numeric","Number"},
    {5,"keyword","keyword","Tag or operator"},
    {6,"function","function","Builtin function/filter/test"},
    {7,"identifier","identifier","Twig variable"},
    {8,"operator","operator","Expression punctuation"},
    {9,"verbatim","literal string","Uninterpreted template text"},
};
const char * const words[] = {"Tags and operators", "Filters", "Functions", "Tests", nullptr};
bool ident(unsigned char c) { return c >= 128 || std::isalnum(c) || c == '_'; }
class LexerTwig final : public DefaultLexer {
    WordList keywords[4];
public:
    LexerTwig(): DefaultLexer("twig",146,classes,std::size(classes)) {}
    const char * SCI_METHOD DescribeWordListSets() override { return "Tags and operators\nFilters\nFunctions\nTests"; }
    Sci_Position SCI_METHOD WordListSet(int n, const char *s) override { return n >= 0 && n < 4 && keywords[n].Set(s) ? 0 : -1; }
    void SCI_METHOD Lex(Sci_PositionU start, Sci_Position length, int, IDocument *doc) override {
        // The editor requests full snapshots. Rebuild region state on partial
        // calls too, then publish only the requested byte range.
        std::string text(doc->Length(),'\0');
        doc->GetCharRange(text.data(),0,doc->Length());
        std::string styles(text.size(),0);
        auto paint=[&](size_t a,size_t b,int style) { std::fill(styles.begin()+a, styles.begin()+std::min(b,text.size()), static_cast<char>(style)); };
        size_t p=0;
        while(p<text.size()) {
            if (text.compare(p,2,"{#")==0) {
                size_t end=text.find("#}",p+2); end=end==std::string::npos?text.size():end+2;
                paint(p,end,2); p=end; continue;
            }
            bool tag=text.compare(p,2,"{%") == 0;
            if (!tag && text.compare(p,2,"{{") != 0) { ++p; continue; }
            size_t begin=p; paint(p,p+2,1); p+=2;
            if(p<text.size() && (text[p]=='-' || text[p]=='~')) { paint(p,p+1,1); ++p; }
            size_t first=p; while(first<text.size() && std::isspace(static_cast<unsigned char>(text[first]))) ++first;
            bool verbatim=tag && text.compare(first,8,"verbatim")==0 && (first+8==text.size() || !ident(text[first+8]));
            int nesting=0;
            while(p<text.size()) {
                const char *close=tag?"%}":"}}";
                if(nesting==0 && (text.compare(p,2,close)==0 || ((text[p]=='-' || text[p]=='~') && text.compare(p+1,2,close)==0))) {
                    size_t end=p+(text[p]=='-' || text[p]=='~'?3:2); paint(p,end,1); p=end; break;
                }
                unsigned char c=text[p];
                if(c=='\'' || c=='"') {
                    size_t a=p++; int quote=c;
                    while(p<text.size()) { if(text[p]=='\\') p=std::min(p+2,text.size()); else if(text[p++]==quote) break; }
                    paint(a,p,3);
                } else if(std::isdigit(c)) {
                    size_t a=p++; while(p<text.size() && (std::isdigit(static_cast<unsigned char>(text[p])) || text[p]=='.' || text[p]=='_')) ++p;
                    paint(a,p,4);
                } else if(ident(c)) {
                    size_t a=p++; while(p<text.size() && ident(text[p])) ++p;
                    std::string word=text.substr(a,p-a); int style=7;
                    if(keywords[0].InList(word.c_str())) style=5;
                    else for(int i=1;i<4;++i) if(keywords[i].InList(word.c_str())) style=6;
                    paint(a,p,style);
                } else {
                    if(c=='(' || c=='[' || c=='{') ++nesting;
                    if(c==')' || c==']' || c=='}') nesting=std::max(0,nesting-1);
                    paint(p,p+1,std::isspace(c)?7:8); ++p;
                }
            }
            if(verbatim && p<text.size()) {
                // Find a genuine endverbatim tag, allowing whitespace controls.
                size_t raw=p, search=p, end=std::string::npos;
                while((search=text.find("{%",search))!=std::string::npos) {
                    size_t q=search+2; if(q<text.size() && (text[q]=='-' || text[q]=='~')) ++q;
                    while(q<text.size() && std::isspace(static_cast<unsigned char>(text[q]))) ++q;
                    if(text.compare(q,11,"endverbatim")==0 && (q+11==text.size() || !ident(text[q+11]))) { end=search; break; }
                    search+=2;
                }
                if(end==std::string::npos) { paint(raw,text.size(),9); p=text.size(); }
                else { paint(raw,end,9); p=end; }
            }
            (void)begin;
        }
        size_t count=std::min(static_cast<size_t>(length), text.size()-std::min(static_cast<size_t>(start),text.size()));
        doc->StartStyling(start); doc->SetStyles(count,styles.data()+start);
    }
    static ILexer5 *Factory() { return new LexerTwig(); }
};
}
extern const LexerModule lmTwig(146,LexerTwig::Factory,"twig",words);
