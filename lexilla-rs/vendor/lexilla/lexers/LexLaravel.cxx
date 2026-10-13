/** @file LexLaravel.cxx
 ** Laravel-aware lexer for Scintilla / Lexilla 5.
 ** Extends LexPHP.cxx with Laravel-specific keywords, facades, route
 ** methods, Eloquent builders, middleware aliases and Artisan commands.
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
 *  SCLEX_LARAVEL is our own id; pick a value that doesn't clash with any
 *  upstream SCLEX_* constant in your fork.
 * ------------------------------------------------------------------------- */
#ifndef SCLEX_LARAVEL
#define SCLEX_LARAVEL 144
#endif

#ifndef SCE_LARAVEL_DEFAULT
#define SCE_LARAVEL_DEFAULT          0
#define SCE_LARAVEL_COMMENT          1
#define SCE_LARAVEL_COMMENTLINE      2
#define SCE_LARAVEL_DOCCOMMENT       3
#define SCE_LARAVEL_VARIABLE         4
#define SCE_LARAVEL_STRING           5
#define SCE_LARAVEL_STRING_VARIABLE  6
#define SCE_LARAVEL_STRING_DQ        7
#define SCE_LARAVEL_HEREDOC          8
#define SCE_LARAVEL_NOWDOC           9
#define SCE_LARAVEL_CHARACTER        10
#define SCE_LARAVEL_OPERATOR         11
#define SCE_LARAVEL_NUMBER           12
#define SCE_LARAVEL_WORD             13   /* PHP keywords            */
#define SCE_LARAVEL_WORD2            14   /* Builtin functions       */
#define SCE_LARAVEL_WORD3            15   /* Builtin types           */
#define SCE_LARAVEL_WORD4            16   /* Other PHP keywords      */
#define SCE_LARAVEL_IDENTIFIER       17
#define SCE_LARAVEL_LEXERROR         18
#define SCE_LARAVEL_OPEN_TAG         19
#define SCE_LARAVEL_CLOSE_TAG        20
#define SCE_LARAVEL_INLINE_HTML      21
/* ---- Laravel-specific styles -------------------------------------------- */
#define SCE_LARAVEL_FACADE           22   /* Route, Auth, DB, Cache, ...    */
#define SCE_LARAVEL_ROUTE_METHOD     23   /* get, post, resource, ...       */
#define SCE_LARAVEL_MIDDLEWARE       24   /* auth, guest, throttle, can, ...*/
#define SCE_LARAVEL_ELOQUENT         25   /* where, with, hasMany, ...      */
#define SCE_LARAVEL_ARTISAN          26   /* make:model, migrate, ...       */
#define SCE_LARAVEL_HELPER           27   /* route(), view(), config(), ... */
#define SCE_LARAVEL_ATTRIBUTE        28   /* #[Middleware], #[Authorize]    */
#define SCE_LARAVEL_DIRECTIVE        29   /* @if, @foreach, @yield, ...     */
#define SCE_LARAVEL_DIRECTIVE_END    30   /* @endif, @endforeach, ...       */
/* ---- Blade template styles (lexer "blade" only) -------------------------- */
#define SCE_LARAVEL_BLADE_COMMENT    31   /* {{-- ... --}}                  */
#define SCE_LARAVEL_BLADE_DELIM      32   /* {{ }}  {!! !!}                 */
#define SCE_LARAVEL_HTML_TAG         33   /* <div  </div  >  />  <!DOCTYPE */
#define SCE_LARAVEL_HTML_ATTR        34   /* attribute name                 */
#define SCE_LARAVEL_HTML_VALUE       35   /* attribute value                */
#define SCE_LARAVEL_HTML_COMMENT     36   /* <!-- ... -->                   */
#define SCE_LARAVEL_HTML_ENTITY      37   /* &amp; &#39;                    */
#endif
#ifndef SCLEX_BLADE
#define SCLEX_BLADE 145
#endif

