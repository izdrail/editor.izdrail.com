#include "../shim.cpp"
#include <cassert>
int main(){
 Document d("a\r\nb\rc\n",7);
 assert(d.LineFromPosition(0)==0 && d.LineFromPosition(3)==1 && d.LineFromPosition(5)==2 && d.LineFromPosition(7)==3);
 assert(d.LineEnd(0)==1 && d.LineEnd(1)==4 && d.LineEnd(2)==6);
 Document u("a\xc3\xa9\xf0\x9f\x8c\x8d",7); Sci_Position w=0;
 assert(u.GetCharacterAndWidth(1,&w)==0xe9 && w==2);
 assert(u.GetCharacterAndWidth(3,&w)==0x1f30d && w==4);
 assert(u.GetRelativePosition(0,3)==7 && u.GetRelativePosition(7,-2)==1);
 Document broken("\xf0\x9f",2); assert(broken.GetCharacterAndWidth(0,&w)==0xf0 && w==1);
 void *s=lx_new("cpp","",0); assert(s && lx_lex(s,0,0,0,1)==0);lx_free(s);
 assert(!lx_new("no-such-lexer","",0));
}
