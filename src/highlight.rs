use ratatui::{
    style::{Color, Style},
    text::Span,
};

pub struct SyntaxRules {
    pub line_comments: &'static [&'static str],
    pub string_quotes: &'static [char],
    pub keywords: &'static [&'static str],
}

impl SyntaxRules {
    pub fn for_extension(ext: &str) -> Self {
        match ext {
            "rs" => SyntaxRules {
                line_comments: &["//"],
                string_quotes: &['"', '\''],
                keywords: &[
                    "fn", "let", "mut", "pub", "use", "mod", "struct", "enum", "impl", "trait",
                    "type", "match", "if", "else", "for", "while", "loop", "return", "async",
                    "await", "const", "static", "break", "continue", "where", "super", "self",
                    "Self", "true", "false", "unsafe", "dyn", "ref", "in", "as",
                ],
            },
            "py" => SyntaxRules {
                line_comments: &["#"],
                string_quotes: &['"', '\''],
                keywords: &[
                    "def", "class", "import", "from", "return", "if", "elif", "else", "for",
                    "while", "try", "except", "finally", "with", "as", "lambda", "pass", "break",
                    "continue", "yield", "in", "is", "not", "and", "or", "True", "False", "None",
                    "async", "await", "global", "nonlocal",
                ],
            },
            "js" | "ts" | "jsx" | "tsx" => SyntaxRules {
                line_comments: &["//"],
                string_quotes: &['"', '\'', '`'],
                keywords: &[
                    "function", "const", "let", "var", "return", "if", "else", "for", "while", "do",
                    "switch", "case", "default", "try", "catch", "finally", "class", "extends",
                    "import", "export", "from", "async", "await", "yield", "new", "this", "typeof",
                    "instanceof", "true", "false", "null", "undefined", "of", "in",
                ],
            },
            "c" | "cpp" | "h" | "hpp" | "cc" => SyntaxRules {
                line_comments: &["//"],
                string_quotes: &['"', '\''],
                keywords: &[
                    "int", "char", "float", "double", "void", "long", "short", "unsigned", "signed",
                    "struct", "class", "return", "if", "else", "for", "while", "do", "switch",
                    "case", "break", "continue", "const", "static", "extern", "typedef",
                    "namespace", "template", "using", "true", "false", "bool", "nullptr",
                ],
            },
            "toml" | "yaml" | "yml" | "ini" => SyntaxRules {
                line_comments: &["#", ";"],
                string_quotes: &['"', '\''],
                keywords: &["true", "false", "null"],
            },
            "sh" | "bash" | "zsh" => SyntaxRules {
                line_comments: &["#"],
                string_quotes: &['"', '\''],
                keywords: &[
                    "if", "then", "else", "elif", "fi", "for", "in", "do", "done", "while",
                    "case", "esac", "function", "return", "exit", "export", "local",
                ],
            },
            _ => SyntaxRules {
                line_comments: &["//", "#"],
                string_quotes: &['"', '\''],
                keywords: &[
                    "fn", "def", "function", "let", "var", "const", "if", "else", "return", "true",
                    "false",
                ],
            },
        }
    }
}

pub fn highlight_line(line: &str, ext: &str) -> Vec<Span<'static>> {
    if line.is_empty() {
        return vec![Span::raw("")];
    }

    let rules = SyntaxRules::for_extension(ext);
    let trimmed = line.trim_start();

    // Check full-line comments
    for &prefix in rules.line_comments {
        if trimmed.starts_with(prefix) {
            return vec![Span::styled(
                line.to_string(),
                Style::default().fg(Color::Green),
            )];
        }
    }

    let mut spans = Vec::new();
    let mut current_token = String::new();
    let mut in_string = false;
    let mut string_quote = ' ';
    let mut escaped = false;

    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        if in_string {
            current_token.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == string_quote {
                spans.push(Span::styled(
                    current_token.clone(),
                    Style::default().fg(Color::Yellow),
                ));
                current_token.clear();
                in_string = false;
            }
            i += 1;
            continue;
        }

        // Check inline line comments (e.g., code // comment)
        let slice = &line[chars_to_byte_idx(&chars, i)..];
        let mut found_comment = false;
        for &prefix in rules.line_comments {
            if slice.starts_with(prefix) {
                if !current_token.is_empty() {
                    spans.extend(style_token(&current_token, rules.keywords));
                    current_token.clear();
                }
                spans.push(Span::styled(
                    slice.to_string(),
                    Style::default().fg(Color::Green),
                ));
                found_comment = true;
                break;
            }
        }
        if found_comment {
            break;
        }

        // Rust lifetime / single quote disambiguation
        if ext == "rs" && c == '\'' {
            let is_lifetime = if i + 1 < len {
                let next = chars[i + 1];
                let is_prev_ident = i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
                !is_prev_ident && (next.is_alphabetic() || next == '_') && (i + 2 >= len || chars[i + 2] != '\'')
            } else {
                false
            };

            if is_lifetime {
                if !current_token.is_empty() {
                    spans.extend(style_token(&current_token, rules.keywords));
                    current_token.clear();
                }
                current_token.push(c);
                i += 1;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    current_token.push(chars[i]);
                    i += 1;
                }
                spans.push(Span::styled(
                    current_token.clone(),
                    Style::default().fg(Color::LightBlue),
                ));
                current_token.clear();
                continue;
            }
        }

        if rules.string_quotes.contains(&c) {
            if !current_token.is_empty() {
                spans.extend(style_token(&current_token, rules.keywords));
                current_token.clear();
            }
            in_string = true;
            string_quote = c;
            escaped = false;
            current_token.push(c);
            i += 1;
            continue;
        }

        if c.is_alphanumeric() || c == '_' {
            current_token.push(c);
        } else {
            if !current_token.is_empty() {
                spans.extend(style_token(&current_token, rules.keywords));
                current_token.clear();
            }
            spans.push(Span::raw(c.to_string()));
        }

        i += 1;
    }

    if !current_token.is_empty() {
        if in_string {
            spans.push(Span::styled(
                current_token,
                Style::default().fg(Color::Yellow),
            ));
        } else {
            spans.extend(style_token(&current_token, rules.keywords));
        }
    }

    spans
}

fn chars_to_byte_idx(chars: &[char], char_idx: usize) -> usize {
    chars[..char_idx].iter().map(|c| c.len_utf8()).sum()
}

fn style_token(token: &str, keywords: &[&str]) -> Vec<Span<'static>> {
    if keywords.contains(&token) {
        vec![Span::styled(
            token.to_string(),
            Style::default().fg(Color::Magenta).bold(),
        )]
    } else if token.chars().all(|c| c.is_numeric() || c == '.') {
        vec![Span::styled(
            token.to_string(),
            Style::default().fg(Color::LightRed),
        )]
    } else if token.chars().next().map_or(false, |c| c.is_uppercase()) {
        vec![Span::styled(
            token.to_string(),
            Style::default().fg(Color::Cyan),
        )]
    } else {
        vec![Span::raw(token.to_string())]
    }
}
