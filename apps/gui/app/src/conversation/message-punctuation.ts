const LATIN_LETTER_PATTERN = /[A-Za-z]/u;
const CJK_PATTERN = /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}]/u;
const ASCII_WORD_PATTERN = /[A-Za-z0-9]/u;

const ENGLISH_PUNCTUATION_REPLACEMENTS: Readonly<Record<string, string>> = {
  "‘": "'",
  "’": "'",
  "“": '"',
  "”": '"',
  "＇": "'",
  "＂": '"',
  "，": ",",
  "。": ".",
  "！": "!",
  "？": "?",
  "；": ";",
  "：": ":",
  "、": ",",
  "（": "(",
  "）": ")",
  "【": "[",
  "】": "]",
  "《": "<",
  "》": ">",
};

export function normalizeEnglishPunctuation(source: string): string {
  return source
    .split(/(\r\n|\r|\n)/u)
    .map((line) => normalizeLine(line))
    .join("");
}

function normalizeLine(line: string): string {
  if (!LATIN_LETTER_PATTERN.test(line)) {
    return line;
  }
  if (!CJK_PATTERN.test(line)) {
    return Array.from(
      line,
      (character) => ENGLISH_PUNCTUATION_REPLACEMENTS[character] ?? character,
    ).join("");
  }
  const characters = Array.from(line);
  return characters
    .map((character, index) => {
      const replacement = ENGLISH_PUNCTUATION_REPLACEMENTS[character];
      if (!replacement) {
        return character;
      }
      const previous = characters[index - 1] ?? "";
      const next = characters[index + 1] ?? "";
      if (ASCII_WORD_PATTERN.test(previous) && ASCII_WORD_PATTERN.test(next)) {
        return replacement;
      }
      const previousSignificant = nearestNonWhitespace(characters, index, -1);
      const nextSignificant = nearestNonWhitespace(characters, index, 1);
      return ASCII_WORD_PATTERN.test(previousSignificant) &&
        ASCII_WORD_PATTERN.test(nextSignificant)
        ? replacement
        : character;
    })
    .join("");
}

function nearestNonWhitespace(characters: string[], start: number, direction: -1 | 1): string {
  for (let index = start + direction; index >= 0 && index < characters.length; index += direction) {
    if (!/\s/u.test(characters[index] ?? "")) {
      return characters[index] ?? "";
    }
  }
  return "";
}
