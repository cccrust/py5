#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TokenKind {
    Eof,
    Newline,
    Indent,
    Dedent,
    Name(String),
    Int(i64),
    Float(f64),
    String(String),
    FString(String),
    Def,
    Class,
    If,
    Elif,
    Else,
    While,
    For,
    In,
    Return,
    Break,
    Continue,
    Pass,
    Try,
    Except,
    Raise,
    As,
    And,
    Or,
    Not,
    NoneVal,
    TrueVal,
    FalseVal,
    Lambda,
    Import,
    From,
    Lparen,
    Rparen,
    Lbracket,
    Rbracket,
    Lbrace,
    Rbrace,
    Comma,
    Colon,
    Dot,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Equal,
    PlusEq,
    MinusEq,
    Eqeq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Rarrow,
}

#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) line: usize,
    pub(crate) col: usize,
}

pub(crate) fn lex_source(source: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut indent_stack = vec![0];
    let mut line_no = 1;
    let mut paren_level = 0;

    for line in source.lines() {
        let mut col = 0;
        let mut indent = 0;
        let chars: Vec<char> = line.chars().collect();
        while col < chars.len() && (chars[col] == ' ' || chars[col] == '\t') {
            indent += if chars[col] == '\t' { 4 } else { 1 };
            col += 1;
        }
        if col == chars.len() || chars[col] == '#' {
            line_no += 1;
            continue;
        }

        if paren_level == 0 {
            let top = *indent_stack.last().unwrap();
            if indent > top {
                indent_stack.push(indent);
                tokens.push(Token {
                    kind: TokenKind::Indent,
                    line: line_no,
                    col: 1,
                });
            } else {
                while indent < *indent_stack.last().unwrap() {
                    indent_stack.pop();
                    tokens.push(Token {
                        kind: TokenKind::Dedent,
                        line: line_no,
                        col: 1,
                    });
                }
                if indent != *indent_stack.last().unwrap() {
                    return Err(format!("inconsistent indent at line {}", line_no));
                }
            }
        }

        let mut i = col;
        while i < chars.len() {
            let c = chars[i];
            if c == '#' {
                break;
            }
            if c.is_ascii_whitespace() {
                i += 1;
                continue;
            }

            if (c == 'f' || c == 'F')
                && i + 1 < chars.len()
                && (chars[i + 1] == '\'' || chars[i + 1] == '"')
            {
                let quote = chars[i + 1];
                let start = i;
                i += 2;
                let mut val = String::new();
                while i < chars.len() && chars[i] != quote {
                    if chars[i] == '\\' {
                        i += 1;
                        if i == chars.len() {
                            break;
                        }
                        match chars[i] {
                            'n' => val.push('\n'),
                            't' => val.push('\t'),
                            '\\' => val.push('\\'),
                            '{' => val.push('{'),
                            '}' => val.push('}'),
                            '\'' => val.push('\''),
                            '"' => val.push('"'),
                            _ => val.push(chars[i]),
                        }
                    } else {
                        val.push(chars[i]);
                    }
                    i += 1;
                }
                if i == chars.len() {
                    return Err(format!("unterminated f-string line {}", line_no));
                }
                i += 1;
                tokens.push(Token {
                    kind: TokenKind::FString(val),
                    line: line_no,
                    col: start + 1,
                });
                continue;
            }

            if c.is_ascii_alphabetic() || c == '_' {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                let kind = match text.as_str() {
                    "def" => TokenKind::Def,
                    "class" => TokenKind::Class,
                    "if" => TokenKind::If,
                    "elif" => TokenKind::Elif,
                    "else" => TokenKind::Else,
                    "while" => TokenKind::While,
                    "for" => TokenKind::For,
                    "in" => TokenKind::In,
                    "return" => TokenKind::Return,
                    "break" => TokenKind::Break,
                    "continue" => TokenKind::Continue,
                    "pass" => TokenKind::Pass,
                    "try" => TokenKind::Try,
                    "except" => TokenKind::Except,
                    "raise" => TokenKind::Raise,
                    "as" => TokenKind::As,
                    "and" => TokenKind::And,
                    "or" => TokenKind::Or,
                    "not" => TokenKind::Not,
                    "None" => TokenKind::NoneVal,
                    "True" => TokenKind::TrueVal,
                    "False" => TokenKind::FalseVal,
                    "lambda" => TokenKind::Lambda,
                    "import" => TokenKind::Import,
                    "from" => TokenKind::From,
                    _ => TokenKind::Name(text),
                };
                tokens.push(Token {
                    kind,
                    line: line_no,
                    col: start + 1,
                });
                continue;
            }
            if c.is_ascii_digit() {
                let start = i;
                let mut is_float = false;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                if i < chars.len() && chars[i] == '.' {
                    is_float = true;
                    i += 1;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let text: String = chars[start..i].iter().collect();
                let kind = if is_float {
                    TokenKind::Float(text.parse().unwrap())
                } else {
                    TokenKind::Int(text.parse().unwrap())
                };
                tokens.push(Token {
                    kind,
                    line: line_no,
                    col: start + 1,
                });
                continue;
            }
            if c == '\'' || c == '"' {
                let quote = c;
                let start = i;
                let mut val = String::new();
                i += 1;
                while i < chars.len() && chars[i] != quote {
                    if chars[i] == '\\' {
                        i += 1;
                        if i == chars.len() {
                            break;
                        }
                        match chars[i] {
                            'n' => val.push('\n'),
                            't' => val.push('\t'),
                            '\\' => val.push('\\'),
                            '\'' => val.push('\''),
                            '"' => val.push('"'),
                            _ => val.push(chars[i]),
                        }
                    } else {
                        val.push(chars[i]);
                    }
                    i += 1;
                }
                if i == chars.len() {
                    return Err(format!("unterminated string line {}", line_no));
                }
                i += 1;
                tokens.push(Token {
                    kind: TokenKind::String(val),
                    line: line_no,
                    col: start + 1,
                });
                continue;
            }

            let start = i;
            let (kind, step) = if i + 1 < chars.len() && chars[i] == '=' && chars[i + 1] == '=' {
                (TokenKind::Eqeq, 2)
            } else if i + 1 < chars.len() && chars[i] == '!' && chars[i + 1] == '=' {
                (TokenKind::Ne, 2)
            } else if i + 1 < chars.len() && chars[i] == '<' && chars[i + 1] == '=' {
                (TokenKind::Le, 2)
            } else if i + 1 < chars.len() && chars[i] == '>' && chars[i + 1] == '=' {
                (TokenKind::Ge, 2)
            } else if i + 1 < chars.len() && chars[i] == '+' && chars[i + 1] == '=' {
                (TokenKind::PlusEq, 2)
            } else if i + 1 < chars.len() && chars[i] == '-' && chars[i + 1] == '=' {
                (TokenKind::MinusEq, 2)
            } else if i + 1 < chars.len() && chars[i] == '-' && chars[i + 1] == '>' {
                (TokenKind::Rarrow, 2)
            } else {
                let k = match c {
                    '(' => {
                        paren_level += 1;
                        TokenKind::Lparen
                    }
                    ')' => {
                        paren_level -= 1;
                        TokenKind::Rparen
                    }
                    '[' => {
                        paren_level += 1;
                        TokenKind::Lbracket
                    }
                    ']' => {
                        paren_level -= 1;
                        TokenKind::Rbracket
                    }
                    '{' => {
                        paren_level += 1;
                        TokenKind::Lbrace
                    }
                    '}' => {
                        paren_level -= 1;
                        TokenKind::Rbrace
                    }
                    ',' => TokenKind::Comma,
                    ':' => TokenKind::Colon,
                    '.' => TokenKind::Dot,
                    '+' => TokenKind::Plus,
                    '-' => TokenKind::Minus,
                    '*' => TokenKind::Star,
                    '/' => TokenKind::Slash,
                    '%' => TokenKind::Percent,
                    '=' => TokenKind::Equal,
                    '<' => TokenKind::Lt,
                    '>' => TokenKind::Gt,
                    _ => return Err(format!("unexpected '{}' line {}", c, line_no)),
                };
                (k, 1)
            };
            tokens.push(Token {
                kind,
                line: line_no,
                col: start + 1,
            });
            i += step;
        }

        if paren_level == 0 {
            tokens.push(Token {
                kind: TokenKind::Newline,
                line: line_no,
                col: chars.len() + 1,
            });
        }
        line_no += 1;
    }
    while indent_stack.len() > 1 {
        indent_stack.pop();
        tokens.push(Token {
            kind: TokenKind::Dedent,
            line: line_no,
            col: 1,
        });
    }
    tokens.push(Token {
        kind: TokenKind::Eof,
        line: line_no,
        col: 1,
    });
    Ok(tokens)
}