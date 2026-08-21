Fix section matching for headings with inline code and nested same-named headings

Headings containing inline code or math (e.g. `## \`code\` heading`) failed to match section references, since only plain text events were aggregated into the heading name. Their literal text now counts towards the heading name.

Additionally, a same-named heading nested deeper than the target no longer restarts the section: `![[note#Target]]` now embeds from the first matching heading to the end of that section, instead of just the innermost part.
