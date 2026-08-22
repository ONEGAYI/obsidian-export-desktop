Section cuts on embeds now happen on the embedded note's own events,
before its nested embeds are expanded. Previously a heading pulled in by
a nested embed could terminate the outer section cut early, silently
dropping the outer note's own content after it. The embed postprocessor
contract (postprocessors see fully expanded content) is unchanged.
