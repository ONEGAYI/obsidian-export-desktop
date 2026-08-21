Use forward slashes in generated link destinations on Windows

Relative link destinations generated on Windows contained backslashes (e.g. `![img](..\assets\img.png)`), which most Markdown renderers cannot resolve. Destinations now always use forward slashes, matching the output on Unix platforms.
