Block references now resolve to the block they mark: `![[note#^block-id]]`
embeds the marked paragraph, list item or quote block (an id alone on its
own line marks the block above it). The id marker is stripped from the
embedded copy; ids that don't resolve fall back to the `--missing-section`
strategy. Same-file section and block embeds (`![[#Heading]]` /
`![[#^block-id]]`) are now supported as well — previously they degraded
to plain links.
