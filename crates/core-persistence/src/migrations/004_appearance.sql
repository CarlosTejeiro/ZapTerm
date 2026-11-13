-- Migration 004: terminal appearance — color schemes + settings key-value store

CREATE TABLE IF NOT EXISTS color_schemes (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         TEXT NOT NULL UNIQUE,
    palette_json TEXT NOT NULL,
    is_builtin   INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- ── 10 built-in color schemes ────────────────────────────────────────────────

INSERT OR IGNORE INTO color_schemes (name, palette_json, is_builtin) VALUES (
    'One Dark',
    '{"background":"#282c34","foreground":"#abb2bf","cursor":"#528bff","black":"#282c34","red":"#e06c75","green":"#98c379","yellow":"#e5c07b","blue":"#61afef","magenta":"#c678dd","cyan":"#56b6c2","white":"#abb2bf","brightBlack":"#5c6370","brightRed":"#e06c75","brightGreen":"#98c379","brightYellow":"#e5c07b","brightBlue":"#61afef","brightMagenta":"#c678dd","brightCyan":"#56b6c2","brightWhite":"#ffffff"}',
    1
);

INSERT OR IGNORE INTO color_schemes (name, palette_json, is_builtin) VALUES (
    'Dracula',
    '{"background":"#282a36","foreground":"#f8f8f2","cursor":"#f8f8f2","black":"#21222c","red":"#ff5555","green":"#50fa7b","yellow":"#f1fa8c","blue":"#bd93f9","magenta":"#ff79c6","cyan":"#8be9fd","white":"#f8f8f2","brightBlack":"#6272a4","brightRed":"#ff6e6e","brightGreen":"#69ff94","brightYellow":"#ffffa5","brightBlue":"#d6acff","brightMagenta":"#ff92df","brightCyan":"#a4ffff","brightWhite":"#ffffff"}',
    1
);

INSERT OR IGNORE INTO color_schemes (name, palette_json, is_builtin) VALUES (
    'Tokyo Night',
    '{"background":"#1a1b26","foreground":"#c0caf5","cursor":"#c0caf5","black":"#15161e","red":"#f7768e","green":"#9ece6a","yellow":"#e0af68","blue":"#7aa2f7","magenta":"#bb9af7","cyan":"#7dcfff","white":"#a9b1d6","brightBlack":"#414868","brightRed":"#f7768e","brightGreen":"#9ece6a","brightYellow":"#e0af68","brightBlue":"#7aa2f7","brightMagenta":"#bb9af7","brightCyan":"#7dcfff","brightWhite":"#c0caf5"}',
    1
);

INSERT OR IGNORE INTO color_schemes (name, palette_json, is_builtin) VALUES (
    'Nord',
    '{"background":"#2e3440","foreground":"#d8dee9","cursor":"#d8dee9","black":"#3b4252","red":"#bf616a","green":"#a3be8c","yellow":"#ebcb8b","blue":"#81a1c1","magenta":"#b48ead","cyan":"#88c0d0","white":"#e5e9f0","brightBlack":"#4c566a","brightRed":"#bf616a","brightGreen":"#a3be8c","brightYellow":"#ebcb8b","brightBlue":"#81a1c1","brightMagenta":"#b48ead","brightCyan":"#8fbcbb","brightWhite":"#eceff4"}',
    1
);

INSERT OR IGNORE INTO color_schemes (name, palette_json, is_builtin) VALUES (
    'Gruvbox Dark',
    '{"background":"#282828","foreground":"#ebdbb2","cursor":"#ebdbb2","black":"#282828","red":"#cc241d","green":"#98971a","yellow":"#d79921","blue":"#458588","magenta":"#b16286","cyan":"#689d6a","white":"#a89984","brightBlack":"#928374","brightRed":"#fb4934","brightGreen":"#b8bb26","brightYellow":"#fabd2f","brightBlue":"#83a598","brightMagenta":"#d3869b","brightCyan":"#8ec07c","brightWhite":"#ebdbb2"}',
    1
);

INSERT OR IGNORE INTO color_schemes (name, palette_json, is_builtin) VALUES (
    'Catppuccin Mocha',
    '{"background":"#1e1e2e","foreground":"#cdd6f4","cursor":"#f5e0dc","black":"#45475a","red":"#f38ba8","green":"#a6e3a1","yellow":"#f9e2af","blue":"#89b4fa","magenta":"#f5c2e7","cyan":"#94e2d5","white":"#bac2de","brightBlack":"#585b70","brightRed":"#f38ba8","brightGreen":"#a6e3a1","brightYellow":"#f9e2af","brightBlue":"#89b4fa","brightMagenta":"#f5c2e7","brightCyan":"#94e2d5","brightWhite":"#a6adc8"}',
    1
);

INSERT OR IGNORE INTO color_schemes (name, palette_json, is_builtin) VALUES (
    'Monokai',
    '{"background":"#272822","foreground":"#f8f8f2","cursor":"#f8f8f0","black":"#272822","red":"#f92672","green":"#a6e22e","yellow":"#f4bf75","blue":"#66d9e8","magenta":"#ae81ff","cyan":"#a1efe4","white":"#f8f8f2","brightBlack":"#75715e","brightRed":"#f92672","brightGreen":"#a6e22e","brightYellow":"#f4bf75","brightBlue":"#66d9e8","brightMagenta":"#ae81ff","brightCyan":"#a1efe4","brightWhite":"#f9f8f5"}',
    1
);

INSERT OR IGNORE INTO color_schemes (name, palette_json, is_builtin) VALUES (
    'Solarized Dark',
    '{"background":"#002b36","foreground":"#839496","cursor":"#839496","black":"#073642","red":"#dc322f","green":"#859900","yellow":"#b58900","blue":"#268bd2","magenta":"#d33682","cyan":"#2aa198","white":"#eee8d5","brightBlack":"#002b36","brightRed":"#cb4b16","brightGreen":"#586e75","brightYellow":"#657b83","brightBlue":"#839496","brightMagenta":"#6c71c4","brightCyan":"#93a1a1","brightWhite":"#fdf6e3"}',
    1
);

INSERT OR IGNORE INTO color_schemes (name, palette_json, is_builtin) VALUES (
    'Solarized Light',
    '{"background":"#fdf6e3","foreground":"#657b83","cursor":"#586e75","black":"#073642","red":"#dc322f","green":"#859900","yellow":"#b58900","blue":"#268bd2","magenta":"#d33682","cyan":"#2aa198","white":"#eee8d5","brightBlack":"#002b36","brightRed":"#cb4b16","brightGreen":"#586e75","brightYellow":"#657b83","brightBlue":"#839496","brightMagenta":"#6c71c4","brightCyan":"#93a1a1","brightWhite":"#fdf6e3"}',
    1
);

INSERT OR IGNORE INTO color_schemes (name, palette_json, is_builtin) VALUES (
    'GitHub Dark',
    '{"background":"#0d1117","foreground":"#c9d1d9","cursor":"#58a6ff","black":"#484f58","red":"#ff7b72","green":"#3fb950","yellow":"#d29922","blue":"#58a6ff","magenta":"#bc8cff","cyan":"#39c5cf","white":"#b1bac4","brightBlack":"#6e7681","brightRed":"#ffa198","brightGreen":"#56d364","brightYellow":"#e3b341","brightBlue":"#79c0ff","brightMagenta":"#d2a8ff","brightCyan":"#56d4dd","brightWhite":"#f0f6fc"}',
    1
);

-- ── Default terminal settings ────────────────────────────────────────────────

INSERT OR IGNORE INTO settings (key, value) VALUES
    ('terminal.fontFamily',        'Cascadia Code, JetBrains Mono, monospace'),
    ('terminal.fontSize',          '14'),
    ('terminal.lineHeight',        '1.2'),
    ('terminal.cursorStyle',       'block'),
    ('terminal.cursorBlink',       'true'),
    ('terminal.activeColorScheme', 'One Dark');

INSERT OR IGNORE INTO schema_version (version) VALUES (4);
