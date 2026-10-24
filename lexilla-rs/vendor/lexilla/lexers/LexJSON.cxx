// Scintilla source code edit control
/**
 * @file LexJSON.cxx
 * @date February 19, 2016
 * @brief Lexer for JSON and JSON-LD formats
 * @author nkmathew
 *
 * The License.txt file describes the conditions under which this software may
 * be distributed.
 *
 */

#include <cstdlib>
#include <cassert>
#include <cctype>
#include <cstdio>

#include <string>
#include <string_view>
#include <vector>
#include <map>
#include <functional>

#include "ILexer.h"
#include "Scintilla.h"
#include "SciLexer.h"
#include "WordList.h"
#include "LexAccessor.h"
#include "StyleContext.h"
#include "CharacterSet.h"
#include "CharacterCategory.h"
#include "LexerModule.h"
#include "OptionSet.h"
#include "DefaultLexer.h"

using namespace Scintilla;
using namespace Lexilla;

namespace {

const char *const JSONWordListDesc[] = {
	"JSON Keywords",
	"JSON-LD Keywords",
	nullptr
};

/**
 * Used to detect compact IRI/URLs in JSON-LD without first looking ahead for the
 * colon separating the prefix and suffix
 *
 * https://www.w3.org/TR/json-ld/#dfn-compact-iri
 */
struct CompactIRI {
	int colonCount;
	bool foundInvalidChar;
	CharacterSet setCompactIRI;
	CompactIRI() {
		colonCount = 0;
		foundInvalidChar = false;
		setCompactIRI = CharacterSet(CharacterSet::setAlpha, "$_-");
	}
	void resetState() noexcept {
		colonCount = 0;
		foundInvalidChar = false;
	}
	void checkChar(int ch) noexcept {
		if (ch == ':') {
			colonCount++;
		} else {
			foundInvalidChar |= !setCompactIRI.Contains(ch);
		}
	}
	[[nodiscard]] bool shouldHighlight() const noexcept {
		return !foundInvalidChar && colonCount == 1;
	}
};

/**
 * Keeps track of escaped characters in strings as per:
 *
 * https://tools.ietf.org/html/rfc7159#section-7
 */
struct EscapeSequence {
	int digitsLeft;
	CharacterSet setHexDigits;
	CharacterSet setEscapeChars;
	EscapeSequence() {
		digitsLeft = 0;
		setHexDigits = CharacterSet(CharacterSet::setDigits, "ABCDEFabcdef");
		setEscapeChars = CharacterSet(CharacterSet::setNone, "\\\"tnbfru/");
	}
	// Returns true if the following character is a valid escaped character
	bool newSequence(int nextChar) noexcept {
		digitsLeft = 0;
		if (nextChar == 'u') {
			digitsLeft = 5;
		} else if (!setEscapeChars.Contains(nextChar)) {
			return false;
		}
		return true;
	}
	[[nodiscard]] bool atEscapeEnd() const noexcept {
		return digitsLeft <= 0;
	}
	[[nodiscard]] bool isInvalidChar(int currChar) const noexcept {
		return !setHexDigits.Contains(currChar);
	}
};

struct OptionsJSON {
	bool foldCompact;
	bool fold;
	bool allowComments;
	bool escapeSequence;
	bool json5;
	OptionsJSON() {
		foldCompact = false;
		fold = false;
		allowComments = false;
		escapeSequence = false;
		json5 = false;
	}
};

struct OptionSetJSON : public OptionSet<OptionsJSON> {
	OptionSetJSON() {
		DefineProperty("lexer.json.escape.sequence", &OptionsJSON::escapeSequence,
					   "Set to 1 to enable highlighting of escape sequences in strings");

		DefineProperty("lexer.json.json5", &OptionsJSON::json5,
			"Set to 1 to enable JSON5 syntax (leaves strict JSON unchanged)");

		DefineProperty("lexer.json.allow.comments", &OptionsJSON::allowComments,
					   "Set to 1 to enable highlighting of line/block comments in JSON");

		DefineProperty("fold.compact", &OptionsJSON::foldCompact);
		DefineProperty("fold", &OptionsJSON::fold);
		DefineWordListSets(JSONWordListDesc);
	}
};

const LexicalClass jsonLexicalClasses[] = {
	{0, "SCE_JSON_DEFAULT", "default", "Whitespace and punctuation"},
	{1, "SCE_JSON_NUMBER", "literal numeric", "Numbers"},
	{2, "SCE_JSON_STRING", "literal string", "Strings"},
	{3, "SCE_JSON_STRINGEOL", "error literal string", "Unterminated strings"},
	{4, "SCE_JSON_PROPERTYNAME", "property", "Property names"},
	{5, "SCE_JSON_ESCAPESEQUENCE", "literal string", "Escape sequences"},
	{6, "SCE_JSON_LINECOMMENT", "comment", "Line comments"},
	{7, "SCE_JSON_BLOCKCOMMENT", "comment", "Block comments"},
	{8, "SCE_JSON_OPERATOR", "operator", "Operators"},
	{9, "SCE_JSON_URI", "literal string", "URIs"},
	{10, "SCE_JSON_COMPACTIRI", "literal string", "JSON-LD compact IRIs"},
	{11, "SCE_JSON_KEYWORD", "keyword", "true, false, null"},
	{12, "SCE_JSON_LDKEYWORD", "keyword", "JSON-LD keywords"},
	{13, "SCE_JSON_ERROR", "error", "Invalid input"},
};

class LexerJSON : public DefaultLexer {
	OptionsJSON options;
	OptionSetJSON optSetJSON;
	EscapeSequence escapeSeq;
	WordList keywordsJSON;
	WordList keywordsJSONLD;
	CharacterSet setOperators;
	CharacterSet setURL;
	CharacterSet setKeywordJSONLD;
	CharacterSet setKeywordJSON;
	CompactIRI compactIRI;

