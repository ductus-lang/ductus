// Minimal CodeMirror language for chi: keyword/number/comment tokens.
import { StreamLanguage } from "@codemirror/language";

const keywords = new Set([
  "params", "metrics", "glyph", "contour", "guide", "anchor", "node", "def",
  "dir", "width", "nib", "cycle", "corner", "import", "for", "in", "if",
  "else", "and", "or", "not", "shifted", "scaled", "xscaled", "yscaled",
  "rotated", "slanted", "reversed",
]);

const constants = new Set(["up", "down", "left", "right"]);

export const chiLanguage = StreamLanguage.define({
  name: "chi",
  token(stream) {
    if (stream.eatSpace()) return null;
    if (stream.match(/#.*/)) return "comment";
    if (stream.match(/U\+[0-9A-Fa-f]+/)) return "atom";
    if (stream.match(/([0-9]*\.)?[0-9]+/)) return "number";
    if (stream.match(/"[^"]*"/)) return "string";
    if (stream.match(/--|\.\.|->/)) return "operator";
    const word = stream.match(/[a-zA-Z_][a-zA-Z0-9_]*/);
    if (word) {
      if (keywords.has(word[0])) return "keyword";
      if (constants.has(word[0])) return "atom";
      return null;
    }
    stream.next();
    return null;
  },
  languageData: {
    commentTokens: { line: "#" },
  },
});
