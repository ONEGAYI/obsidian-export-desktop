Section anchors now match exactly what GitHub and VS Code generate for a heading

Anchors are now produced with the `github-slugger` crate instead of a hand-rolled
slug function, fixing four divergences from GitHub/VS Code behavior (vectors for
the new behavior were captured from live GitHub rendering):

- Fullwidth punctuation such as `：` and `，` is now stripped. Previously it was
  kept verbatim in the anchor, so links like `[[note#总纲：三份形态，两个断口]]`
  produced `#总纲：三份形态，两个断口`, which resolves on neither GitHub nor VS
  Code (both generate `#总纲三份形态两个断口`).
- Punctuation no longer leaves a hyphen behind. Numbered headings such as
  `1.1.1 C` (see the upstream issue this closes for the "Number Headings"
  plugin use case) now produce `#111-c` instead of the broken `#1-1-1-c`.
- Runs of consecutive hyphens are kept (`this--or-that` stays `this--or-that`).
- Leading/trailing hyphens are no longer trimmed.

Link display text is unchanged; only the `#anchor` part of generated links is
affected.