	static bool IsNextNonWhitespace(LexAccessor &styler, Sci_Position start, char ch) {
		Sci_Position i = 0;
		while (i < 50) {
			i++;
			const char curr = styler.SafeGetCharAt(start+i, '\0');
			const char next = styler.SafeGetCharAt(start+i+1, '\0');
			const bool atEOL = (curr == '\r' && next != '\n') || (curr == '\n');
			if (curr == ch) {
				return true;
			} else if (!isspacechar(curr) || atEOL) {
				return false;
			}
		}
		return false;
	}

	/**
	 * Looks for the colon following the end quote
	 *
	 * Assumes property names of lengths no longer than a 100 characters.
	 * The colon is also expected to be less than 50 spaces after the end
	 * quote for the string to be considered a property name
	 */
	static bool AtPropertyName(LexAccessor &styler, Sci_Position start) {
		Sci_Position i = 0;
		bool escaped = false;
		while (i < 100) {
			i++;
			const char curr = styler.SafeGetCharAt(start+i, '\0');
			if (escaped) {
				escaped = false;
				continue;
			}
			escaped = curr == '\\';
			if (curr == '"') {
				return IsNextNonWhitespace(styler, start+i, ':');
			} else if (!curr) {
				return false;
			}
		}
		return false;
	}

	static bool IsNextWordInList(const WordList &keywordList, const CharacterSet &wordSet,
								 const StyleContext &context, LexAccessor &styler) {
		char word[51];
		const Sci_Position currPos = static_cast<Sci_Position>(context.currentPos);
		int i = 0;
		while (i < 50) {
			const char ch = styler.SafeGetCharAt(currPos + i);
			if (!wordSet.Contains(ch)) {
				break;
			}
			word[i] = ch;
			i++;
		}
		word[i] = '\0';
		return keywordList.InList(word);
	}

	public:

