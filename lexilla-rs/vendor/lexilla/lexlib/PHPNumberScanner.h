// Shared PHP numeric scanning. Lexer-specific styles stay with each caller.
#ifndef PHP_NUMBER_SCANNER_H
#define PHP_NUMBER_SCANNER_H

#include "Accessor.h"
#include "CharacterSet.h"

namespace Lexilla {
template<int NumberStyle, int ErrorStyle>
struct PHPNumberScanner {
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
		if (styler.SafeGetCharAt(pos, '\0') == '.') {
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

	styler.ColourTo(pos - 1, error ? ErrorStyle : NumberStyle);
}

};
} // namespace Lexilla

#endif
