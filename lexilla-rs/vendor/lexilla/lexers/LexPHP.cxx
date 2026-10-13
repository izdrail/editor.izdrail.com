/** @file LexPHP.cxx
 ** Lexer for PHP.
 **
 ** Modeled on LexRust.cxx (which was Copyright (c) 2013 by SiegeLord).
 **/

#include <cstdlib>
#include <cassert>
#include <cstring>
#include <cctype>
#include <cstdio>
#include <cstdarg>

#include <string>
#include <string_view>
#include <map>
#include <functional>

#include "ILexer.h"
#include "Scintilla.h"
#include "SciLexer.h"

#include "PropSetSimple.h"
#include "WordList.h"
#include "LexAccessor.h"
#include "Accessor.h"
#include "StyleContext.h"
#include "CharacterSet.h"
#include "LexerModule.h"
#include "OptionSet.h"
#include "DefaultLexer.h"

using namespace Scintilla;
using namespace Lexilla;

/* -------------------------------------------------------------------------
 *  Lexer id / styles.
 *
 *  Standard SciLexer.h does not currently ship a dedicated SCLEX_PHP with
 *  its own SCE_PHP_* range; PHP is normally handled inside LexHTML with the
 *  SCE_HPHP_* styles.  To keep this file self-contained (and to make it a
 *  real standalone lexer, matching your LexRust.cxx), we provide our own.
 *  Adjust the numbers if they clash with anything in your fork.
 * ------------------------------------------------------------------------- */
#ifndef SCLEX_PHP
#define SCLEX_PHP 143
#endif

#ifndef SCE_PHP_DEFAULT
#define SCE_PHP_DEFAULT         0
#define SCE_PHP_COMMENT         1
#define SCE_PHP_COMMENTLINE     2
#define SCE_PHP_DOCCOMMENT      3
#define SCE_PHP_VARIABLE        4
#define SCE_PHP_STRING          5
#define SCE_PHP_STRING_VARIABLE 6
#define SCE_PHP_STRING_DQ       7
#define SCE_PHP_HEREDOC         8
#define SCE_PHP_NOWDOC          9
#define SCE_PHP_CHARACTER       10
#define SCE_PHP_OPERATOR        11
#define SCE_PHP_NUMBER          12
#define SCE_PHP_WORD            13
#define SCE_PHP_WORD2           14
#define SCE_PHP_WORD3           15
#define SCE_PHP_WORD4           16
#define SCE_PHP_IDENTIFIER      17
#define SCE_PHP_LEXERROR        18
#define SCE_PHP_OPEN_TAG        19
#define SCE_PHP_CLOSE_TAG       20
#define SCE_PHP_INLINE_HTML     21
#endif

