// Shared PHP heredoc/nowdoc scanning. Each lexer owns its style IDs.
#ifndef PHP_HEREDOC_SCANNER_H
#define PHP_HEREDOC_SCANNER_H

#include <cctype>
#include "Accessor.h"
#include "CharacterSet.h"

namespace Lexilla {
template<int HeredocStyle, int NowdocStyle, int VariableStyle, int ErrorStyle>
struct PHPHeredocScanner {
private:
    static bool IsIdentifierStart(int ch) {
        return (IsASCII(ch) && (isalpha(ch) || ch == '_')) || (!IsASCII(ch) && ch >= 0x80);
    }
    static bool IsIdentifierContinue(int ch) {
        return (IsASCII(ch) && (isalnum(ch) || ch == '_')) || (!IsASCII(ch) && ch >= 0x80);
    }
public:
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
				while (p < lineEnd && (styler.SafeGetCharAt(p, '\0') == ' ' || styler.SafeGetCharAt(p, '\0') == '\t')) p++;
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
	const int style = nowdoc ? NowdocStyle : HeredocStyle;
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
			if (c == '\n' || c == '\r') {
				pos++;
				if (c == '\r' && pos < max && styler.SafeGetCharAt(pos, '\0') == '\n') pos++;
				break;
			}
			if (!nowdoc && c == '\\' && pos + 1 < max &&
			    styler.SafeGetCharAt(pos + 1, '\0') != '\n' &&
			    styler.SafeGetCharAt(pos + 1, '\0') != '\r') {
				pos += 2;
				continue;
			}
			if (!nowdoc && c == '$' && IsIdentifierStart(styler.SafeGetCharAt(pos + 1, '\0'))) {
				styler.ColourTo(pos - 1, style);
				pos++;
				while (pos < max && IsIdentifierContinue(styler.SafeGetCharAt(pos, '\0'))) pos++;
				styler.ColourTo(pos - 1, VariableStyle);
				continue;
			}
			pos++;
		}
	}
	styler.ColourTo(pos - 1, style);
}

/* pos points just past "<<<". */
static void ScanHeredoc(Accessor &styler, Sci_Position &pos, Sci_Position max) {
	int quote = 0;
	while (pos < max && (styler.SafeGetCharAt(pos, '\0') == ' ' || styler.SafeGetCharAt(pos, '\0') == '\t')) pos++;
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
		styler.ColourTo(pos - 1, ErrorStyle);
		return;
	}
	if (quote) {
		if (styler.SafeGetCharAt(pos, '\0') != quote) {
			styler.ColourTo(pos - 1, ErrorStyle);
			return;
		}
		pos++;
	}
	/* Skip the rest of the opening line. */
	while (pos < max) {
		const int ch = styler.SafeGetCharAt(pos, '\0');
		if (ch == '\n' || ch == '\r') {
				pos++;
				if (ch == '\r' && pos < max && styler.SafeGetCharAt(pos, '\0') == '\n') pos++;
				break;
			}
		pos++;
	}
	ScanHeredocBody(styler, pos, max, idStart, idLen, quote == '\'');
}

};
} // namespace Lexilla

#endif
