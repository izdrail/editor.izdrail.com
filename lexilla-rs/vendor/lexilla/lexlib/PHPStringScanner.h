// Shared PHP quoted-string scanning for standalone PHP and Laravel/Blade.
// Style IDs stay owned by the registered lexer. No source is evaluated.
#ifndef PHP_STRING_SCANNER_H
#define PHP_STRING_SCANNER_H

#include <algorithm>
#include <cctype>

#include "Accessor.h"
#include "CharacterSet.h"

namespace Lexilla {
template<int SingleQuotedStyle, int DoubleQuotedStyle, int VariableStyle>
struct PHPStringScanner {
private:
    static bool IsIdentifierStart(int ch) {
        return (IsASCII(ch) && (isalpha(ch) || ch == '_')) || (!IsASCII(ch) && ch >= 0x80);
    }
    static bool IsIdentifierContinue(int ch) {
        return (IsASCII(ch) && (isalnum(ch) || ch == '_')) || (!IsASCII(ch) && ch >= 0x80);
    }
public:
static void ScanSingleQuotedString(Accessor &styler, Sci_Position &pos, Sci_Position max) {
	while (pos < max) {
		const int c = styler.SafeGetCharAt(pos, '\0');
		if (c == '\\') {
			pos = std::min(pos + 2, max);
		} else if (c == '\'') {
			pos++;
			break;
		} else {
			pos++;
		}
	}
	styler.ColourTo(pos - 1, SingleQuotedStyle);
}

// Consume a {$...} interpolation without letting its quoted keys close the
// surrounding string. This is a bounded lexical scan, not PHP evaluation.
static void ScanCurlyInterpolation(Accessor &styler, Sci_Position &pos, Sci_Position max, int stringStyle) {
	int depth = 1, quote = 0;
	bool blockComment = false, lineComment = false;
	pos++; // opening {
	while (pos < max && depth > 0) {
		const int c = styler.SafeGetCharAt(pos, '\0');
		const int n = styler.SafeGetCharAt(pos + 1, '\0');
		if (quote) {
			if (c == '\\' && pos + 1 < max) { pos += 2; continue; }
			if (c == quote) quote = 0;
		} else if (blockComment) {
			if (c == '*' && n == '/') { blockComment = false; pos += 2; continue; }
		} else if (lineComment) {
			if (c == '\r' || c == '\n') lineComment = false;
		} else if (c == '\'' || c == '"' || c == '`') {
			quote = c;
		} else if (c == '/' && n == '*') {
			blockComment = true; pos += 2; continue;
		} else if ((c == '/' && n == '/') || (c == '#' && n != '[')) {
			lineComment = true;
		} else if (c == '{') {
			depth++;
		} else if (c == '}') {
			depth--;
		} else if (c == '$' && IsIdentifierStart(n)) {
			styler.ColourTo(pos - 1, stringStyle);
			pos++;
			while (pos < max && IsIdentifierContinue(styler.SafeGetCharAt(pos, '\0'))) pos++;
			styler.ColourTo(pos - 1, VariableStyle);
			continue;
		}
		pos++;
	}
}

/* pos points just past the opening ".  Handles escapes and $variables. */
static void ScanDoubleQuotedString(Accessor &styler, Sci_Position &pos, Sci_Position max,
                                   int delimiter = '"', int stringStyle = DoubleQuotedStyle) {
	while (pos < max) {
		const int c = styler.SafeGetCharAt(pos, '\0');
		if (c == '\\') {
			pos = std::min(pos + 2, max);
		} else if (c == '{' && styler.SafeGetCharAt(pos + 1, '\0') == '$') {
			ScanCurlyInterpolation(styler, pos, max, stringStyle);
		} else if (c == delimiter) {
			pos++;
			break;
		} else if (c == '$' && IsIdentifierStart(styler.SafeGetCharAt(pos + 1, '\0'))) {
			/* flush the string piece up to (but not including) the $ */
			styler.ColourTo(pos - 1, stringStyle);
			pos++;   // consume '$'
			while (pos < max && IsIdentifierContinue(styler.SafeGetCharAt(pos, '\0')))
				pos++;
			styler.ColourTo(pos - 1, VariableStyle);
		} else {
			pos++;
		}
	}
	styler.ColourTo(pos - 1, stringStyle);
}

};
} // namespace Lexilla

#endif
