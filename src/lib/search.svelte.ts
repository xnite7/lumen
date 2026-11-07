// The search field in the wide top bar. Typing there drives the Search page.
export const searchBar = $state({ q: "", open: false });

// The field's placeholder types itself out: "search shows", then films,
// anime, actors... (typed, held, deleted, next).
export const searchHint = $state({ text: "search shows" });
if (typeof window !== "undefined") {
  const words = ["shows", "films", "anime", "actors"];
  let w = 0, n = words[0].length, dir = -1, hold = 0;
  setInterval(() => {
    if (hold > 0) return void hold--;
    const word = words[w];
    n += dir;
    if (n >= word.length) {
      n = word.length;
      dir = -1;
      hold = 22; // ~1.8 s on the full word
    } else if (n <= 0) {
      n = 0;
      dir = 1;
      w = (w + 1) % words.length;
      hold = 3;
    }
    searchHint.text = "search " + words[w].slice(0, n);
  }, 80);
}