	LexerJSON() :
		DefaultLexer("json", SCLEX_JSON, jsonLexicalClasses, std::size(jsonLexicalClasses)),
		setOperators(CharacterSet::setNone, "[{}]:,"),
		setURL(CharacterSet::setAlphaNum, "-._~:/?#[]@!$&'()*+,),="),
		setKeywordJSONLD(CharacterSet::setAlpha, ":@"),
		setKeywordJSON(CharacterSet::setAlpha, "$_") {
	}
	virtual ~LexerJSON() = default;
	int SCI_METHOD Version() const override {
		return lvRelease5;
	}
	void SCI_METHOD Release() override {
		delete this;
	}
	const char *SCI_METHOD PropertyNames() override {
		return optSetJSON.PropertyNames();
	}
	int SCI_METHOD PropertyType(const char *name) override {
		return optSetJSON.PropertyType(name);
	}
	const char *SCI_METHOD DescribeProperty(const char *name) override {
		return optSetJSON.DescribeProperty(name);
	}
	Sci_Position SCI_METHOD PropertySet(const char *key, const char *val) override {
		if (optSetJSON.PropertySet(&options, key, val)) {
			return 0;
		}
		return -1;
	}
	const char * SCI_METHOD PropertyGet(const char *key) override {
		return optSetJSON.PropertyGet(key);
	}
	Sci_Position SCI_METHOD WordListSet(int n, const char *wl) override {
		WordList *wordListN = nullptr;
		switch (n) {
			case 0:
				wordListN = &keywordsJSON;
				break;
			case 1:
				wordListN = &keywordsJSONLD;
				break;
		}
		Sci_Position firstModification = -1;
		if (wordListN) {
			if (wordListN->Set(wl)) {
				firstModification = 0;
			}
		}
		return firstModification;
	}
	void *SCI_METHOD PrivateCall(int, void *) override {
		return nullptr;
	}
	static ILexer5 *LexerFactoryJSON() {
		return new LexerJSON;
	}
	const char *SCI_METHOD DescribeWordListSets() override {
		return optSetJSON.DescribeWordListSets();
	}
	void SCI_METHOD Lex(Sci_PositionU startPos,
								Sci_Position length,
								int initStyle,
								IDocument *pAccess) override;
	void SCI_METHOD Fold(Sci_PositionU startPos,
								 Sci_Position length,
								 int initStyle,
								 IDocument *pAccess) override;
};

// JSON5-only additions to the vendored Unicode 16 categories.
// Generated from json5/json5 lib/unicode.js (Unicode 18), commit
// 1ecc0125a053862df78ade515f72488d770f57ec; MIT license.
static bool Json5ExtraIdStart(int c) noexcept {
	static constexpr int ranges[][2] = {
		{0x558, 0x558},
		{0x58b, 0x58c},
		{0x88f, 0x88f},
		{0xc5c, 0xc5c},
		{0xcdc, 0xcdc},
		{0x208f, 0x208f},
		{0x209d, 0x209f},
		{0xa7ce, 0xa7cf},
		{0xa7d2, 0xa7d2},
		{0xa7d4, 0xa7d4},
		{0xa7dd, 0xa7dd},
		{0xa7e2, 0xa7e2},
		{0xa7f1, 0xa7f1},
		{0xab6c, 0xab6d},
		{0x107bb, 0x107bf},
		{0x10940, 0x10959},
		{0x10ec5, 0x10ec7},
		{0x10ed9, 0x10eee},
		{0x11b0a, 0x11b0a},
		{0x11db0, 0x11ddb},
		{0x11df1, 0x11df1},
		{0x1246f, 0x1246f},
		{0x12475, 0x1247f},
		{0x12550, 0x12686},
		{0x16ea0, 0x16eb8},
		{0x16ebb, 0x16ed3},
		{0x16ff2, 0x16ff6},
		{0x187f8, 0x187ff},
		{0x18cd6, 0x18cda},
		{0x18d09, 0x18d20},
		{0x18d80, 0x18df2},
		{0x18e00, 0x19191},
		{0x191a0, 0x191d2},
		{0x1b123, 0x1b128},
		{0x1b168, 0x1b168},
		{0x1d6a6, 0x1d6a6},
		{0x1df1f, 0x1df24},
		{0x1df2b, 0x1df81},
		{0x1df90, 0x1df96},
		{0x1dfcd, 0x1dfff},
		{0x1e6c0, 0x1e6de},
		{0x1e6e0, 0x1e6e2},
		{0x1e6e4, 0x1e6e5},
		{0x1e6e7, 0x1e6ed},
		{0x1e6f0, 0x1e6f4},
		{0x1e6fe, 0x1e6ff},
		{0x2b73a, 0x2b73f},
		{0x2b81e, 0x2b81e},
		{0x2cea2, 0x2cead},
		{0x323b0, 0x33479},
		{0x3d000, 0x3fc3f},
	};
	for (const auto &range : ranges) {
		if (c < range[0]) return false;
		if (c <= range[1]) return true;
	}
	return false;
}

static bool Json5ExtraIdContinue(int c) noexcept {
	static constexpr int ranges[][2] = {
		{0x558, 0x558},
		{0x58b, 0x58c},
		{0x5c8, 0x5c9},
		{0x88f, 0x88f},
		{0xb53, 0xb54},
		{0xc5c, 0xc5c},
		{0xcdc, 0xcdc},
		{0x1acf, 0x1af0},
		{0x208f, 0x208f},
		{0x209d, 0x209f},
		{0xa7ce, 0xa7cf},
		{0xa7d2, 0xa7d2},
		{0xa7d4, 0xa7d4},
		{0xa7dd, 0xa7dd},
		{0xa7e2, 0xa7e2},
		{0xa7f1, 0xa7f1},
		{0xab6c, 0xab6d},
		{0x107bb, 0x107bf},
		{0x10940, 0x10959},
		{0x10ec5, 0x10ec7},
		{0x10ecb, 0x10ecf},
		{0x10ed9, 0x10eee},
		{0x10ef0, 0x10efb},
		{0x11b0a, 0x11b0a},
		{0x11b60, 0x11b67},
		{0x11db0, 0x11ddb},
		{0x11de0, 0x11de9},
		{0x11df0, 0x11df1},
		{0x1246f, 0x1246f},
		{0x12475, 0x1247f},
		{0x12550, 0x12686},
		{0x16ea0, 0x16eb8},
		{0x16ebb, 0x16ed3},
		{0x16ff2, 0x16ff6},
		{0x187f8, 0x187ff},
		{0x18cd6, 0x18cda},
		{0x18d09, 0x18d20},
		{0x18d80, 0x18df2},
		{0x18e00, 0x19191},
		{0x191a0, 0x191d2},
		{0x1b123, 0x1b128},
		{0x1b168, 0x1b168},
		{0x1d127, 0x1d128},
		{0x1d250, 0x1d252},
		{0x1d25b, 0x1d25c},
		{0x1d25f, 0x1d25f},
		{0x1d280, 0x1d281},
		{0x1d6a6, 0x1d6a6},
		{0x1df1f, 0x1df24},
		{0x1df2b, 0x1df81},
		{0x1df90, 0x1df96},
		{0x1dfcd, 0x1dfff},
		{0x1e6c0, 0x1e6de},
		{0x1e6e0, 0x1e6f5},
		{0x1e6fe, 0x1e6ff},
		{0x2b73a, 0x2b73f},
		{0x2b81e, 0x2b81e},
		{0x2cea2, 0x2cead},
		{0x323b0, 0x33479},
		{0x3d000, 0x3fc3f},
	};
	for (const auto &range : ranges) {
		if (c < range[0]) return false;
		if (c <= range[1]) return true;
	}
	return false;
}

// JSON5 is opt-in: keep the upstream JSON/JSON-LD state machine unchanged.
// Rescan the document so quote and identifier context is also correct when a
// client asks for highlighting starting in the middle of a continued string.
void LexJSON5(LexAccessor &styler, IDocument *document) {
	const Sci_Position end = styler.Length();
	const auto ch = [&](Sci_Position p) { return styler.SafeGetCharAt(p, '\0'); };
	const auto hex = [](int c) {
		if (c >= '0' && c <= '9') return c - '0';
		if (c >= 'a' && c <= 'f') return c - 'a' + 10;
		if (c >= 'A' && c <= 'F') return c - 'A' + 10;
		return -1;
	};
	const auto digit = [](int c) { return c >= '0' && c <= '9'; };
	const auto white = [](int c) {
		return (c >= 9 && c <= 13) || c == 32 || c == 0x2028 ||
			c == 0x2029 || c == 0xfeff || CategoriseCharacter(c) == ccZs;
	};
	const auto idStart = [](int c) {
		const CharacterCategory category = CategoriseCharacter(c);
		return c == '$' || c == '_' || Json5ExtraIdStart(c) ||
			category == ccLu || category == ccLl ||
			category == ccLt || category == ccLm || category == ccLo || category == ccNl;
	};
	const auto idContinue = [&](int c) {
		const CharacterCategory category = CategoriseCharacter(c);
		return idStart(c) || category == ccMn || category == ccMc ||
			category == ccNd || category == ccPc || c == 0x200c || c == 0x200d ||
			Json5ExtraIdContinue(c);
	};
	const auto codePoint = [&](Sci_Position p, Sci_Position &width) {
		return document->GetCharacterAndWidth(p, &width);
	};
	const auto wordAt = [&](Sci_Position p, std::string_view word) {
		for (char c : word) {
			if (p >= end || ch(p++) != c) return false;
		}
		return true;
	};
	// Whitespace and comments may occur between an identifier and its colon.
	const auto skipTrivia = [&](Sci_Position p) {
		while (p < end) {
			Sci_Position width = 1;
			const int c = codePoint(p, width);
			if (white(c)) {
				p += width;
			} else if (ch(p) == '/' && ch(p + 1) == '*') {
				p += 2;
				while (p < end && !(ch(p) == '*' && ch(p + 1) == '/')) p++;
				if (p < end) p += 2;
			} else if (ch(p) == '/' && ch(p + 1) == '/') {
				p += 2;
				while (p < end) {
					const int next = codePoint(p, width);
					if (next == '\r' || next == '\n' || next == 0x2028 || next == 0x2029) break;
					p += width;
				}
			} else break;
		}
		return p;
	};
	styler.StartAt(0);
	styler.StartSegment(0);
	for (Sci_Position p = 0; p < end;) {
		const Sci_Position start = p;
		Sci_Position width = 1;
		const int c = codePoint(p, width);
		int style = SCE_JSON_ERROR;
		if (white(c)) {
			p += width;
			style = SCE_JSON_DEFAULT;
		} else if (c == '/' && ch(p + 1) == '/') {
			p += 2;
			while (p < end) {
				const int next = codePoint(p, width);
				if (next == '\r' || next == '\n' || next == 0x2028 || next == 0x2029) break;
				p += width;
			}
			style = SCE_JSON_LINECOMMENT;
		} else if (c == '/' && ch(p + 1) == '*') {
			p += 2;
			while (p < end && !(ch(p) == '*' && ch(p + 1) == '/')) p++;
			if (p < end) p += 2;
			style = SCE_JSON_BLOCKCOMMENT;
		} else if (c == '"' || c == '\'') {
			p++;
			bool closed = false;
			bool valid = true;
			while (p < end) {
				const int next = codePoint(p, width);
				if (next == c) { p++; closed = true; break; }
				if (next == '\r' || next == '\n') break;
				if (next != '\\') { p += width; continue; }
				p++;
				if (p >= end) break;
				const int escaped = codePoint(p, width);
				p += width;
				if (escaped == '\r' && ch(p) == '\n') p++;
				if (escaped == 'x' || escaped == 'u') {
					const int count = escaped == 'x' ? 2 : 4;
					for (int i = 0; i < count; i++) {
						if (p >= end || hex(ch(p)) < 0) { valid = false; break; }
						p++;
					}
				} else if (digit(escaped) && (escaped != '0' || digit(ch(p)))) {
					valid = false; // JSON5 forbids legacy octal escapes.
				}
			}
			style = closed && valid ? (ch(skipTrivia(p)) == ':' ?
				SCE_JSON_PROPERTYNAME : SCE_JSON_STRING) : SCE_JSON_STRINGEOL;
		} else if (idStart(c) || c == '\\') {
			bool first = true;
			bool valid = true;
			std::string word;
			while (p < end) {
				int next = codePoint(p, width);
				Sci_Position consumed = width;
				if (next == '\\' && ch(p + 1) == 'u') {
					next = 0;
					consumed = 6;
					for (int i = 2; i < 6; i++) {
						const int h = hex(ch(p + i));
						if (p + i >= end || h < 0) { valid = false; break; }
						next = next * 16 + h;
					}
					if (!valid) break;
				}
				if (!(first ? idStart(next) : idContinue(next))) break;
				// Decoded ASCII is enough to recognize literal words.
				word += next < 128 ? static_cast<char>(next) : '?';
				p += consumed;
				first = false;
			}
			if (p == start) p += width;
			if (valid && !first) {
				if (ch(skipTrivia(p)) == ':') style = SCE_JSON_PROPERTYNAME;
				else if (word == "true" || word == "false" || word == "null") style = SCE_JSON_KEYWORD;
				else if (word == "Infinity" || word == "NaN") style = SCE_JSON_NUMBER;
			}
		} else if (digit(c) || c == '+' || c == '-' || c == '.') {
			if (c == '+' || c == '-') p++;
			bool valid = false;
			if (wordAt(p, "Infinity")) { p += 8; valid = true; }
			else if (wordAt(p, "NaN")) { p += 3; valid = true; }
			else if (ch(p) == '0' && (ch(p + 1) == 'x' || ch(p + 1) == 'X')) {
				p += 2;
				const Sci_Position digits = p;
				while (p < end && hex(ch(p)) >= 0) p++;
				valid = p > digits;
			} else {
				const Sci_Position digits = p;
				while (p < end && digit(ch(p))) p++;
				valid = p > digits;
				if (ch(p) == '.') {
					p++;
					const Sci_Position fraction = p;
					while (p < end && digit(ch(p))) p++;
					valid = valid || p > fraction;
				}
				if (ch(p) == 'e' || ch(p) == 'E') {
					p++;
					if (ch(p) == '+' || ch(p) == '-') p++;
					const Sci_Position exponent = p;
					while (p < end && digit(ch(p))) p++;
					valid = valid && p > exponent;
				}
			}
			if (p == start) p++;
			style = valid ? SCE_JSON_NUMBER : SCE_JSON_ERROR;
		} else {
			p += width;
			if (c == '{' || c == '}' || c == '[' || c == ']' || c == ':' || c == ',')
				style = SCE_JSON_OPERATOR;
		}
		styler.ColourTo(p - 1, style);
	}
	styler.Flush();
}

void SCI_METHOD LexerJSON::Lex(Sci_PositionU startPos,
							   Sci_Position length,
							   int initStyle,
							   IDocument *pAccess) {
	LexAccessor styler(pAccess);
	if (options.json5) {
		LexJSON5(styler, pAccess);
		return;
	}
	StyleContext context(startPos, length, initStyle, styler);
	int stringStyleBefore = SCE_JSON_STRING;
	while (context.More()) {
		switch (context.state) {
			case SCE_JSON_BLOCKCOMMENT:
				if (context.Match("*/")) {
					context.Forward();
					context.ForwardSetState(SCE_JSON_DEFAULT);
				}
				break;
			case SCE_JSON_LINECOMMENT:
				if (context.MatchLineEnd()) {
					context.SetState(SCE_JSON_DEFAULT);
				}
				break;
			case SCE_JSON_STRINGEOL:
				if (context.atLineStart) {
					context.SetState(SCE_JSON_DEFAULT);
				}
				break;
			case SCE_JSON_ESCAPESEQUENCE:
				escapeSeq.digitsLeft--;
				if (!escapeSeq.atEscapeEnd()) {
					if (escapeSeq.isInvalidChar(context.ch)) {
						context.SetState(SCE_JSON_ERROR);
					}
					break;
				}
				if (context.ch == '"') {
					context.SetState(stringStyleBefore);
					context.ForwardSetState(SCE_JSON_DEFAULT);
				} else if (context.ch == '\\') {
					if (!escapeSeq.newSequence(context.chNext)) {
						context.SetState(SCE_JSON_ERROR);
					}
					context.Forward();
				} else {
					context.SetState(stringStyleBefore);
					if (context.atLineEnd) {
						context.ChangeState(SCE_JSON_STRINGEOL);
					}
				}
				break;
			case SCE_JSON_PROPERTYNAME:
			case SCE_JSON_STRING:
				if (context.ch == '"') {
					if (compactIRI.shouldHighlight()) {
						context.ChangeState(SCE_JSON_COMPACTIRI);
						context.ForwardSetState(SCE_JSON_DEFAULT);
						compactIRI.resetState();
					} else {
						context.ForwardSetState(SCE_JSON_DEFAULT);
					}
				} else if (context.atLineEnd) {
					context.ChangeState(SCE_JSON_STRINGEOL);
				} else if (context.ch == '\\') {
					stringStyleBefore = context.state;
					if (options.escapeSequence) {
						context.SetState(SCE_JSON_ESCAPESEQUENCE);
						if (!escapeSeq.newSequence(context.chNext)) {
							context.SetState(SCE_JSON_ERROR);
						}
					}
					context.Forward();
				} else if (context.Match("https://") ||
						   context.Match("http://") ||
						   context.Match("ssh://") ||
						   context.Match("git://") ||
						   context.Match("svn://") ||
						   context.Match("ftp://") ||
						   context.Match("mailto:")) {
					// Handle most common URI schemes only
					stringStyleBefore = context.state;
					context.SetState(SCE_JSON_URI);
				} else if (context.ch == '@') {
					// https://www.w3.org/TR/json-ld/#dfn-keyword
					if (IsNextWordInList(keywordsJSONLD, setKeywordJSONLD, context, styler)) {
						stringStyleBefore = context.state;
						context.SetState(SCE_JSON_LDKEYWORD);
					}
				} else {
					compactIRI.checkChar(context.ch);
				}
				break;
			case SCE_JSON_LDKEYWORD:
			case SCE_JSON_URI:
				if ((!setKeywordJSONLD.Contains(context.ch) &&
					 (context.state == SCE_JSON_LDKEYWORD)) ||
					(!setURL.Contains(context.ch))) {
					context.SetState(stringStyleBefore);
				}
				if (context.ch == '"') {
					context.ForwardSetState(SCE_JSON_DEFAULT);
				} else if (context.ch == '\\') {
					if (options.escapeSequence) {
						context.SetState(SCE_JSON_ESCAPESEQUENCE);
						if (!escapeSeq.newSequence(context.chNext)) {
							context.SetState(SCE_JSON_ERROR);
						}
					}
					context.Forward();
				} else if (context.atLineEnd) {
					context.ChangeState(SCE_JSON_STRINGEOL);
				}
				break;
			case SCE_JSON_OPERATOR:
			case SCE_JSON_NUMBER:
				context.SetState(SCE_JSON_DEFAULT);
				break;
			case SCE_JSON_ERROR:
				if (context.MatchLineEnd()) {
					context.SetState(SCE_JSON_DEFAULT);
				}
				break;
			case SCE_JSON_KEYWORD:
				if (!setKeywordJSON.Contains(context.ch)) {
					context.SetState(SCE_JSON_DEFAULT);
				}
				break;
		}
		if (context.state == SCE_JSON_DEFAULT) {
			if (context.ch == '"') {
				compactIRI.resetState();
				context.SetState(SCE_JSON_STRING);
				const Sci_Position currPos = static_cast<Sci_Position>(context.currentPos);
				if (AtPropertyName(styler, currPos)) {
					context.SetState(SCE_JSON_PROPERTYNAME);
				}
			} else if (setOperators.Contains(context.ch)) {
				context.SetState(SCE_JSON_OPERATOR);
			} else if (options.allowComments && context.Match("/*")) {
				context.SetState(SCE_JSON_BLOCKCOMMENT);
				context.Forward();
			} else if (options.allowComments && context.Match("//")) {
				context.SetState(SCE_JSON_LINECOMMENT);
			} else if (setKeywordJSON.Contains(context.ch)) {
				if (IsNextWordInList(keywordsJSON, setKeywordJSON, context, styler)) {
					context.SetState(SCE_JSON_KEYWORD);
				}
			}
			const bool numberStart =
				IsADigit(context.ch) && (context.chPrev == '+'||
										 context.chPrev == '-' ||
										 context.atLineStart ||
										 IsASpace(context.chPrev) ||
										 setOperators.Contains(context.chPrev));
			const bool exponentPart =
				tolower(context.ch) == 'e' &&
				IsADigit(context.chPrev) &&
				(IsADigit(context.chNext) ||
				 context.chNext == '+' ||
				 context.chNext == '-');
			const bool signPart =
				(context.ch == '-' || context.ch == '+') &&
				((tolower(context.chPrev) == 'e' && IsADigit(context.chNext)) ||
				 ((IsASpace(context.chPrev) || setOperators.Contains(context.chPrev))
				  && IsADigit(context.chNext)));
			const bool adjacentDigit =
				IsADigit(context.ch) && IsADigit(context.chPrev);
			const bool afterExponent = IsADigit(context.ch) && tolower(context.chPrev) == 'e';
			const bool dotPart = context.ch == '.' &&
				IsADigit(context.chPrev) &&
				IsADigit(context.chNext);
			const bool afterDot = IsADigit(context.ch) && context.chPrev == '.';
			if (numberStart ||
				exponentPart ||
				signPart ||
				adjacentDigit ||
				dotPart ||
				afterExponent ||
				afterDot) {
				context.SetState(SCE_JSON_NUMBER);
			} else if (context.state == SCE_JSON_DEFAULT && !IsASpace(context.ch)) {
				context.SetState(SCE_JSON_ERROR);
			}
		}
		context.Forward();
	}
	context.Complete();
}

void SCI_METHOD LexerJSON::Fold(Sci_PositionU startPos,
								Sci_Position length,
								int,
								IDocument *pAccess) {
	if (!options.fold) {
		return;
	}
	LexAccessor styler(pAccess);
	Sci_PositionU currLine = styler.GetLine(startPos);
	const Sci_PositionU endPos = startPos + length;
	int currLevel = SC_FOLDLEVELBASE;
	if (currLine > 0)
		currLevel = styler.LevelAt(currLine - 1) >> 16;
	int nextLevel = currLevel;
	int visibleChars = 0;
	for (Sci_PositionU i = startPos; i < endPos; i++) {
		const char curr = styler.SafeGetCharAt(i);
		const char next = styler.SafeGetCharAt(i+1);
		const bool atEOL = (curr == '\r' && next != '\n') || (curr == '\n');
		if (styler.StyleAt(i) == SCE_JSON_OPERATOR) {
			if (curr == '{' || curr == '[') {
				nextLevel++;
			} else if (curr == '}' || curr == ']') {
				nextLevel--;
			}
		}
		if (atEOL || i == (endPos-1)) {
			int level = currLevel | nextLevel << 16;
			if (!visibleChars && options.foldCompact) {
				level |= SC_FOLDLEVELWHITEFLAG;
			} else if (nextLevel > currLevel) {
				level |= SC_FOLDLEVELHEADERFLAG;
			}
			if (level != styler.LevelAt(currLine)) {
				styler.SetLevel(currLine, level);
			}
			currLine++;
			currLevel = nextLevel;
			visibleChars = 0;
		}
		if (!isspacechar(curr)) {
			visibleChars++;
		}
	}
}

}

extern const LexerModule lmJSON(SCLEX_JSON,
				   LexerJSON::LexerFactoryJSON,
				   "json",
				   JSONWordListDesc);
