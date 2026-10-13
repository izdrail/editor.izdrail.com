// UTF-8-only snapshot adapter. Positions and styles are BYTES, not characters.
#include <algorithm>
#include <array>
#include <cstring>
#include <memory>
#include <mutex>
#include <string>
#include <vector>
#include "ILexer.h"
#include "Lexilla.h"
using namespace Scintilla;
class Document final : public IDocument {
public:
    std::string text;
    std::vector<unsigned char> styles;
    std::vector<Sci_Position> starts;
    std::vector<int> states, levels;
    Sci_Position cursor = 0;
    int error = 0;
    mutable std::array<unsigned long long,24> calls{};
    Document(const char *p, size_t n):text(p,n),styles(n,0),starts{0} {
        for(size_t i=0;i<n;i++) {
            if(text[i]=='\r') { if(i+1<n && text[i+1]=='\n') ++i; starts.push_back(i+1); }
            else if(text[i]=='\n') starts.push_back(i+1);
        }
        states.resize(starts.size()+1); levels.resize(starts.size()+1,0x400);
    }
    int SCI_METHOD Version() const override { calls[0]++; return dvRelease4; }
    void SCI_METHOD SetErrorStatus(int s) override { calls[1]++; error=s; }
    Sci_Position SCI_METHOD Length() const override { calls[2]++; return text.size(); }
    void SCI_METHOD GetCharRange(char *b,Sci_Position p,Sci_Position n) const override {
        calls[3]++; if(p<0 || n<0 || size_t(p+n)>text.size()) { std::memset(b,0,std::max<Sci_Position>(0,n)); return; }
        std::memcpy(b,text.data()+p,n);
    }
    char SCI_METHOD StyleAt(Sci_Position p) const override { calls[4]++; return p>=0 && size_t(p)<styles.size()?styles[p]:0; }
    Sci_Position SCI_METHOD LineFromPosition(Sci_Position p) const override {
        calls[5]++; p=std::clamp<Sci_Position>(p,0,text.size());
        return std::upper_bound(starts.begin(),starts.end(),p)-starts.begin()-1;
    }
    Sci_Position SCI_METHOD LineStart(Sci_Position l) const override {
        calls[6]++; return l<0?0:size_t(l)<starts.size()?starts[l]:text.size();
    }
    int SCI_METHOD GetLevel(Sci_Position l) const override { calls[7]++; return l>=0 && size_t(l)<levels.size()?levels[l]:0x400; }
    int SCI_METHOD SetLevel(Sci_Position l,int v) override { calls[8]++; if(l<0 || size_t(l)>=levels.size())return 0x400; int old=levels[l]; levels[l]=v; return old; }
    int SCI_METHOD GetLineState(Sci_Position l) const override { calls[9]++; return l>=0 && size_t(l)<states.size()?states[l]:0; }
    int SCI_METHOD SetLineState(Sci_Position l,int v) override { calls[10]++; if(l<0 || size_t(l)>=states.size()) return 0; int old=states[l]; states[l]=v; return old; }
    void SCI_METHOD StartStyling(Sci_Position p) override { calls[11]++; cursor=p; }
    bool SCI_METHOD SetStyleFor(Sci_Position n,char v) override {
        calls[12]++; if(cursor<0 || n<0 || size_t(cursor+n)>styles.size()) { error=1; return false; }
        std::fill_n(styles.data()+cursor,n,static_cast<unsigned char>(v)); cursor+=n; return true;
    }
    bool SCI_METHOD SetStyles(Sci_Position n,const char *s) override {
        calls[13]++; if(cursor<0 || n<0 || size_t(cursor+n)>styles.size()) { error=1; return false; }
        std::memcpy(styles.data()+cursor,s,n); cursor+=n; return true;
    }
    void SCI_METHOD DecorationSetCurrentIndicator(int) override { calls[14]++; /* omitted: no indicator output */ }
    void SCI_METHOD DecorationFillRange(Sci_Position,int,Sci_Position) override { calls[15]++; /* omitted */ }
    void SCI_METHOD ChangeLexerState(Sci_Position,Sci_Position) override { calls[16]++; /* full snapshot has no downstream invalidation */ }
    int SCI_METHOD CodePage() const override { calls[17]++; return 65001; }
    bool SCI_METHOD IsDBCSLeadByte(char) const override { calls[18]++; return false; /* UTF-8 only, not a DBCS encoding */ }
    const char * SCI_METHOD BufferPointer() override { calls[19]++; return text.data(); }
    int SCI_METHOD GetLineIndentation(Sci_Position l) override {
        calls[20]++; int col=0;
        for(Sci_Position p=LineStart(l);size_t(p)<text.size();++p) {
            if(text[p]==' ') ++col; else if(text[p]=='\t')col=(col/8+1)*8; else break;
        } return col;
    }
    Sci_Position SCI_METHOD LineEnd(Sci_Position l) const override {
        calls[21]++; Sci_Position p=LineStart(l+1),s=LineStart(l);
        if(p>s && text[p-1]=='\n')--p; if(p>s && text[p-1]=='\r')--p; return p;
    }
    Sci_Position SCI_METHOD GetRelativePosition(Sci_Position p,Sci_Position offset) const override {
        calls[22]++; if(p<0 || size_t(p)>text.size())return -1;
        while(offset<0) { if(p==0)return -1; --p; while(p>0 && (static_cast<unsigned char>(text[p])&0xc0)==0x80)--p; ++offset; }
        while(offset>0) { if(size_t(p)>=text.size())return -1; Sci_Position w; GetCharacterAndWidth(p,&w); p+=w; --offset; } return p;
    }
    int SCI_METHOD GetCharacterAndWidth(Sci_Position p,Sci_Position *width) const override {
        calls[23]++; if(width)*width=1; if(p<0 || size_t(p)>=text.size())return 0;
        unsigned char c=text[p]; int n=c<0x80?1:c>=0xc2 && c<=0xdf?2:c>=0xe0 && c<=0xef?3:c>=0xf0 && c<=0xf4?4:1;
        if(size_t(p+n)>text.size())return c;
        int value=c & (n==1?0xff:n==2?0x1f:n==3?0xf:7);
        for(int i=1;i<n;i++) { unsigned char t=text[p+i]; if((t&0xc0)!=0x80)return c; value=(value<<6)|(t&0x3f); }
        if((n==2 && value<0x80)||(n==3 && value<0x800)||(n==4 && value<0x10000)||(value>=0xd800 && value<=0xdfff)||value>0x10ffff)return c;
        if(width)*width=n; return value;
    }
};
struct Session { std::unique_ptr<ILexer5,void(*)(ILexer5*)> lexer; Document doc;
    Session(ILexer5 *l,const char *t,size_t n):lexer(l,[](ILexer5 *p){p->Release();}),doc(t,n){} };
