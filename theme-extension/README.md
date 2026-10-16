# Farum Azula Themes

Two dark themes for [Zed](https://zed.dev):

| Theme                       | Window      | Built on                          |
|-----------------------------|-------------|-----------------------------------|
| **Farum Azula Dark**        | opaque      | VScode Dark Plus                  |
| **Farum Azula Transparent** | transparent | Transparent Prism - Frost         |

Both share one syntax palette: blue keywords, gold functions, green types and attributes, orange
operators, yellow brackets and delimiters, italic comments.

## Self-contained

Each theme is a complete copy: every color of its base theme, with this collection's changes
already applied. Neither base theme has to be installed, and an update to either cannot change
these themes.

Colors the base themes leave unset (and which therefore are not in `themes/farum-azula.json`)
fall back to Zed's built-in dark defaults, as for any theme.

## Installation

Until the extension is published, install it from a clone of this repository:

1. Command palette, `zed: install dev extension`.
2. Pick the `theme-extension` folder.
3. `theme selector: toggle`, then choose a Farum Azula theme, or set it in `settings.json`:

   ```json
   "theme": "Farum Azula Dark"
   ```

After editing `themes/farum-azula.json`, press **Rebuild** next to the extension on the
Extensions page (`zed: extensions`).

## Credits

- Farum Azula Dark: [VScode Dark Plus](https://github.com/d1y/vscode_dark_plus.zed) by d1y.
- Farum Azula Transparent: [Transparent Prism Collection](https://github.com/magnuspladsen/dark-glass-theme)
  by Magnus Pladsen (MIT, see [LICENSE](LICENSE)).