namespace {

constexpr int NUM_LARAVEL_KEYWORD_LISTS = 9;
constexpr int MAX_LARAVEL_IDENT_CHARS   = 1023;

/* Word-list indexes (keep in sync with laravelWordLists[] below). */
enum {
	WL_PHP_KEYWORDS = 0,
	WL_PHP_BUILTINS,
	WL_PHP_TYPES,
	WL_PHP_OTHER,
	WL_LARAVEL_FACADES,
	WL_LARAVEL_METHODS,       /* route methods (after ::)               */
	WL_LARAVEL_ARTISAN,       /* Artisan commands and sub-commands      */
	WL_LARAVEL_HELPERS,       /* route(), view(), config(), env(), ...  */
	WL_LARAVEL_ELOQUENT,      /* Eloquent builder methods (after ::)    */
};

/* ----------------------------- Options ---------------------------------- */

struct OptionsLaravel {
	bool fold;
	bool foldSyntaxBased;
	bool foldComment;
	bool foldCommentMultiline;
	bool foldCompact;
	bool foldAtElse;
	bool colourLaravel;   /* turn the Laravel extras on/off          */
	OptionsLaravel() {
		fold = false;
		foldSyntaxBased = true;
		foldComment = false;
		foldCommentMultiline = true;
		foldCompact = true;
		foldAtElse = false;
		colourLaravel = true;
	}
};

static const char * const laravelWordLists[NUM_LARAVEL_KEYWORD_LISTS + 1] = {
	"PHP keywords",
	"PHP builtin functions",
	"PHP builtin types",
	"PHP other keywords",
	"Laravel facades",
	"Laravel route methods",
	"Laravel Artisan commands",
	"Laravel helpers",
	"Laravel Eloquent methods",
	nullptr,
};

struct OptionSetLaravel : public OptionSet<OptionsLaravel> {
	OptionSetLaravel() {
		DefineProperty("fold", &OptionsLaravel::fold);
		DefineProperty("fold.comment", &OptionsLaravel::foldComment);
		DefineProperty("fold.compact", &OptionsLaravel::foldCompact);
		DefineProperty("fold.at.else", &OptionsLaravel::foldAtElse);

		DefineProperty("fold.laravel.syntax.based", &OptionsLaravel::foldSyntaxBased,
			"Set this property to 0 to disable syntax based folding.");

		DefineProperty("fold.laravel.comment.multiline", &OptionsLaravel::foldCommentMultiline,
			"Set this property to 0 to disable folding multi-line comments when fold.comment=1.");

		DefineProperty("lexer.laravel.colour", &OptionsLaravel::colourLaravel,
			"Set this property to 0 to disable Laravel-specific colouring and fall back to plain PHP.");

		DefineWordListSets(laravelWordLists);
	}
};

const LexicalClass lexicalClasses[] = {
	// Lexer laravel SCLEX_LARAVEL SCE_LARAVEL_:
	0,  "SCE_LARAVEL_DEFAULT",         "default",               "White space / inline HTML",
	1,  "SCE_LARAVEL_COMMENT",         "comment",               "Comment",
	2,  "SCE_LARAVEL_COMMENTLINE",     "comment line",          "Line comment",
	3,  "SCE_LARAVEL_DOCCOMMENT",      "comment documentation", "Doc comment",
	4,  "SCE_LARAVEL_VARIABLE",        "identifier variable",   "Variable",
	5,  "SCE_LARAVEL_STRING",          "literal string",        "Single-quoted string",
	6,  "SCE_LARAVEL_STRING_VARIABLE", "identifier variable",   "Variable in string",
	7,  "SCE_LARAVEL_STRING_DQ",       "literal string",        "Double-quoted string",
	8,  "SCE_LARAVEL_HEREDOC",         "literal string",        "Heredoc",
	9,  "SCE_LARAVEL_NOWDOC",          "literal string",        "Nowdoc",
	10, "SCE_LARAVEL_CHARACTER",       "literal string character", "Character",
	11, "SCE_LARAVEL_OPERATOR",        "operator",              "Operator",
	12, "SCE_LARAVEL_NUMBER",          "literal numeric",       "Number",
	13, "SCE_LARAVEL_WORD",            "keyword",               "PHP keywords",
	14, "SCE_LARAVEL_WORD2",           "identifier",            "PHP builtin functions",
	15, "SCE_LARAVEL_WORD3",           "identifier type",            "PHP builtin types",
	16, "SCE_LARAVEL_WORD4",           "identifier",            "PHP other keywords",
	17, "SCE_LARAVEL_IDENTIFIER",      "identifier",            "Identifier",
	18, "SCE_LARAVEL_LEXERROR",        "error",                 "Lexical error",
	19, "SCE_LARAVEL_OPEN_TAG",        "operator",              "PHP open tag",
	20, "SCE_LARAVEL_CLOSE_TAG",       "operator",              "PHP close tag",
	21, "SCE_LARAVEL_INLINE_HTML",     "default",               "Inline HTML",
	/* Laravel specific */
	22, "SCE_LARAVEL_FACADE",          "type namespace",             "Laravel facade",
	23, "SCE_LARAVEL_ROUTE_METHOD",    "function",              "Route method",
	24, "SCE_LARAVEL_MIDDLEWARE",      "keyword",               "Middleware alias",
	25, "SCE_LARAVEL_ELOQUENT",        "function",              "Eloquent builder method",
	26, "SCE_LARAVEL_ARTISAN",         "function",              "Artisan command",
	27, "SCE_LARAVEL_HELPER",          "function",              "Laravel helper",
	28, "SCE_LARAVEL_ATTRIBUTE",       "type annotation",            "PHP attribute",
	29, "SCE_LARAVEL_DIRECTIVE",       "keyword",               "Blade directive",
	30, "SCE_LARAVEL_DIRECTIVE_END",   "keyword",               "Blade end directive",
	/* Blade template */
	31, "SCE_LARAVEL_BLADE_COMMENT",   "comment",               "Blade comment",
	32, "SCE_LARAVEL_BLADE_DELIM",     "keyword",               "Blade echo delimiter",
	33, "SCE_LARAVEL_HTML_TAG",        "tag",                   "HTML tag",
	34, "SCE_LARAVEL_HTML_ATTR",       "identifier type",       "HTML attribute name",
	35, "SCE_LARAVEL_HTML_VALUE",      "literal string",        "HTML attribute value",
	36, "SCE_LARAVEL_HTML_COMMENT",    "comment",               "HTML comment",
	37, "SCE_LARAVEL_HTML_ENTITY",     "literal numeric",       "HTML entity",
};

class LexerLaravel : public DefaultLexer {
	WordList keywords[NUM_LARAVEL_KEYWORD_LISTS];
	OptionsLaravel options;
	OptionSetLaravel osLaravel;
public:
	const bool blade;
	explicit LexerLaravel(bool isBlade = false) :
		DefaultLexer(isBlade ? "blade" : "laravel", isBlade ? SCLEX_BLADE : SCLEX_LARAVEL,
		             lexicalClasses, std::size(lexicalClasses)), blade(isBlade) {
	}
	virtual ~LexerLaravel() {
	}
	void SCI_METHOD Release() override {
		delete this;
	}
	int SCI_METHOD Version() const override {
		return lvRelease5;
	}
	const char * SCI_METHOD PropertyNames() override {
		return osLaravel.PropertyNames();
	}
	int SCI_METHOD PropertyType(const char *name) override {
		return osLaravel.PropertyType(name);
	}
	const char * SCI_METHOD DescribeProperty(const char *name) override {
		return osLaravel.DescribeProperty(name);
	}
	Sci_Position SCI_METHOD PropertySet(const char *key, const char *val) override;
	const char * SCI_METHOD PropertyGet(const char *key) override {
		return osLaravel.PropertyGet(key);
	}
	const char * SCI_METHOD DescribeWordListSets() override {
		return osLaravel.DescribeWordListSets();
	}
	Sci_Position SCI_METHOD WordListSet(int n, const char *wl) override;
	void SCI_METHOD Lex(Sci_PositionU startPos, Sci_Position length, int initStyle, IDocument *pAccess) override;
	void SCI_METHOD Fold(Sci_PositionU startPos, Sci_Position length, int initStyle, IDocument *pAccess) override;
	void * SCI_METHOD PrivateCall(int, void *) override {
		return nullptr;
	}
	void PhpToken(Accessor &styler, Sci_Position &pos, Sci_Position max, bool &inPhp);
	void PhpRegion(Accessor &styler, Sci_Position pos, Sci_Position end);
	bool TryBlade(Accessor &styler, Sci_Position &pos, Sci_Position max);
	void LexBlade(Accessor &styler, Sci_Position max);
	static ILexer5 *LexerFactoryLaravel() {
		return new LexerLaravel();
	}
	static ILexer5 *LexerFactoryBlade() {
		return new LexerLaravel(true);
	}
};

Sci_Position SCI_METHOD LexerLaravel::PropertySet(const char *key, const char *val) {
	if (osLaravel.PropertySet(&options, key, val)) {
		return 0;
	}
	return -1;
}

Sci_Position SCI_METHOD LexerLaravel::WordListSet(int n, const char *wl) {
	Sci_Position firstModification = -1;
	if (n < NUM_LARAVEL_KEYWORD_LISTS) {
		WordList *wordListN = &keywords[n];
		if (wordListN->Set(wl)) {
			firstModification = 0;
		}
	}
	return firstModification;
}

/* ------------------------- PHP scanners (copied from LexPHP) ------------- */

static bool IsWhitespace(int c) {
	return c == ' ' || c == '\t' || c == '\r' || c == '\n';
}

static bool IsIdentifierStart(int ch) {
	return (IsASCII(ch) && (isalpha(ch) || ch == '_')) || (!IsASCII(ch) && ch >= 0x80);
}
static bool IsIdentifierContinue(int ch) {
	return (IsASCII(ch) && (isalnum(ch) || ch == '_')) || (!IsASCII(ch) && ch >= 0x80);
}

static bool IsStreamCommentStyle(int style) noexcept {
	return style == SCE_LARAVEL_COMMENT || style == SCE_LARAVEL_DOCCOMMENT;
}

static void GrabString(char *s, Accessor &styler, Sci_Position start, Sci_Position len) {
	for (Sci_Position i = 0; i < len; i++)
		s[i] = styler[i + start];
	s[len] = '\0';
}

static void ScanLineComment(Accessor &styler, Sci_Position &pos, Sci_Position max) {
	while (pos < max) {
		const int c = styler.SafeGetCharAt(pos, '\0');
		if (c == '\n' || c == '\r')
			break;
		pos++;
	}
	styler.ColourTo(pos - 1, SCE_LARAVEL_COMMENTLINE);
}

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
	styler.ColourTo(pos - 1, isDoc ? SCE_LARAVEL_DOCCOMMENT : SCE_LARAVEL_COMMENT);
}

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
	styler.ColourTo(pos - 1, SCE_LARAVEL_STRING);
}

