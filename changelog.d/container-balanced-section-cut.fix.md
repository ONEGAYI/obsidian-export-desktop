Section cuts around headings inside block containers (blockquotes, lists)
no longer produce unbalanced event streams. Previously the stray
Start/End events polluted the renderer's padding stack and could swallow
following output into the quote — e.g. an Obsidian callout heading
referenced via `![[note#Heading]]`.
