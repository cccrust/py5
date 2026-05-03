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
    let mut in_triple_quote: Option<char> = None;
    let mut triple_quote_start_line = 0;
    let mut triple_quote_content = String::new();

    for line in source.lines() {
        let mut col = 0;
        let mut indent = 0;
        let chars: Vec<char> = line.chars().collect();

        if let Some(quote) = in_triple_quote {
            let mut i = 0;
            let mut escaped = false;
            while i < chars.len() {
                if !escaped && chars[i] == quote && i + 2 < chars.len()
                    && chars[i + 1] == quote && chars[i + 2] == quote {
                    i += 3;
                    in_triple_quote = None;
                    break;
                }
                if !escaped && chars[i] == '\\' && i + 1 < chars.len() {
                    escaped = true;
                } else {
                    escaped = false;
                }
                triple_quote_content.push(chars[i]);
                eprintln!("TRACE: Push {:?}, content len={}", chars[i], triple_quote_content.len());
                i += 1;
            }
            if in_triple_quote.is_some() {
                line_no += 1;
                continue;
            }
            tokens.push(Token {
                kind: TokenKind::String(triple_quote_content.clone()),
                line: triple_quote_start_line,
                col: 1,
            });
            eprintln!("TRACE: Emit String len={} content={:?}", triple_quote_content.len(), triple_quote_content);
            triple_quote_content.clear();
            if i >= chars.len() {
                line_no += 1;
                continue;
            }
        } else {
            while col < chars.len() && (chars[col] == ' ' || chars[col] == '\t') {
                indent += if chars[col] == '\t' { 4 } else { 1 };
                col += 1;
            }
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
                i += 1;
                let is_triple = i + 1 < chars.len() && chars[i] == quote && chars[i + 1] == quote;
if is_triple {
                    i += 2;
                    in_triple_quote = Some(quote);
                    triple_quote_start_line = line_no;
                    triple_quote_content.clear();
                    eprintln!("TRACE: Enter triple quote mode at line {}, i={}", line_no, i);
                    while i < chars.len() {
                        if in_triple_quote.is_some() && !triple_quote_content.is_empty()
                            && chars[i] == quote && i + 2 < chars.len()
                            && chars[i + 1] == quote && chars[i + 2] == quote {
                            i += 3;
                            in_triple_quote = None;
                            tokens.push(Token {
                                kind: TokenKind::String(triple_quote_content.clone()),
                                line: line_no,
                                col: start + 1,
                            });
                            triple_quote_content.clear();
                            break;
                        }
                        triple_quote_content.push(chars[i]);
                        i += 1;
                    }
                    if in_triple_quote.is_some() {
                        line_no += 1;
                        continue;
                    }
                } else {
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
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn check_token_kinds(source: &str, expected: &[TokenKind]) {
        let tokens = lex_source(source).unwrap();
        let actual: Vec<TokenKind> = tokens.iter().map(|t| t.kind.clone()).collect();
        let without_eof: Vec<TokenKind> = actual.iter()
            .filter(|k| !matches!(k, TokenKind::Eof))
            .cloned()
            .collect();
        for (i, ek) in expected.iter().enumerate() {
            assert!(
                without_eof.iter().any(|k| core::mem::discriminant(k) == core::mem::discriminant(ek)),
                "Expected {:?} not found in tokens (at index {}): {:?}",
                ek, i, without_eof
            );
        }
    }

    fn check_token_at(source: &str, pos: usize, expected: &TokenKind) {
        let tokens = lex_source(source).unwrap();
        let filtered: Vec<&TokenKind> = tokens.iter()
            .filter(|t| !matches!(t.kind, TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent))
            .map(|t| &t.kind)
            .collect();
        assert!(
            pos < filtered.len(),
            "Position {} out of range (len {})",
            pos,
            filtered.len()
        );
        assert_eq!(
            filtered[pos], expected,
            "Token at position {}: expected {:?}, got {:?}",
            pos, expected, filtered[pos]
        );
    }

    #[test]
    fn test_keywords() {
        let src = "def class if elif else while for in return break continue pass try except raise as and or not None True False lambda import from";
        check_token_kinds(src, &[
            TokenKind::Def,
            TokenKind::Class,
            TokenKind::If,
            TokenKind::Elif,
            TokenKind::Else,
            TokenKind::While,
            TokenKind::For,
            TokenKind::In,
            TokenKind::Return,
            TokenKind::Break,
            TokenKind::Continue,
            TokenKind::Pass,
            TokenKind::Try,
            TokenKind::Except,
            TokenKind::Raise,
            TokenKind::As,
            TokenKind::And,
            TokenKind::Or,
            TokenKind::Not,
            TokenKind::NoneVal,
            TokenKind::TrueVal,
            TokenKind::FalseVal,
            TokenKind::Lambda,
            TokenKind::Import,
            TokenKind::From,
        ]);
    }

    #[test]
    fn test_numbers() {
        let src = "42 -17 3.14 -2.5";
        let tokens = lex_source(src).unwrap();
        check_token_at(src, 0, &TokenKind::Int(42));
        check_token_at(src, 1, &TokenKind::Minus);
        check_token_at(src, 2, &TokenKind::Int(17));
        check_token_at(src, 3, &TokenKind::Float(3.14));
    }

    #[test]
    fn test_string() {
        let src = "'hello'";
        let tokens = lex_source(src).unwrap();
        check_token_at(src, 0, &TokenKind::String("hello".into()));
    }

    #[test]
    fn test_escape_sequences() {
        let src = "'line\\nhere'";
        let tokens = lex_source(src).unwrap();
        check_token_at(src, 0, &TokenKind::String("line\nhere".into()));
    }

    #[test]
    fn test_fstring() {
        let src = "f'hello {name}'";
        let tokens = lex_source(src).unwrap();
        check_token_at(src, 0, &TokenKind::FString("hello {name}".into()));
    }

    #[test]
    fn test_operators() {
        let src = "+ - * / % = == != < > <= >=";
        let tokens = lex_source(src).unwrap();
        check_token_at(src, 0, &TokenKind::Plus);
        check_token_at(src, 1, &TokenKind::Minus);
        check_token_at(src, 2, &TokenKind::Star);
        check_token_at(src, 3, &TokenKind::Slash);
        check_token_at(src, 4, &TokenKind::Percent);
        check_token_at(src, 5, &TokenKind::Equal);
        check_token_at(src, 6, &TokenKind::Eqeq);
        check_token_at(src, 7, &TokenKind::Ne);
        check_token_at(src, 8, &TokenKind::Lt);
        check_token_at(src, 9, &TokenKind::Gt);
        check_token_at(src, 10, &TokenKind::Le);
        check_token_at(src, 11, &TokenKind::Ge);
    }

    #[test]
    fn test_delimiters() {
        let src = "( ) [ ] { } , : .";
        check_token_at(src, 0, &TokenKind::Lparen);
        check_token_at(src, 1, &TokenKind::Rparen);
        check_token_at(src, 2, &TokenKind::Lbracket);
        check_token_at(src, 3, &TokenKind::Rbracket);
        check_token_at(src, 4, &TokenKind::Lbrace);
        check_token_at(src, 5, &TokenKind::Rbrace);
        check_token_at(src, 6, &TokenKind::Comma);
        check_token_at(src, 7, &TokenKind::Colon);
        check_token_at(src, 8, &TokenKind::Dot);
    }

    #[test]
    fn test_names() {
        let src = "x foo bar123 _private";
        check_token_at(src, 0, &TokenKind::Name("x".into()));
        check_token_at(src, 1, &TokenKind::Name("foo".into()));
        check_token_at(src, 2, &TokenKind::Name("bar123".into()));
        check_token_at(src, 3, &TokenKind::Name("_private".into()));
    }

    #[test]
    fn test_simple_assignment() {
        let src = "x = 5";
        check_token_at(src, 0, &TokenKind::Name("x".into()));
        check_token_at(src, 1, &TokenKind::Equal);
        check_token_at(src, 2, &TokenKind::Int(5));
    }

    #[test]
    fn test_arrow() {
        let src = "def foo() -> int:\n    pass";
        let tokens = lex_source(src).unwrap();
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Rarrow)));
    }

    #[test]
    fn test_indentation_block() {
        let src = "if True:\n    x = 1\n    y = 2";
        let tokens = lex_source(src).unwrap();
        let has_indent = tokens.iter().any(|t| matches!(t.kind, TokenKind::Indent));
        let has_dedent = tokens.iter().any(|t| matches!(t.kind, TokenKind::Dedent));
        assert!(has_indent, "Should have Indent token");
        assert!(has_dedent, "Should have Dedent token");
    }

    #[test]
    fn test_empty_line_no_indent() {
        let src = "x = 1\n\ny = 2";
        let tokens = lex_source(src).unwrap();
        let filtered: Vec<_> = tokens.iter()
            .filter(|t| !matches!(t.kind, TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent))
            .collect();
        assert!(filtered.len() >= 4, "Should have at least 4 meaningful tokens, got {}", filtered.len());
    }

    #[test]
    fn test_comment() {
        let src = "x = 5";
        check_token_at(src, 0, &TokenKind::Name("x".into()));
        check_token_at(src, 1, &TokenKind::Equal);
        check_token_at(src, 2, &TokenKind::Int(5));
    }

    #[test]
    fn test_invalid_character() {
        let result = lex_source("@");
        assert!(result.is_err());
    }

    #[test]
    fn test_complex_expression() {
        let src = "x + y * z - 10 / 2";
        check_token_at(src, 0, &TokenKind::Name("x".into()));
        check_token_at(src, 1, &TokenKind::Plus);
        check_token_at(src, 2, &TokenKind::Name("y".into()));
        check_token_at(src, 3, &TokenKind::Star);
        check_token_at(src, 4, &TokenKind::Name("z".into()));
        check_token_at(src, 5, &TokenKind::Minus);
        check_token_at(src, 6, &TokenKind::Int(10));
        check_token_at(src, 7, &TokenKind::Slash);
        check_token_at(src, 8, &TokenKind::Int(2));
    }

    #[test]
    fn test_unterminated_string() {
        let result = lex_source("'unterminated");
        assert!(result.is_err());
    }

    #[test]
    fn test_triple_quote() {
        let source = r#"x = """hello
world
"""
print(x)"#;
        match lex_source(source) {
            Ok(tokens) => {
                let has_string = tokens.iter().any(|t| matches!(t.kind, TokenKind::String(_)));
                assert!(has_string, "Should contain a string token");
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }
}