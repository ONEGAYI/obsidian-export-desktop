Desktop: settings switches and filter tabs are now fully labeled for screen readers and accessibility trees

The settings switches (hard line breaks, hidden files, link check, …)
exposed no accessible name at all and appeared as anonymous buttons in
accessibility trees. Their accessible name now carries the row title plus
the current state ("Hard line breaks (on)"): tree-based tooling typically
does not render the toggle state, so without the state in the name a click
left the observed tree completely unchanged. Screen readers will announce
the state twice (name and toggle state) — an accepted trade-off for
walk-through tooling. Additionally, the link-check report's filter tabs
now expose their pressed state (`aria-pressed`), path inputs are named by
their field label instead of falling back to the placeholder, and the
radio groups carry their group label.
