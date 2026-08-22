Desktop: bilingual UI (Chinese / English) with a language menu

All desktop UI strings moved from hard-coded Chinese into i18n dictionaries, with a full English translation alongside. A language dropdown in the title bar offers Chinese, English, and "follow system" (detected from the OS locale, any `zh*` locale resolves to Chinese); any of the three can be picked freely and the choice persists across sessions.
