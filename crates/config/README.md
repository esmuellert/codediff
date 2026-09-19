# config

Persistent user preferences for `codediff`.

The crate owns the versioned JSON file format, platform path selection, and
atomic writes. It does not depend on Git or the terminal; application crates
convert the typed preferences into their runtime settings.

The application searches for a file in this order:

1. `--config PATH`;
2. `CODEDIFF_CONFIG`;
3. `$XDG_CONFIG_HOME/codediff/config.json`, or
   `$HOME/.config/codediff/config.json` on macOS and Linux;
4. `%APPDATA%\\codediff\\config.json` on Windows.

A minimal file can override only the preferences it needs:

```json
{
  "version": 1,
  "ui": {
    "layout": "inline",
    "theme": "catppuccin-mocha",
    "explorer_width": 40,
    "explorer_mode": "tree",
    "wrap": true
  },
  "diff": {
    "ignore_trim_whitespace": false
  },
  "keybindings": {
    "quit": ["q"]
  }
}
```

Missing values use the built-in defaults. Malformed or unsupported files are
left untouched and the application reports a warning while using defaults.
