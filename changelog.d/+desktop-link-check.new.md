Desktop: automatic link check after export + paginated settings

The desktop app can now run the link checker automatically after a
successful export. A new "Link Check" settings page toggles it and picks
the check target: the vault source (default — catches dead wikilinks,
embeds, and plain links before conversion) or the exported tree (verifies
the generated Markdown links and anchors; broken wikilinks have already
collapsed to plain text there). Checking the vault source reuses the
export's filter options so the checked file set matches the exported one.

The report panel shows the run summary (files, links, broken, skipped)
with filter tabs (broken by default, all, skipped); verdicts are rendered
from the structured payloads and localized, while paths and raw link text
stay verbatim. A check can be cancelled by going back to the main view.

The settings view itself was rebuilt as a paginated layout (side
navigation: Conversion / Content Filtering / Files & Process / Link
Check), collapsing to horizontal tabs on narrow windows.