// Lexilla's AddEachLexer lazily mutates a global catalogue without a lock.
// Rayon workers may first enter it concurrently. Initialize exactly once here
// before any public catalogue function; subsequent reads are immutable.
static void initialize_catalogue() {
    static std::once_flag once;
    std::call_once(once, [] { (void)GetLexerCount(); });
}
extern "C" {
void *lx_new(const char *name,const char *text,size_t n) noexcept {
    try { initialize_catalogue(); auto *l=CreateLexer(name); if(!l)return nullptr; return new Session(l,text,n); }catch(...){return nullptr;}
}
void lx_free(void *p) noexcept {delete static_cast<Session*>(p);}
int lx_words(void *p,int i,const char *words) noexcept {try {return static_cast<Session*>(p)->lexer->WordListSet(i,words);}catch(...){return -2;}}
int lx_property(void *p,const char *key,const char *val) noexcept {try{return static_cast<Session*>(p)->lexer->PropertySet(key,val);}catch(...){return -2;}}
int lx_lex(void *p,size_t start,size_t length,int init,int fold) noexcept {
    auto &s=*static_cast<Session*>(p); if(start>s.doc.text.size()||length>s.doc.text.size()-start)return -3;
    try {s.lexer->Lex(start,length,init,&s.doc);if(fold)s.lexer->Fold(start,length,init,&s.doc);return s.doc.error;}catch(...){return -2;}
}
const unsigned char *lx_styles(void *p) noexcept {return static_cast<Session*>(p)->doc.styles.data();}
const unsigned long long *lx_calls(void *p) noexcept {return static_cast<Session*>(p)->doc.calls.data();}
int lx_named(void *p) noexcept {return static_cast<Session*>(p)->lexer->NamedStyles();}
const char *lx_name(void *p,int i) noexcept {return static_cast<Session*>(p)->lexer->NameOfStyle(i);}
const char *lx_tags(void *p,int i) noexcept {return static_cast<Session*>(p)->lexer->TagsOfStyle(i);}
int lx_count() noexcept {initialize_catalogue(); return GetLexerCount();}
void lx_lexer_name(unsigned int i,char *out,int n) noexcept {initialize_catalogue(); GetLexerName(i,out,n);}
}