namespace {

constexpr int NUM_PHP_KEYWORD_LISTS = 4;
constexpr int MAX_PHP_IDENT_CHARS   = 1023;

/* ----------------------------- Options ---------------------------------- */

struct OptionsPHP {
	bool fold;
	bool foldSyntaxBased;
	bool foldComment;
	bool foldCommentMultiline;
	bool foldCompact;
	bool foldAtElse;
	OptionsPHP() {
		fold = false;
		foldSyntaxBased = true;
		foldComment = false;
		foldCommentMultiline = true;
		foldCompact = true;
		foldAtElse = false;
	}
};

static const char * const phpWordLists[NUM_PHP_KEYWORD_LISTS + 1] = {
	"Keywords",
	"Builtin functions",
	"Builtin types",
	"Other keywords",
	nullptr,
};

struct OptionSetPHP : public OptionSet<OptionsPHP> {
	OptionSetPHP() {
		DefineProperty("fold", &OptionsPHP::fold);
		DefineProperty("fold.comment", &OptionsPHP::foldComment);
		DefineProperty("fold.compact", &OptionsPHP::foldCompact);
		DefineProperty("fold.at.else", &OptionsPHP::foldAtElse);

		DefineProperty("fold.php.syntax.based", &OptionsPHP::foldSyntaxBased,
			"Set this property to 0 to disable syntax based folding.");

		DefineProperty("fold.php.comment.multiline", &OptionsPHP::foldCommentMultiline,
			"Set this property to 0 to disable folding multi-line comments when fold.comment=1.");

		DefineWordListSets(phpWordLists);
	}
};

const LexicalClass lexicalClasses[] = {
	// Lexer php SCLEX_PHP SCE_PHP_:
	0,  "SCE_PHP_DEFAULT",         "default",               "White space / inline HTML",
	1,  "SCE_PHP_COMMENT",         "comment",               "Comment",
	2,  "SCE_PHP_COMMENTLINE",     "comment line",          "Line comment",
	3,  "SCE_PHP_DOCCOMMENT",      "comment documentation", "Doc comment",
	4,  "SCE_PHP_VARIABLE",        "identifier variable",   "Variable",
	5,  "SCE_PHP_STRING",          "literal string",        "Single-quoted string",
	6,  "SCE_PHP_STRING_VARIABLE", "identifier variable",   "Variable in string",
	7,  "SCE_PHP_STRING_DQ",       "literal string",        "Double-quoted string",
	8,  "SCE_PHP_HEREDOC",         "literal string",        "Heredoc",
	9,  "SCE_PHP_NOWDOC",          "literal string",        "Nowdoc",
	10, "SCE_PHP_CHARACTER",       "literal string character", "Character",
	11, "SCE_PHP_OPERATOR",        "operator",              "Operator",
	12, "SCE_PHP_NUMBER",          "literal numeric",       "Number",
	13, "SCE_PHP_WORD",            "keyword",               "Keywords",
	14, "SCE_PHP_WORD2",           "identifier",            "Builtin functions",
	15, "SCE_PHP_WORD3",           "identifier type",            "Builtin types",
	16, "SCE_PHP_WORD4",           "identifier",            "Other keywords",
	17, "SCE_PHP_IDENTIFIER",      "identifier",            "Identifier",
	18, "SCE_PHP_LEXERROR",        "error",                 "Lexical error",
	19, "SCE_PHP_OPEN_TAG",        "operator",              "PHP open tag",
	20, "SCE_PHP_CLOSE_TAG",       "operator",              "PHP close tag",
	21, "SCE_PHP_INLINE_HTML",     "default",               "Inline HTML",
};

class LexerPHP : public DefaultLexer {
	WordList keywords[NUM_PHP_KEYWORD_LISTS];
	OptionsPHP options;
	OptionSetPHP osPHP;
public:
	LexerPHP() : DefaultLexer("php", SCLEX_PHP, lexicalClasses, std::size(lexicalClasses)) {
	}
	virtual ~LexerPHP() {
	}
	void SCI_METHOD Release() override {
		delete this;
	}
	int SCI_METHOD Version() const override {
		return lvRelease5;
	}
	const char * SCI_METHOD PropertyNames() override {
		return osPHP.PropertyNames();
	}
	int SCI_METHOD PropertyType(const char *name) override {
		return osPHP.PropertyType(name);
	}
	const char * SCI_METHOD DescribeProperty(const char *name) override {
		return osPHP.DescribeProperty(name);
	}
	Sci_Position SCI_METHOD PropertySet(const char *key, const char *val) override;
	const char * SCI_METHOD PropertyGet(const char *key) override {
		return osPHP.PropertyGet(key);
	}
	const char * SCI_METHOD DescribeWordListSets() override {
		return osPHP.DescribeWordListSets();
	}
	Sci_Position SCI_METHOD WordListSet(int n, const char *wl) override;
	void SCI_METHOD Lex(Sci_PositionU startPos, Sci_Position length, int initStyle, IDocument *pAccess) override;
	void SCI_METHOD Fold(Sci_PositionU startPos, Sci_Position length, int initStyle, IDocument *pAccess) override;
	void * SCI_METHOD PrivateCall(int, void *) override {
		return nullptr;
	}
	static ILexer5 *LexerFactoryPHP() {
		return new LexerPHP();
	}
};

Sci_Position SCI_METHOD LexerPHP::PropertySet(const char *key, const char *val) {
	if (osPHP.PropertySet(&options, key, val)) {
		return 0;
	}
	return -1;
}

Sci_Position SCI_METHOD LexerPHP::WordListSet(int n, const char *wl) {
	Sci_Position firstModification = -1;
	if (n < NUM_PHP_KEYWORD_LISTS) {
		WordList *wordListN = &keywords[n];
		if (wordListN->Set(wl)) {
			firstModification = 0;
		}
	}
	return firstModification;
}

/* --------------------------- Small helpers ------------------------------ */

static bool IsWhitespace(int c) {
	return c == ' ' || c == '\t' || c == '\r' || c == '\n';
}

/* Not perfectly correct for Unicode identifiers, same as Rust lexer. */
static bool IsIdentifierStart(int ch) {
	return (IsASCII(ch) && (isalpha(ch) || ch == '_')) || (!IsASCII(ch) && ch >= 0x80);
}
static bool IsIdentifierContinue(int ch) {
	return (IsASCII(ch) && (isalnum(ch) || ch == '_')) || (!IsASCII(ch) && ch >= 0x80);
}

static bool IsStreamCommentStyle(int style) noexcept {
	return style == SCE_PHP_COMMENT || style == SCE_PHP_DOCCOMMENT;
}

static void GrabString(char *s, Accessor &styler, Sci_Position start, Sci_Position len) {
	for (Sci_Position i = 0; i < len; i++)
		s[i] = styler[i + start];
	s[len] = '\0';
}

/* -------------------------- Scanners ------------------------------------ */

static void ScanLineComment(Accessor &styler, Sci_Position &pos, Sci_Position max) {
	while (pos < max) {
		const int c = styler.SafeGetCharAt(pos, '\0');
		if (c == '\n' || c == '\r')
			break;
		pos++;
	}
	styler.ColourTo(pos - 1, SCE_PHP_COMMENTLINE);
}

/* pos points just past the opening "/*". */
static void ScanBlockComment(Accessor &styler, Sci_Position &pos, Sci_Position max, bool isDoc) {
	int level = 1;
	while (pos < max) {
		const int c = styler.SafeGetCharAt(pos, '\0');
		const int n = styler.SafeGetCharAt(pos + 1, '\0');
		if (c == '/' && n == '*') {
			level++;
			pos += 2;
		} else if (c == '*' && n == '/') {
			level--;
			pos += 2;
			if (level == 0)
				break;
		} else {
			pos++;
		}
	}
	styler.ColourTo(pos - 1, isDoc ? SCE_PHP_DOCCOMMENT : SCE_PHP_COMMENT);
}

/* pos points just past the opening quote.  Closes on the next unescaped '. */
static void ScanSingleQuotedString(Accessor &styler, Sci_Position &pos, Sci_Position max) {
	while (pos < max) {
		const int c = styler.SafeGetCharAt(pos, '\0');
		if (c == '\\') {
			pos += 2;
		} else if (c == '\'') {
			pos++;
			break;
		} else {
			pos++;
		}
	}
	styler.ColourTo(pos - 1, SCE_PHP_STRING);
}

/* pos points just past the opening ".  Handles escapes and $variables. */
static void ScanDoubleQuotedString(Accessor &styler, Sci_Position &pos, Sci_Position max) {
	while (pos < max) {
		const int c = styler.SafeGetCharAt(pos, '\0');
		if (c == '\\') {
			pos += 2;
		} else if (c == '"') {
			pos++;
			break;
		} else if (c == '$' && IsIdentifierStart(styler.SafeGetCharAt(pos + 1, '\0'))) {
			/* flush the string piece up to (but not including) the $ */
			styler.ColourTo(pos - 1, SCE_PHP_STRING_DQ);
			pos++;   // consume '$'
			while (pos < max && IsIdentifierContinue(styler.SafeGetCharAt(pos, '\0')))
				pos++;
			styler.ColourTo(pos - 1, SCE_PHP_STRING_VARIABLE);
		} else {
			pos++;
		}
	}
	styler.ColourTo(pos - 1, SCE_PHP_STRING_DQ);
}

static void ScanNumber(Accessor &styler, Sci_Position &pos, Sci_Position max) {
	const int c = styler.SafeGetCharAt(pos, '\0');
	const int n = styler.SafeGetCharAt(pos + 1, '\0');
	bool error = false;

	if (c == '0' && (n == 'x' || n == 'X')) {
		pos += 2;
		bool any = false;
		while (pos < max) {
			int d = styler.SafeGetCharAt(pos, '\0');
			if (IsADigit(d, 16) || d == '_') { pos++; any = true; } else break;
		}
		if (!any) error = true;
	} else if (c == '0' && (n == 'b' || n == 'B')) {
		pos += 2;
		bool any = false;
		while (pos < max) {
			int d = styler.SafeGetCharAt(pos, '\0');
			if (d == '0' || d == '1' || d == '_') { pos++; any = true; } else break;
		}
		if (!any) error = true;
	} else if (c == '0' && (n == 'o' || n == 'O')) {
		pos += 2;
		bool any = false;
		while (pos < max) {
			int d = styler.SafeGetCharAt(pos, '\0');
			if ((d >= '0' && d <= '7') || d == '_') { pos++; any = true; } else break;
		}
		if (!any) error = true;
	} else {
		while (pos < max) {
			int d = styler.SafeGetCharAt(pos, '\0');
			if ((d >= '0' && d <= '9') || d == '_') pos++; else break;
		}
		/* fractional part */
		if (styler.SafeGetCharAt(pos, '\0') == '.' &&
		    IsADigit(styler.SafeGetCharAt(pos + 1, '\0'), 10)) {
			pos++;
			while (pos < max) {
				int d = styler.SafeGetCharAt(pos, '\0');
				if ((d >= '0' && d <= '9') || d == '_') pos++; else break;
			}
		}
		/* exponent */
		int e = styler.SafeGetCharAt(pos, '\0');
		if (e == 'e' || e == 'E') {
			Sci_Position save = pos;
			pos++;
			int s = styler.SafeGetCharAt(pos, '\0');
			if (s == '+' || s == '-') pos++;
			if (IsADigit(styler.SafeGetCharAt(pos, '\0'), 10)) {
				while (pos < max) {
					int d = styler.SafeGetCharAt(pos, '\0');
					if ((d >= '0' && d <= '9') || d == '_') pos++; else break;
				}
			} else {
				pos = save;
			}
		}
	}

	styler.ColourTo(pos - 1, error ? SCE_PHP_LEXERROR : SCE_PHP_NUMBER);
}

static void ScanIdentifier(Accessor &styler, Sci_Position &pos, Sci_Position max, WordList *keywords) {
	const Sci_Position start = pos;
	while (pos < max && IsIdentifierContinue(styler.SafeGetCharAt(pos, '\0')))
		pos++;

	char s[MAX_PHP_IDENT_CHARS + 1];
	Sci_Position len = pos - start;
	len = len > MAX_PHP_IDENT_CHARS ? MAX_PHP_IDENT_CHARS : len;
	GrabString(s, styler, start, len);

	for (int ii = 0; ii < NUM_PHP_KEYWORD_LISTS; ii++) {
		if (keywords[ii].InList(s)) {
			styler.ColourTo(pos - 1, SCE_PHP_WORD + ii);
			return;
		}
	}
	styler.ColourTo(pos - 1, SCE_PHP_IDENTIFIER);
}

/* Find the "<<<" that opened the heredoc containing `pos`. */
static bool FindHeredocStart(Accessor &styler, Sci_Position pos,
                             Sci_Position &idStart, Sci_Position &idLen,
                             bool &nowdoc) {
	Sci_Position line = styler.GetLine(pos);
	for (; line >= 0; line--) {
		const Sci_Position lineStart = styler.LineStart(line);
		const Sci_Position lineEnd   = styler.LineStart(line + 1);
		for (Sci_Position p = lineStart; p + 2 < lineEnd; p++) {
			if (styler.SafeGetCharAt(p,     '\0') == '<' &&
			    styler.SafeGetCharAt(p + 1, '\0') == '<' &&
			    styler.SafeGetCharAt(p + 2, '\0') == '<') {
				p += 3;
				int q = styler.SafeGetCharAt(p, '\0');
				if (q == '\'' || q == '"') {
					nowdoc = (q == '\'');
					p++;
				} else {
					nowdoc = false;
				}
				idStart = p;
				while (IsIdentifierContinue(styler.SafeGetCharAt(p, '\0')))
					p++;
				idLen = p - idStart;
				return idLen > 0;
			}
		}
	}
	return false;
}

/* Scan from `pos` (at the beginning of the heredoc body) to the closing
 * identifier.  The identifier is described by [idStart, idStart+idLen). */
static void ScanHeredocBody(Accessor &styler, Sci_Position &pos, Sci_Position max,
                            Sci_Position idStart, Sci_Position idLen, bool nowdoc) {
	const int style = nowdoc ? SCE_PHP_NOWDOC : SCE_PHP_HEREDOC;
	while (pos < max) {
		const Sci_Position lineStart = pos;
		Sci_Position p = lineStart;
		/* PHP 7.3+ allows indentation before the closing identifier. */
		while (p < max) {
			const int c = styler.SafeGetCharAt(p, '\0');
			if (c == ' ' || c == '\t') p++; else break;
		}
		const Sci_Position wordStart = p;
		while (IsIdentifierContinue(styler.SafeGetCharAt(p, '\0')))
			p++;
		const Sci_Position wordLen = p - wordStart;
		if (wordLen == idLen) {
			bool match = true;
			for (Sci_Position i = 0; i < idLen; i++) {
				if (styler.SafeGetCharAt(idStart + i, '\0') !=
				    styler.SafeGetCharAt(wordStart + i, '\0')) {
					match = false;
					break;
				}
			}
			if (match && !IsIdentifierContinue(styler.SafeGetCharAt(p, '\0'))) {
				pos = p;
				styler.ColourTo(pos - 1, style);
				return;
			}
		}
		/* Not the end - advance to the next line. */
		pos = lineStart;
		while (pos < max) {
			const int c = styler.SafeGetCharAt(pos, '\0');
			if (c == '\n') { pos++; break; }
			pos++;
		}
	}
	styler.ColourTo(pos - 1, style);
}

/* pos points just past "<<<". */
static void ScanHeredoc(Accessor &styler, Sci_Position &pos, Sci_Position max) {
	int quote = 0;
	const int c = styler.SafeGetCharAt(pos, '\0');
	if (c == '\'' || c == '"') {
		quote = c;
		pos++;
	}
	const Sci_Position idStart = pos;
	while (IsIdentifierContinue(styler.SafeGetCharAt(pos, '\0')))
		pos++;
	const Sci_Position idLen = pos - idStart;
	if (idLen == 0) {
		styler.ColourTo(pos - 1, SCE_PHP_LEXERROR);
		return;
	}
	if (quote) {
		if (styler.SafeGetCharAt(pos, '\0') != quote) {
			styler.ColourTo(pos - 1, SCE_PHP_LEXERROR);
			return;
		}
		pos++;
	}
	/* Skip the rest of the opening line. */
	while (pos < max) {
		const int ch = styler.SafeGetCharAt(pos, '\0');
		if (ch == '\n') { pos++; break; }
		pos++;
	}
	ScanHeredocBody(styler, pos, max, idStart, idLen, quote == '\'');
}

static bool IsOperatorChar(int c) {
	return c == '+' || c == '-' || c == '*' || c == '/' || c == '%' ||
	       c == '=' || c == '!' || c == '<' || c == '>' || c == '&' ||
	       c == '|' || c == '^' || c == '~' || c == '?' || c == ':' ||
	       c == ';' || c == ',' || c == '.' || c == '(' || c == ')' ||
	       c == '[' || c == ']' || c == '{' || c == '}' || c == '@' ||
	       c == '\\';
}

/* -------------------------------- Lex ----------------------------------- */

void SCI_METHOD LexerPHP::Lex(Sci_PositionU startPos, Sci_Position length, int initStyle, IDocument *pAccess) {
	PropSetSimple props;
	Accessor styler(pAccess, &props);
	Sci_Position pos = startPos;
	const Sci_Position max = pos + length;

	styler.StartAt(pos);
	styler.StartSegment(pos);

	bool inPhp = (initStyle != SCE_PHP_INLINE_HTML);

	/* First call on the document: sniff whether we start inside or outside PHP. */
	if (startPos == 0 && initStyle == SCE_PHP_DEFAULT) {
		Sci_Position p = 0;
		while (p < max && IsWhitespace(styler.SafeGetCharAt(p, '\0')))
			p++;
		inPhp = !(p + 1 < max &&
		          styler.SafeGetCharAt(p,     '\0') == '<' &&
		          styler.SafeGetCharAt(p + 1, '\0') == '?');
	}

	/* Resume multi-line constructs. */
	if (initStyle == SCE_PHP_COMMENT || initStyle == SCE_PHP_DOCCOMMENT) {
		ScanBlockComment(styler, pos, max, initStyle == SCE_PHP_DOCCOMMENT);
	} else if (initStyle == SCE_PHP_COMMENTLINE) {
		ScanLineComment(styler, pos, max);
	} else if (initStyle == SCE_PHP_STRING) {
		ScanSingleQuotedString(styler, pos, max);
	} else if (initStyle == SCE_PHP_STRING_DQ) {
		ScanDoubleQuotedString(styler, pos, max);
	} else if (initStyle == SCE_PHP_HEREDOC || initStyle == SCE_PHP_NOWDOC) {
		Sci_Position idStart = 0, idLen = 0;
		bool nowdoc = (initStyle == SCE_PHP_NOWDOC);
		if (FindHeredocStart(styler, pos, idStart, idLen, nowdoc)) {
			ScanHeredocBody(styler, pos, max, idStart, idLen, nowdoc);
		} else {
			/* Couldn't locate the opener - colour what we have. */
			styler.ColourTo(max - 1, initStyle);
			pos = max;
		}
	}

	while (pos < max) {
		const int c  = styler.SafeGetCharAt(pos,     '\0');
		const int n  = styler.SafeGetCharAt(pos + 1, '\0');
		const int n2 = styler.SafeGetCharAt(pos + 2, '\0');
		const int n3 = styler.SafeGetCharAt(pos + 3, '\0');

		/* -------------------- inline HTML mode -------------------- */
		if (!inPhp) {
			if (c == '<' && n == '?') {
				if (n2 == 'p' && n3 == 'h' &&
				    styler.SafeGetCharAt(pos + 4, '\0') == 'p') {
					pos += 5;
					styler.ColourTo(pos - 1, SCE_PHP_OPEN_TAG);
					inPhp = true;
					continue;
				}
				if (n2 == '=') {
					pos += 3;
					styler.ColourTo(pos - 1, SCE_PHP_OPEN_TAG);
					inPhp = true;
					continue;
				}
				if (n2 == ' ' || n2 == '\t' || n2 == '\n' || n2 == '\r') {
					pos += 2;
					styler.ColourTo(pos - 1, SCE_PHP_OPEN_TAG);
					inPhp = true;
					continue;
				}
			}
			/* Consume HTML text up to the next "<?" so we colour it in one go. */
			const Sci_Position segStart = pos;
			while (pos < max) {
				const int h = styler.SafeGetCharAt(pos, '\0');
				if (h == '<' && styler.SafeGetCharAt(pos + 1, '\0') == '?')
					break;
				pos++;
			}
			styler.ColourTo(pos - 1, SCE_PHP_INLINE_HTML);
			(void)segStart;
			continue;
		}

		/* ------------------------ PHP mode ------------------------ */

		if (IsWhitespace(c)) {
			while (pos < max && IsWhitespace(styler.SafeGetCharAt(pos, '\0')))
				pos++;
			styler.ColourTo(pos - 1, SCE_PHP_DEFAULT);
		} else if (c == '?' && n == '>') {
			pos += 2;
			styler.ColourTo(pos - 1, SCE_PHP_CLOSE_TAG);
			inPhp = false;
		} else if (c == '<' && n == '?') {
			/* We may start in PHP mode because initStyle was DEFAULT on a
			 * document that in fact begins with <?php - handle it here. */
			if (n2 == 'p' && n3 == 'h' &&
			    styler.SafeGetCharAt(pos + 4, '\0') == 'p') {
				pos += 5;
				styler.ColourTo(pos - 1, SCE_PHP_OPEN_TAG);
			} else if (n2 == '=') {
				pos += 3;
				styler.ColourTo(pos - 1, SCE_PHP_OPEN_TAG);
			} else {
				pos += 2;
				styler.ColourTo(pos - 1, SCE_PHP_OPERATOR);
			}
		} else if (c == '/' && n == '/') {
			pos += 2;
			ScanLineComment(styler, pos, max);
		} else if (c == '#') {
			if (n == '[') {           /* PHP 8 attribute, not a comment. */
				pos++;
				styler.ColourTo(pos - 1, SCE_PHP_OPERATOR);
			} else {
				pos++;
				ScanLineComment(styler, pos, max);
			}
		} else if (c == '/' && n == '*') {
			const bool isDoc = (n2 == '*' &&
			                    styler.SafeGetCharAt(pos + 3, '\0') != '/');
			pos += 2;
			ScanBlockComment(styler, pos, max, isDoc);
		} else if (c == '\'') {
			pos++;
			ScanSingleQuotedString(styler, pos, max);
		} else if (c == '"') {
			pos++;
			ScanDoubleQuotedString(styler, pos, max);
		} else if ((c == 'b' || c == 'B') && (n == '\'' || n == '"')) {
			/* binary string prefix */
			pos++;
			if (styler.SafeGetCharAt(pos, '\0') == '\'') {
				pos++;
				ScanSingleQuotedString(styler, pos, max);
			} else {
				pos++;
				ScanDoubleQuotedString(styler, pos, max);
			}
		} else if (c == '$' && IsIdentifierStart(n)) {
			pos += 2;
			while (pos < max && IsIdentifierContinue(styler.SafeGetCharAt(pos, '\0')))
				pos++;
			styler.ColourTo(pos - 1, SCE_PHP_VARIABLE);
		} else if (c == '<' && n == '<' && n2 == '<') {
			pos += 3;
			ScanHeredoc(styler, pos, max);
		} else if (IsIdentifierStart(c)) {
			ScanIdentifier(styler, pos, max, keywords);
		} else if (c >= '0' && c <= '9') {
			ScanNumber(styler, pos, max);
		} else if (IsOperatorChar(c)) {
			pos++;
			styler.ColourTo(pos - 1, SCE_PHP_OPERATOR);
		} else {
			pos++;
			styler.ColourTo(pos - 1, SCE_PHP_LEXERROR);
		}
	}

	styler.ColourTo(pos - 1, SCE_PHP_DEFAULT);
	styler.Flush();
}

/* -------------------------------- Fold ---------------------------------- */

void SCI_METHOD LexerPHP::Fold(Sci_PositionU startPos, Sci_Position length, int initStyle, IDocument *pAccess) {
	if (!options.fold)
		return;

	LexAccessor styler(pAccess);

	const Sci_PositionU endPos = startPos + length;
	int visibleChars = 0;
	bool inLineComment = false;
	Sci_Position lineCurrent = styler.GetLine(startPos);
	int levelCurrent = SC_FOLDLEVELBASE;
	if (lineCurrent > 0)
		levelCurrent = styler.LevelAt(lineCurrent - 1) >> 16;
	Sci_PositionU lineStartNext = styler.LineStart(lineCurrent + 1);
	int levelMinCurrent = levelCurrent;
	int levelNext = levelCurrent;
	char chNext = styler[startPos];
	int styleNext = styler.StyleAt(startPos);
	int style = initStyle;

	for (Sci_PositionU i = startPos; i < endPos; i++) {
		const char ch = chNext;
		chNext = styler.SafeGetCharAt(i + 1);
		const int stylePrev = style;
		style = styleNext;
		styleNext = styler.StyleAt(i + 1);
		const bool atEOL = i == (lineStartNext - 1);

		if (style == SCE_PHP_COMMENTLINE)
			inLineComment = true;

		if (options.foldComment && options.foldCommentMultiline &&
		    IsStreamCommentStyle(style) && !inLineComment) {
			if (!IsStreamCommentStyle(stylePrev)) {
				levelNext++;
			} else if (!IsStreamCommentStyle(styleNext) && !atEOL) {
				levelNext--;
			}
		}

		if (options.foldSyntaxBased && style == SCE_PHP_OPERATOR) {
			if (ch == '{') {
				if (levelMinCurrent > levelNext)
					levelMinCurrent = levelNext;
				levelNext++;
			} else if (ch == '}') {
				levelNext--;
			}
		}

		if (!IsASpace(ch))
			visibleChars++;

		if (atEOL || (i == endPos - 1)) {
			int levelUse = levelCurrent;
			if (options.foldSyntaxBased && options.foldAtElse)
				levelUse = levelMinCurrent;
			int lev = levelUse | levelNext << 16;
			if (visibleChars == 0 && options.foldCompact)
				lev |= SC_FOLDLEVELWHITEFLAG;
			if (levelUse < levelNext)
				lev |= SC_FOLDLEVELHEADERFLAG;
			if (lev != styler.LevelAt(lineCurrent)) {
				styler.SetLevel(lineCurrent, lev);
			}
			lineCurrent++;
			lineStartNext = styler.LineStart(lineCurrent + 1);
			levelCurrent = levelNext;
			levelMinCurrent = levelCurrent;
			if (atEOL && (i == static_cast<Sci_PositionU>(styler.Length() - 1))) {
				styler.SetLevel(lineCurrent,
				                (levelCurrent | levelCurrent << 16) | SC_FOLDLEVELWHITEFLAG);
			}
			visibleChars = 0;
			inLineComment = false;
		}
	}
}

} // anonymous namespace

extern const LexerModule lmPHP(SCLEX_PHP, LexerPHP::LexerFactoryPHP, "php", phpWordLists);