static void ScanDoubleQuotedString(Accessor &styler, Sci_Position &pos, Sci_Position max) {
	while (pos < max) {
		const int c = styler.SafeGetCharAt(pos, '\0');
		if (c == '\\') {
			pos += 2;
		} else if (c == '"') {
			pos++;
			break;
		} else if (c == '$' && IsIdentifierStart(styler.SafeGetCharAt(pos + 1, '\0'))) {
			styler.ColourTo(pos - 1, SCE_LARAVEL_STRING_DQ);
			pos++;
			while (pos < max && IsIdentifierContinue(styler.SafeGetCharAt(pos, '\0')))
				pos++;
			styler.ColourTo(pos - 1, SCE_LARAVEL_STRING_VARIABLE);
		} else {
			pos++;
		}
	}
	styler.ColourTo(pos - 1, SCE_LARAVEL_STRING_DQ);
}

static void ScanNumber(Accessor &styler, Sci_Position &pos, Sci_Position max) {
	/* Same numeric logic as LexPHP.cxx - kept compact here. */
	const int c = styler.SafeGetCharAt(pos, '\0');
	const int n = styler.SafeGetCharAt(pos + 1, '\0');
	if (c == '0' && (n == 'x' || n == 'X')) {
		pos += 2;
		while (pos < max) {
			int d = styler.SafeGetCharAt(pos, '\0');
			if (IsADigit(d, 16) || d == '_') pos++; else break;
		}
	} else if (c == '0' && (n == 'b' || n == 'B')) {
		pos += 2;
		while (pos < max) {
			int d = styler.SafeGetCharAt(pos, '\0');
			if (d == '0' || d == '1' || d == '_') pos++; else break;
		}
	} else {
		while (pos < max) {
			int d = styler.SafeGetCharAt(pos, '\0');
			if ((d >= '0' && d <= '9') || d == '_') pos++; else break;
		}
		if (styler.SafeGetCharAt(pos, '\0') == '.' &&
		    IsADigit(styler.SafeGetCharAt(pos + 1, '\0'), 10)) {
			pos++;
			while (pos < max) {
				int d = styler.SafeGetCharAt(pos, '\0');
				if ((d >= '0' && d <= '9') || d == '_') pos++; else break;
			}
		}
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
	styler.ColourTo(pos - 1, SCE_LARAVEL_NUMBER);
}

static void ScanIdentifier(Accessor &styler, Sci_Position &pos, Sci_Position max, WordList *keywords) {
	const Sci_Position start = pos;
	while (pos < max && IsIdentifierContinue(styler.SafeGetCharAt(pos, '\0')))
		pos++;

	char s[MAX_LARAVEL_IDENT_CHARS + 1];
	Sci_Position len = pos - start;
	len = len > MAX_LARAVEL_IDENT_CHARS ? MAX_LARAVEL_IDENT_CHARS : len;
	GrabString(s, styler, start, len);

	for (int ii = 0; ii < NUM_LARAVEL_KEYWORD_LISTS; ii++) {
		if (keywords[ii].InList(s)) {
			styler.ColourTo(pos - 1, SCE_LARAVEL_WORD + ii);
			return;
		}
	}
	styler.ColourTo(pos - 1, SCE_LARAVEL_IDENTIFIER);
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

static void ScanHeredocBody(Accessor &styler, Sci_Position &pos, Sci_Position max,
                            Sci_Position idStart, Sci_Position idLen, bool nowdoc) {
	const int style = nowdoc ? SCE_LARAVEL_NOWDOC : SCE_LARAVEL_HEREDOC;
	while (pos < max) {
		const Sci_Position lineStart = pos;
		Sci_Position p = lineStart;
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
		pos = lineStart;
		while (pos < max) {
			const int c = styler.SafeGetCharAt(pos, '\0');
			if (c == '\n') { pos++; break; }
			pos++;
		}
	}
	styler.ColourTo(pos - 1, style);
}

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
		styler.ColourTo(pos - 1, SCE_LARAVEL_LEXERROR);
		return;
	}
	if (quote) {
		if (styler.SafeGetCharAt(pos, '\0') != quote) {
			styler.ColourTo(pos - 1, SCE_LARAVEL_LEXERROR);
			return;
		}
		pos++;
	}
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

/* -------------------- Laravel-specific recognisers ----------------------- */

/*
 * Middleware aliases. A single-quoted string whose content is a known alias
 * (optionally with parameters, "throttle:60,1") is coloured as a middleware
 * alias, but only when the same line before the string mentions "middleware"
 * (->middleware('auth'), 'middleware' => ['auth', 'verified']). Anywhere else
 * 'auth' or 'web' are ordinary strings (config('auth'), guard('web')).
 */
static bool IsKnownMiddleware(const char *s) {
	static const char * const aliases[] = {
		"auth", "auth.basic", "auth.session", "guest", "bindings",
		"cache.headers", "can", "password.confirm", "signed",
		"subscribed", "throttle", "verified", "web", "api",
		"localize", "setlocale", "encrypt.cookies", "start.session",
		"share.errors", "csrf", "sanctum", "ability", "role",
		"permission", "verified.api", "ensure.frontend",
		nullptr
	};
	for (int i = 0; aliases[i]; i++)
		if (strcmp(s, aliases[i]) == 0)
			return true;
	return false;
}

/* Is the single-quoted string opening at `quote` a middleware alias? On true,
 * `end` is the position just after the closing quote. */
static bool IsMiddlewareString(Accessor &styler, Sci_Position quote, Sci_Position max,
                               Sci_Position &end) {
	char content[64];
	Sci_Position p = quote + 1;
	int n = 0;
	while (p < max) {
		const int ch = styler.SafeGetCharAt(p, '\0');
		if (ch == '\'')
			break;
		if (ch == '\\' || ch == '\n' || ch == '\r' || n >= 63)
			return false;
		if (ch == ':' ) {
			/* "throttle:60,1": only the alias before ':' matters */
			while (p < max && styler.SafeGetCharAt(p, '\0') != '\'') {
				const int c2 = styler.SafeGetCharAt(p, '\0');
				if (c2 == '\\' || c2 == '\n' || c2 == '\r')
					return false;
				p++;
			}
			break;
		}
		content[n++] = static_cast<char>(ch);
		p++;
	}
	if (p >= max || styler.SafeGetCharAt(p, '\0') != '\'')
		return false;
	content[n] = '\0';
	if (!IsKnownMiddleware(content))
		return false;
	/* "middleware" must appear earlier on the same line. */
	Sci_Position q = quote - 1;
	char word[11];
	while (q >= 0) {
		const int ch = styler.SafeGetCharAt(q, '\0');
		if (ch == '\n' || ch == '\r')
			break;
		if ((ch == 'e' || ch == 'E') && q >= 9) {
			GrabString(word, styler, q - 9, 10);
			word[10] = '\0';
			bool match = true;
			static const char target[] = "middleware";
			for (int i = 0; i < 10; i++)
				if (tolower(static_cast<unsigned char>(word[i])) != target[i]) { match = false; break; }
			if (match) {
				end = p + 1;
				return true;
			}
		}
		q--;
	}
	return false;
}

/* Blade directives recognised in inline-HTML mode. */
static bool IsBladeDirective(const char *s) {
	static const char * const names[] = {
		"if", "elseif", "else", "unless", "isset", "empty", "auth", "guest",
		"foreach", "forelse", "for", "while", "switch", "case", "default",
		"break", "continue", "extends", "section", "yield", "show", "parent",
		"include", "includeIf", "includeWhen", "includeUnless", "includeFirst",
		"each", "component", "slot", "push", "pushIf", "pushOnce", "stack",
		"prepend", "csrf", "method", "error", "php", "verbatim", "props",
		"aware", "once", "can", "cannot", "canany", "env", "production",
		"lang", "json", "class", "style", "checked", "selected", "disabled",
		"readonly", "required", "vite", "inject", "use", "dd", "dump", "stop",
		"overwrite", "append", "hasSection", "sectionMissing", "fragment",
		"livewire", "livewireStyles", "livewireScripts", "viteReactRefresh",
		"session", "teleport", "bool", "elseauth", "elseguest", "elsecan",
		"elsecannot", "elsecanany", "elseunless", "componentFirst", "pushElseif",
		"persist",
		nullptr
	};
	for (int i = 0; names[i]; i++)
		if (strcmp(s, names[i]) == 0)
			return true;
	return false;
}

static bool IsBladeEndDirective(const char *s) {
	if (strncmp(s, "end", 3) != 0)
		return false;
	return IsBladeDirective(s + 3) || strcmp(s + 3, "section") == 0 ||
	       strcmp(s + 3, "slot") == 0;
}

/* -------------------------------- Lex ----------------------------------- */

void LexerLaravel::PhpToken(Accessor &styler, Sci_Position &pos, Sci_Position max, bool &inPhp) {
	const int c  = styler.SafeGetCharAt(pos,     '\0');
	const int n  = styler.SafeGetCharAt(pos + 1, '\0');
	const int n2 = styler.SafeGetCharAt(pos + 2, '\0');
	const int n3 = styler.SafeGetCharAt(pos + 3, '\0');
	if (IsWhitespace(c)) {
		while (pos < max && IsWhitespace(styler.SafeGetCharAt(pos, '\0')))
			pos++;
		styler.ColourTo(pos - 1, SCE_LARAVEL_DEFAULT);
	} else if (c == '?' && n == '>') {
		pos += 2;
		styler.ColourTo(pos - 1, SCE_LARAVEL_CLOSE_TAG);
		inPhp = false;
	} else if (c == '<' && n == '?') {
		if (n2 == 'p' && n3 == 'h' &&
		    styler.SafeGetCharAt(pos + 4, '\0') == 'p') {
			pos += 5;
			styler.ColourTo(pos - 1, SCE_LARAVEL_OPEN_TAG);
		} else if (n2 == '=') {
			pos += 3;
			styler.ColourTo(pos - 1, SCE_LARAVEL_OPEN_TAG);
		} else {
			pos += 2;
			styler.ColourTo(pos - 1, SCE_LARAVEL_OPERATOR);
		}
	} else if (c == '/' && n == '/') {
		pos += 2;
		ScanLineComment(styler, pos, max);
	} else if (c == '#') {
		if (n == '[') {
			/* PHP 8 attribute - colour the whole #[...] span. */
			pos += 2;
			int depth = 1;
			while (pos < max && depth > 0) {
				const int a = styler.SafeGetCharAt(pos, '\0');
				if (a == '[') depth++;
				else if (a == ']') depth--;
				pos++;
			}
			styler.ColourTo(pos - 1, SCE_LARAVEL_ATTRIBUTE);
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
		Sci_Position mwEnd = 0;
		if (options.colourLaravel && IsMiddlewareString(styler, pos, max, mwEnd)) {
			pos = mwEnd;
			styler.ColourTo(pos - 1, SCE_LARAVEL_MIDDLEWARE);
		} else {
			pos++;
			ScanSingleQuotedString(styler, pos, max);
		}
	} else if (c == '"') {
		pos++;
		ScanDoubleQuotedString(styler, pos, max);
	} else if ((c == 'b' || c == 'B') && (n == '\'' || n == '"')) {
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
		styler.ColourTo(pos - 1, SCE_LARAVEL_VARIABLE);
	} else if (c == '<' && n == '<' && n2 == '<') {
		pos += 3;
		ScanHeredoc(styler, pos, max);
	} else if (IsIdentifierStart(c)) {
		const Sci_Position identStart = pos;

		/* ---- Laravel helpers and facades ----------------------------
		 * We must decide the style *before* consuming the identifier,
		 * because helpers are followed by "(" while ordinary
		 * identifiers are not.  We look ahead past the identifier. */
		while (pos < max && IsIdentifierContinue(styler.SafeGetCharAt(pos, '\0')))
			pos++;
		const Sci_Position identEnd = pos;

		char s[MAX_LARAVEL_IDENT_CHARS + 1];
		Sci_Position len = identEnd - identStart;
		len = len > MAX_LARAVEL_IDENT_CHARS ? MAX_LARAVEL_IDENT_CHARS : len;
		GrabString(s, styler, identStart, len);

		/* Look ahead for "(" (helper) or "::" (facade). */
		Sci_Position la = identEnd;
		while (la < max && IsWhitespace(styler.SafeGetCharAt(la, '\0')))
			la++;
		const bool nextIsParen = (la < max && styler.SafeGetCharAt(la, '\0') == '(');
		const bool nextIsDoubleColon = (la + 1 < max &&
		                               styler.SafeGetCharAt(la,     '\0') == ':' &&
		                               styler.SafeGetCharAt(la + 1, '\0') == ':');

		/* Preceding "::" means this is a static call. */
		Sci_Position pb = identStart - 1;
		while (pb >= 0 && IsWhitespace(styler.SafeGetCharAt(pb, '\0')))
			pb--;
		const bool prevIsDoubleColon = (pb >= 1 &&
		                               styler.SafeGetCharAt(pb,     '\0') == ':' &&
		                               styler.SafeGetCharAt(pb - 1, '\0') == ':');

		if (options.colourLaravel) {
			/* 1) User-provided facades win. */
			if (keywords[WL_LARAVEL_FACADES].InList(s)) {
				styler.ColourTo(identEnd - 1, SCE_LARAVEL_FACADE);
				return;
			}
			/* 2) Heuristic: PascalCase before "::" is a facade. */
			if (nextIsDoubleColon && isupper(static_cast<unsigned char>(s[0]))) {
				styler.ColourTo(identEnd - 1, SCE_LARAVEL_FACADE);
				return;
			}
			/* 3) After "::" - route method, Eloquent, middleware, etc. */
			if (prevIsDoubleColon) {
				if (keywords[WL_LARAVEL_METHODS].InList(s)) {
					styler.ColourTo(identEnd - 1, SCE_LARAVEL_ROUTE_METHOD);
					return;
				}
				/* Eloquent builder methods and anything else on the
				 * Laravel method list are also coloured here. */
				if (keywords[WL_LARAVEL_ELOQUENT].InList(s)) {
					styler.ColourTo(identEnd - 1, SCE_LARAVEL_ELOQUENT);
					return;
				}
			}
			/* 4) Helpers: known helper names called as functions. */
			if (nextIsParen && keywords[WL_LARAVEL_HELPERS].InList(s)) {
				styler.ColourTo(identEnd - 1, SCE_LARAVEL_HELPER);
				return;
			}
			/* 5) Artisan sub-commands are identifiers containing ':' */
			/*    (handled below in the identifier loop)             */
		}

		/* Fall back to the plain PHP word lists. */
		bool keyword = false;
		for (int ii = 0; ii < 4; ii++) {   /* WL_PHP_* */
			if (keywords[ii].InList(s)) {
				styler.ColourTo(identEnd - 1, SCE_LARAVEL_WORD + ii);
				keyword = true;
				break;
			}
		}
		if (!keyword) {
			/* Artisan commands contain ':' (make:model, migrate:fresh).
			 * The PHP identifier scanner stops at ':', so we have to
			 * extend it here. */
			if (pos < max && styler.SafeGetCharAt(pos, '\0') == ':') {
				const Sci_Position saved = pos;
				pos++;
				const Sci_Position subStart = pos;
				while (pos < max && IsIdentifierContinue(styler.SafeGetCharAt(pos, '\0')))
					pos++;
				if (pos > subStart) {
					/* Re-read the whole "a:b" string and test it. */
					char full[MAX_LARAVEL_IDENT_CHARS + 1];
					Sci_Position fullLen = pos - identStart;
					fullLen = fullLen > MAX_LARAVEL_IDENT_CHARS ? MAX_LARAVEL_IDENT_CHARS : fullLen;
					GrabString(full, styler, identStart, fullLen);
					if (keywords[WL_LARAVEL_ARTISAN].InList(full)) {
						styler.ColourTo(pos - 1, SCE_LARAVEL_ARTISAN);
						return;
					}
					/* Not an Artisan command - keep the sub-part
					 * as a plain identifier by re-colouring. */
					styler.ColourTo(identEnd - 1, SCE_LARAVEL_IDENTIFIER);
					styler.ColourTo(pos - 1, SCE_LARAVEL_IDENTIFIER);
					return;
				}
				pos = saved;
			}
			styler.ColourTo(identEnd - 1, SCE_LARAVEL_IDENTIFIER);
		}
	} else if (c >= '0' && c <= '9') {
		ScanNumber(styler, pos, max);
	} else if (IsOperatorChar(c)) {
		pos++;
		styler.ColourTo(pos - 1, SCE_LARAVEL_OPERATOR);
	} else {
		pos++;
		styler.ColourTo(pos - 1, SCE_LARAVEL_LEXERROR);
	}
}

void SCI_METHOD LexerLaravel::Lex(Sci_PositionU startPos, Sci_Position length, int initStyle, IDocument *pAccess) {
	PropSetSimple props;
	Accessor styler(pAccess, &props);
	Sci_Position pos = startPos;
	const Sci_Position max = pos + length;

	if (blade) {
		/* Blade templates are always lexed as one document from the start. */
		styler.StartAt(0);
		styler.StartSegment(0);
		LexBlade(styler, max);
		styler.Flush();
		return;
	}

	styler.StartAt(pos);
	styler.StartSegment(pos);

	bool inPhp = (initStyle != SCE_LARAVEL_INLINE_HTML);

	if (startPos == 0 && initStyle == SCE_LARAVEL_DEFAULT) {
		Sci_Position p = 0;
		while (p < max && IsWhitespace(styler.SafeGetCharAt(p, '\0')))
			p++;
		inPhp = !(p + 1 < max &&
		          styler.SafeGetCharAt(p,     '\0') == '<' &&
		          styler.SafeGetCharAt(p + 1, '\0') == '?');
	}

	if (initStyle == SCE_LARAVEL_COMMENT || initStyle == SCE_LARAVEL_DOCCOMMENT) {
		ScanBlockComment(styler, pos, max, initStyle == SCE_LARAVEL_DOCCOMMENT);
	} else if (initStyle == SCE_LARAVEL_COMMENTLINE) {
		ScanLineComment(styler, pos, max);
	} else if (initStyle == SCE_LARAVEL_STRING) {
		ScanSingleQuotedString(styler, pos, max);
	} else if (initStyle == SCE_LARAVEL_STRING_DQ) {
		ScanDoubleQuotedString(styler, pos, max);
	} else if (initStyle == SCE_LARAVEL_HEREDOC || initStyle == SCE_LARAVEL_NOWDOC) {
		Sci_Position idStart = 0, idLen = 0;
		bool nowdoc = (initStyle == SCE_LARAVEL_NOWDOC);
		if (FindHeredocStart(styler, pos, idStart, idLen, nowdoc)) {
			ScanHeredocBody(styler, pos, max, idStart, idLen, nowdoc);
		} else {
			styler.ColourTo(max - 1, initStyle);
			pos = max;
		}
	}

	while (pos < max) {
		const int c  = styler.SafeGetCharAt(pos,     '\0');
		const int n  = styler.SafeGetCharAt(pos + 1, '\0');
		const int n2 = styler.SafeGetCharAt(pos + 2, '\0');
		const int n3 = styler.SafeGetCharAt(pos + 3, '\0');

		if (!inPhp) {
			if (c == '<' && n == '?') {
				if (n2 == 'p' && n3 == 'h' &&
				    styler.SafeGetCharAt(pos + 4, '\0') == 'p') {
					pos += 5;
					styler.ColourTo(pos - 1, SCE_LARAVEL_OPEN_TAG);
					inPhp = true;
					continue;
				}
				if (n2 == '=') {
					pos += 3;
					styler.ColourTo(pos - 1, SCE_LARAVEL_OPEN_TAG);
					inPhp = true;
					continue;
				}
				if (n2 == ' ' || n2 == '\t' || n2 == '\n' || n2 == '\r') {
					pos += 2;
					styler.ColourTo(pos - 1, SCE_LARAVEL_OPEN_TAG);
					inPhp = true;
					continue;
				}
			}
			if (options.colourLaravel && c == '@' && IsIdentifierStart(n) &&
			    (pos == 0 || !IsIdentifierContinue(styler.SafeGetCharAt(pos - 1, '\0')))) {
				Sci_Position e = pos + 1;
				while (e < max && IsIdentifierContinue(styler.SafeGetCharAt(e, '\0')))
					e++;
				char dname[64];
				Sci_Position dl = e - (pos + 1);
				dl = dl > 63 ? 63 : dl;
				GrabString(dname, styler, pos + 1, dl);
				dname[dl] = '\0';
				const bool isEnd = IsBladeEndDirective(dname);
				if (isEnd || IsBladeDirective(dname)) {
					pos = e;
					styler.ColourTo(pos - 1, isEnd ? SCE_LARAVEL_DIRECTIVE_END : SCE_LARAVEL_DIRECTIVE);
					continue;
				}
			}
			pos++;
			while (pos < max) {
				const int h = styler.SafeGetCharAt(pos, '\0');
				if ((h == '<' && styler.SafeGetCharAt(pos + 1, '\0') == '?') || h == '@')
					break;
				pos++;
			}
			styler.ColourTo(pos - 1, SCE_LARAVEL_INLINE_HTML);
			continue;
		}

		PhpToken(styler, pos, max, inPhp);
	}

	styler.ColourTo(pos - 1, SCE_LARAVEL_DEFAULT);
	styler.Flush();
}

/* ------------------------------- Blade ---------------------------------- */

static Sci_Position FindText(Accessor &styler, Sci_Position from, Sci_Position max, const char *needle) {
	const Sci_Position nl = static_cast<Sci_Position>(strlen(needle));
	for (Sci_Position p = from; p + nl <= max; p++) {
		Sci_Position i = 0;
		while (i < nl && styler.SafeGetCharAt(p + i, '\0') == needle[i])
			i++;
		if (i == nl)
			return p;
	}
	return -1;
}

static bool MatchWordCI(Accessor &styler, Sci_Position p, Sci_Position max, const char *w) {
	const Sci_Position wl = static_cast<Sci_Position>(strlen(w));
	if (p + wl > max)
		return false;
	for (Sci_Position i = 0; i < wl; i++)
		if (tolower(static_cast<unsigned char>(styler.SafeGetCharAt(p + i, '\0'))) != w[i])
			return false;
	return true;
}

/* Index of the ')' matching the '(' at open, skipping quoted strings; -1 if none. */
static Sci_Position FindMatchingParen(Accessor &styler, Sci_Position open, Sci_Position max) {
	int depth = 0;
	for (Sci_Position p = open; p < max; p++) {
		const int ch = styler.SafeGetCharAt(p, '\0');
		if (ch == '\'' || ch == '"') {
			for (p++; p < max; p++) {
				const int q = styler.SafeGetCharAt(p, '\0');
				if (q == '\\') p++;
				else if (q == ch) break;
			}
		} else if (ch == '(') {
			depth++;
		} else if (ch == ')') {
			if (--depth == 0)
				return p;
		}
	}
	return -1;
}

/* PHP tokens in [pos, end); the region never runs past end. */
void LexerLaravel::PhpRegion(Accessor &styler, Sci_Position pos, Sci_Position end) {
	bool inPhp = true;
	while (pos < end) {
		const Sci_Position before = pos;
		PhpToken(styler, pos, end, inPhp);
		inPhp = true;           /* a stray "?>" does not end a Blade expression */
		if (pos <= before)
			pos = before + 1;
	}
}

static bool IsHtmlNameChar(int ch) {
	return IsASCII(ch) && (isalnum(ch) || ch == '-' || ch == '_' || ch == ':' || ch == '.');
}

/* Blade constructs that apply in every HTML state. Returns true if it consumed input. */
bool LexerLaravel::TryBlade(Accessor &styler, Sci_Position &pos, Sci_Position max) {
	const int c  = styler.SafeGetCharAt(pos,     '\0');
	const int n  = styler.SafeGetCharAt(pos + 1, '\0');
	const int n2 = styler.SafeGetCharAt(pos + 2, '\0');
	const int n3 = styler.SafeGetCharAt(pos + 3, '\0');

	if (c == '{' && n == '{' && n2 == '-' && n3 == '-') {
		const Sci_Position e = FindText(styler, pos + 4, max, "--}}");
		pos = e < 0 ? max : e + 4;
		styler.ColourTo(pos - 1, SCE_LARAVEL_BLADE_COMMENT);
		return true;
	}
	if (c == '@' && n == '{' && n2 == '{') {
		/* @{{ x }} is output literally for JavaScript frameworks. */
		const Sci_Position e = FindText(styler, pos + 3, max, "}}");
		pos = e < 0 ? max : e + 2;
		styler.ColourTo(pos - 1, SCE_LARAVEL_INLINE_HTML);
		return true;
	}
	if (c == '@' && n == '@') {
		pos += 2;
		styler.ColourTo(pos - 1, SCE_LARAVEL_INLINE_HTML);
		return true;
	}
	if (c == '{' && (n == '{' || (n == '!' && n2 == '!'))) {
		const bool raw = (n == '!');
		const int dl = raw ? 3 : 2;
		const Sci_Position e = FindText(styler, pos + dl, max, raw ? "!!}" : "}}");
		const Sci_Position innerEnd = e < 0 ? max : e;
		pos += dl;
		styler.ColourTo(pos - 1, SCE_LARAVEL_BLADE_DELIM);
		PhpRegion(styler, pos, innerEnd);
		pos = innerEnd;
		if (e >= 0) {
			pos = e + dl;
			styler.ColourTo(pos - 1, SCE_LARAVEL_BLADE_DELIM);
		}
		return true;
	}
	if (c == '<' && n == '?') {
		int open = 0;
		if (n2 == 'p' && n3 == 'h' && styler.SafeGetCharAt(pos + 4, '\0') == 'p') open = 5;
		else if (n2 == '=') open = 3;
		else if (n2 == ' ' || n2 == '\t' || n2 == '\n' || n2 == '\r') open = 2;
		if (!open)
			return false;
		pos += open;
		styler.ColourTo(pos - 1, SCE_LARAVEL_OPEN_TAG);
		bool inPhp = true;
		while (pos < max && inPhp) {
			const Sci_Position before = pos;
			PhpToken(styler, pos, max, inPhp);
			if (pos <= before)
				pos = before + 1;
		}
		return true;
	}
	if (c == '@' && IsIdentifierStart(n) &&
	    (pos == 0 || !IsIdentifierContinue(styler.SafeGetCharAt(pos - 1, '\0')))) {
		Sci_Position e = pos + 1;
		while (e < max && IsIdentifierContinue(styler.SafeGetCharAt(e, '\0')))
			e++;
		char dname[64];
		Sci_Position dl = e - (pos + 1);
		dl = dl > 63 ? 63 : dl;
		GrabString(dname, styler, pos + 1, dl);
		dname[dl] = '\0';
		const bool isEnd = IsBladeEndDirective(dname);
		if (!isEnd && !IsBladeDirective(dname))
			return false;
		pos = e;
		styler.ColourTo(pos - 1, isEnd ? SCE_LARAVEL_DIRECTIVE_END : SCE_LARAVEL_DIRECTIVE);
		if (isEnd)
			return true;

		Sci_Position q = pos;
		while (q < max && (styler.SafeGetCharAt(q, '\0') == ' ' || styler.SafeGetCharAt(q, '\0') == '\t'))
			q++;
		const bool hasArgs = styler.SafeGetCharAt(q, '\0') == '(';

		if (strcmp(dname, "verbatim") == 0) {
			const Sci_Position v = FindText(styler, pos, max, "@endverbatim");
			const Sci_Position stop = v < 0 ? max : v;
			if (stop > pos) {
				styler.ColourTo(stop - 1, SCE_LARAVEL_INLINE_HTML);
				pos = stop;
			}
			return true;
		}
		if (strcmp(dname, "php") == 0 && !hasArgs) {
			const Sci_Position v = FindText(styler, pos, max, "@endphp");
			const Sci_Position stop = v < 0 ? max : v;
			PhpRegion(styler, pos, stop);
			pos = stop;
			return true;
		}
		if (hasArgs) {
			const Sci_Position r = FindMatchingParen(styler, q, max);
			if (r >= 0) {
				PhpRegion(styler, pos, r + 1);
				pos = r + 1;
			}
		}
		return true;
	}
	return false;
}

/* HTML around Blade constructs. Blade is expanded on the server, so its
 * constructs are recognised inside tags, attribute values and comments too. */
void LexerLaravel::LexBlade(Accessor &styler, Sci_Position max) {
	enum { H_TEXT, H_TAG, H_COMMENT, H_DQ, H_SQ, H_RAW };
	int hs = H_TEXT;
	bool afterEq = false;
	bool closing = false;
	char tagName[16] = "";
	char rawEnd[16] = "";
	Sci_Position pos = 0;

	while (pos < max) {
		const Sci_Position before = pos;
		if (TryBlade(styler, pos, max))
			continue;
		const int c = styler.SafeGetCharAt(pos, '\0');
		const int n = styler.SafeGetCharAt(pos + 1, '\0');
		const int n2 = styler.SafeGetCharAt(pos + 2, '\0');

		switch (hs) {
		case H_TEXT:
			if (c == '<' && n == '!' && n2 == '-' && styler.SafeGetCharAt(pos + 3, '\0') == '-') {
				pos += 4;
				styler.ColourTo(pos - 1, SCE_LARAVEL_HTML_COMMENT);
				hs = H_COMMENT;
			} else if (c == '<' && n == '!') {
				while (pos < max && styler.SafeGetCharAt(pos, '\0') != '>')
					pos++;
				if (pos < max)
					pos++;
				styler.ColourTo(pos - 1, SCE_LARAVEL_HTML_TAG);
			} else if (c == '<' && (IsASCII(n) && isalpha(n) ||
			           (n == '/' && IsASCII(n2) && isalpha(n2)))) {
				closing = (n == '/');
				pos += closing ? 2 : 1;
				int k = 0;
				while (pos < max && IsHtmlNameChar(styler.SafeGetCharAt(pos, '\0'))) {
					if (k < 15)
						tagName[k++] = static_cast<char>(tolower(static_cast<unsigned char>(styler.SafeGetCharAt(pos, '\0'))));
					pos++;
				}
				tagName[k] = '\0';
				styler.ColourTo(pos - 1, SCE_LARAVEL_HTML_TAG);
				hs = H_TAG;
				afterEq = false;
			} else if (c == '&') {
				Sci_Position e = pos + 1;
				while (e < max && e - pos < 12 && (isalnum(static_cast<unsigned char>(styler.SafeGetCharAt(e, '\0'))) ||
				       styler.SafeGetCharAt(e, '\0') == '#'))
					e++;
				if (e > pos + 1 && styler.SafeGetCharAt(e, '\0') == ';') {
					pos = e + 1;
					styler.ColourTo(pos - 1, SCE_LARAVEL_HTML_ENTITY);
				} else {
					pos++;
					styler.ColourTo(pos - 1, SCE_LARAVEL_INLINE_HTML);
				}
			} else {
				pos++;
				while (pos < max) {
					const int h = styler.SafeGetCharAt(pos, '\0');
					if (h == '<' || h == '{' || h == '@' || h == '&')
						break;
					pos++;
				}
				styler.ColourTo(pos - 1, SCE_LARAVEL_INLINE_HTML);
			}
			break;

		case H_COMMENT:
			if (c == '-' && n == '-' && n2 == '>') {
				pos += 3;
				styler.ColourTo(pos - 1, SCE_LARAVEL_HTML_COMMENT);
				hs = H_TEXT;
			} else {
				pos++;
				while (pos < max) {
					const int h = styler.SafeGetCharAt(pos, '\0');
					if (h == '-' || h == '{' || h == '@' || h == '<')
						break;
					pos++;
				}
				styler.ColourTo(pos - 1, SCE_LARAVEL_HTML_COMMENT);
			}
			break;

		case H_RAW:
			if (c == '<' && n == '/' && MatchWordCI(styler, pos, max, rawEnd)) {
				hs = H_TEXT;
			} else {
				pos++;
				while (pos < max) {
					const int h = styler.SafeGetCharAt(pos, '\0');
					if (h == '<' || h == '{' || h == '@')
						break;
					pos++;
				}
				styler.ColourTo(pos - 1, SCE_LARAVEL_INLINE_HTML);
			}
			break;

		case H_DQ:
		case H_SQ: {
			const int quote = (hs == H_DQ) ? '"' : '\'';
			if (c == quote) {
				pos++;
				hs = H_TAG;
			} else {
				pos++;
				while (pos < max) {
					const int h = styler.SafeGetCharAt(pos, '\0');
					if (h == quote || h == '{' || h == '@' || h == '<')
						break;
					pos++;
				}
			}
			styler.ColourTo(pos - 1, SCE_LARAVEL_HTML_VALUE);
			break;
		}

		case H_TAG:
		default:
			if (IsWhitespace(c)) {
				while (pos < max && IsWhitespace(styler.SafeGetCharAt(pos, '\0')))
					pos++;
				styler.ColourTo(pos - 1, SCE_LARAVEL_DEFAULT);
			} else if (c == '>') {
				pos++;
				styler.ColourTo(pos - 1, SCE_LARAVEL_HTML_TAG);
				hs = H_TEXT;
				if (!closing && (strcmp(tagName, "script") == 0 || strcmp(tagName, "style") == 0)) {
					hs = H_RAW;
					snprintf(rawEnd, sizeof(rawEnd), "</%s", tagName);
				}
			} else if (c == '/' && n == '>') {
				pos += 2;
				styler.ColourTo(pos - 1, SCE_LARAVEL_HTML_TAG);
				hs = H_TEXT;
			} else if (c == '/') {
				pos++;
				styler.ColourTo(pos - 1, SCE_LARAVEL_HTML_TAG);
			} else if (c == '=') {
				pos++;
				styler.ColourTo(pos - 1, SCE_LARAVEL_OPERATOR);
				afterEq = true;
			} else if (c == '"' || c == '\'') {
				pos++;
				styler.ColourTo(pos - 1, SCE_LARAVEL_HTML_VALUE);
				hs = (c == '"') ? H_DQ : H_SQ;
				afterEq = false;
			} else if (c == '<') {
				hs = H_TEXT;      /* unterminated tag: let the text state restart */
				afterEq = false;
			} else {
				pos++;
				while (pos < max) {
					const int h = styler.SafeGetCharAt(pos, '\0');
					if (IsWhitespace(h) || h == '>' || h == '"' || h == '\'' || h == '<' ||
					    (h == '=' && !afterEq) || (h == '/' && !afterEq) ||
					    (h == '{' && (styler.SafeGetCharAt(pos + 1, '\0') == '{' ||
					                  styler.SafeGetCharAt(pos + 1, '\0') == '!')))
						break;
					pos++;
				}
				styler.ColourTo(pos - 1, afterEq ? SCE_LARAVEL_HTML_VALUE : SCE_LARAVEL_HTML_ATTR);
				afterEq = false;
			}
			break;
		}
	}
	styler.ColourTo(max - 1, SCE_LARAVEL_DEFAULT);
}

/* -------------------------------- Fold ---------------------------------- */

void SCI_METHOD LexerLaravel::Fold(Sci_PositionU startPos, Sci_Position length, int initStyle, IDocument *pAccess) {
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

		if (style == SCE_LARAVEL_COMMENTLINE)
			inLineComment = true;

		if (options.foldComment && options.foldCommentMultiline &&
		    IsStreamCommentStyle(style) && !inLineComment) {
			if (!IsStreamCommentStyle(stylePrev)) {
				levelNext++;
			} else if (!IsStreamCommentStyle(styleNext) && !atEOL) {
				levelNext--;
			}
		}

		if (options.foldSyntaxBased && style == SCE_LARAVEL_OPERATOR) {
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

extern const LexerModule lmLaravel(SCLEX_LARAVEL, LexerLaravel::LexerFactoryLaravel, "laravel", laravelWordLists);
extern const LexerModule lmBlade(SCLEX_BLADE, LexerLaravel::LexerFactoryBlade, "blade", laravelWordLists);
