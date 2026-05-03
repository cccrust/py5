use crate::ast::{Expr, Stmt};
use crate::lexer::{Token, TokenKind};

pub(crate) struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    filename: &'a str,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(tokens: &'a [Token], filename: &'a str) -> Self {
        Self {
            tokens,
            pos: 0,
            filename,
        }
    }
    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }
    fn prev(&self) -> &Token {
        &self.tokens[self.pos - 1]
    }
    fn match_token(&mut self, kind: &TokenKind) -> bool {
        if core::mem::discriminant(&self.peek().kind) == core::mem::discriminant(kind) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, kind: TokenKind, msg: &str) -> Result<&Token, String> {
        if core::mem::discriminant(&self.peek().kind) != core::mem::discriminant(&kind) {
            Err(format!(
                "{}:{}:{}: {}",
                self.filename,
                self.peek().line,
                self.peek().col,
                msg
            ))
        } else {
            self.pos += 1;
            Ok(self.prev())
        }
    }
    fn skip_newlines(&mut self) {
        while self.match_token(&TokenKind::Newline) {}
    }

    fn maybe_skip_type_annotation(&mut self) {
        if self.match_token(&TokenKind::Colon) {
            let _ = self.parse_expr();
        }
    }

    fn parse_expr_list(&mut self) -> Result<Expr, String> {
        let first = self.parse_expr()?;
        if self.match_token(&TokenKind::Comma) {
            let mut items = vec![first];
            if matches!(
                self.peek().kind,
                TokenKind::Equal
                    | TokenKind::PlusEq
                    | TokenKind::MinusEq
                    | TokenKind::In
                    | TokenKind::Colon
                    | TokenKind::Newline
                    | TokenKind::Eof
                    | TokenKind::Rparen
                    | TokenKind::Rbracket
                    | TokenKind::Rbrace
            ) {
                return Ok(Expr::Tuple(items));
            }
            loop {
                items.push(self.parse_expr()?);
                if !self.match_token(&TokenKind::Comma) {
                    break;
                }
                if matches!(
                    self.peek().kind,
                    TokenKind::Equal
                        | TokenKind::PlusEq
                        | TokenKind::MinusEq
                        | TokenKind::In
                        | TokenKind::Colon
                        | TokenKind::Newline
                        | TokenKind::Eof
                        | TokenKind::Rparen
                        | TokenKind::Rbracket
                        | TokenKind::Rbrace
                ) {
                    break;
                }
            }
            Ok(Expr::Tuple(items))
        } else {
            Ok(first)
        }
    }

    fn parse_dotted_name(&mut self) -> Result<String, String> {
        let mut name = if let TokenKind::Name(n) = &self
            .expect(TokenKind::Name("".into()), "expected module name")?
            .kind
        {
            n.clone()
        } else {
            unreachable!()
        };
        while self.match_token(&TokenKind::Dot) {
            if let TokenKind::Name(n) = &self
                .expect(TokenKind::Name("".into()), "expected module name after dot")?
                .kind
            {
                name.push('.');
                name.push_str(n);
            } else {
                unreachable!()
            }
        }
        Ok(name)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let tok = self.peek().clone();
        let mut e = match &tok.kind {
            TokenKind::NoneVal => {
                self.pos += 1;
                Expr::NoneVal
            }
            TokenKind::TrueVal => {
                self.pos += 1;
                Expr::Bool(true)
            }
            TokenKind::FalseVal => {
                self.pos += 1;
                Expr::Bool(false)
            }
            TokenKind::Int(v) => {
                self.pos += 1;
                Expr::Int(*v)
            }
            TokenKind::Float(v) => {
                self.pos += 1;
                Expr::Float(*v)
            }
            TokenKind::String(v) => {
                self.pos += 1;
                Expr::String(v.clone())
            }
            TokenKind::FString(v) => {
                self.pos += 1;
                Expr::FString(v.clone())
            }
            TokenKind::Name(n) => {
                self.pos += 1;
                Expr::Name(n.clone())
            }
            TokenKind::Lparen => {
                self.pos += 1;
                if self.match_token(&TokenKind::Rparen) {
                    Expr::Tuple(vec![])
                } else {
                    let first = self.parse_expr()?;
                    if self.match_token(&TokenKind::Comma) {
                        let mut items = vec![first];
                        if self.peek().kind != TokenKind::Rparen {
                            loop {
                                items.push(self.parse_expr()?);
                                if !self.match_token(&TokenKind::Comma)
                                    || self.peek().kind == TokenKind::Rparen
                                {
                                    break;
                                }
                            }
                        }
                        self.expect(TokenKind::Rparen, "expected ')'")?;
                        Expr::Tuple(items)
                    } else {
                        self.expect(TokenKind::Rparen, "expected ')'")?;
                        first
                    }
                }
            }
            TokenKind::Lbracket => {
                self.pos += 1;
                let mut items = Vec::new();
                if self.match_token(&TokenKind::Rbracket) {
                    Expr::List(vec![])
                } else {
                    let first = self.parse_expr()?;
                    if self.match_token(&TokenKind::For) {
                        let target = self.parse_expr_list()?;
                        self.expect(TokenKind::In, "expected 'in'")?;
                        let iter = self.parse_expr()?;
                        let cond = if self.match_token(&TokenKind::If) {
                            Some(Box::new(self.parse_expr()?))
                        } else {
                            None
                        };
                        self.expect(TokenKind::Rbracket, "expected ']'")?;
                        Expr::ListComp(Box::new(first), Box::new(target), Box::new(iter), cond)
                    } else {
                        items.push(first);
                        if self.match_token(&TokenKind::Comma)
                            && self.peek().kind != TokenKind::Rbracket
                        {
                            loop {
                                items.push(self.parse_expr()?);
                                if !self.match_token(&TokenKind::Comma)
                                    || self.peek().kind == TokenKind::Rbracket
                                {
                                    break;
                                }
                            }
                        }
                        self.expect(TokenKind::Rbracket, "expected ']'")?;
                        Expr::List(items)
                    }
                }
            }
            TokenKind::Lbrace => {
                self.pos += 1;
                let mut pairs = Vec::new();
                if !self.match_token(&TokenKind::Rbrace) {
                    loop {
                        let k = self.parse_expr()?;
                        self.expect(TokenKind::Colon, "expected ':'")?;
                        pairs.push((k, self.parse_expr()?));
                        if !self.match_token(&TokenKind::Comma)
                            || self.peek().kind == TokenKind::Rbrace
                        {
                            break;
                        }
                    }
                    self.expect(TokenKind::Rbrace, "expected '}'")?;
                }
                Expr::Dict(pairs)
            }
            _ => {
                return Err(format!(
                    "{}:{}:{}: expected expr",
                    self.filename, tok.line, tok.col
                ))
            }
        };
        self.parse_postfix(&mut e)?;
        Ok(e)
    }

    fn parse_postfix(&mut self, expr: &mut Expr) -> Result<(), String> {
        loop {
            if self.match_token(&TokenKind::Lparen) {
                let mut args = Vec::new();
                let mut kwargs = Vec::new();
                if !self.match_token(&TokenKind::Rparen) {
                    loop {
                        let mut is_kwarg = false;
                        if let Some(t1) = self.tokens.get(self.pos) {
                            if let TokenKind::Name(_) = t1.kind {
                                if let Some(t2) = self.tokens.get(self.pos + 1) {
                                    if t2.kind == TokenKind::Equal {
                                        is_kwarg = true;
                                    }
                                }
                            }
                        }
                        if is_kwarg {
                            let name = if let TokenKind::Name(n) = &self.peek().kind {
                                n.clone()
                            } else {
                                unreachable!()
                            };
                            self.pos += 2;
                            kwargs.push((name, self.parse_expr()?));
                        } else {
                            args.push(self.parse_expr()?);
                        }
                        if !self.match_token(&TokenKind::Comma)
                            || self.peek().kind == TokenKind::Rparen
                        {
                            break;
                        }
                    }
                    self.expect(TokenKind::Rparen, "expected ')'")?;
                }
                *expr = Expr::Call(Box::new(expr.clone()), args, kwargs);
            } else if self.match_token(&TokenKind::Dot) {
                if let TokenKind::Name(n) = &self
                    .expect(TokenKind::Name("".into()), "expected attr")?
                    .kind
                {
                    *expr = Expr::Attribute(Box::new(expr.clone()), n.clone());
                }
            } else if self.match_token(&TokenKind::Lbracket) {
                let idx = self.parse_expr()?;
                self.expect(TokenKind::Rbracket, "expected ']'")?;
                *expr = Expr::Subscript(Box::new(expr.clone()), Box::new(idx));
            } else {
                break;
            }
        }
        Ok(())
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.match_token(&TokenKind::Minus) {
            Ok(Expr::UnaryOp(
                crate::ast::Op::Neg,
                Box::new(self.parse_unary()?),
            ))
        } else {
            self.parse_primary()
        }
    }
    fn parse_factor(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_unary()?;
        loop {
            let op = if self.match_token(&TokenKind::Star) {
                crate::ast::Op::Mul
            } else if self.match_token(&TokenKind::Slash) {
                crate::ast::Op::Div
            } else if self.match_token(&TokenKind::Percent) {
                crate::ast::Op::Mod
            } else {
                break;
            };
            e = Expr::BinOp(op, Box::new(e), Box::new(self.parse_unary()?));
        }
        Ok(e)
    }
    fn parse_term(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_factor()?;
        loop {
            let op = if self.match_token(&TokenKind::Plus) {
                crate::ast::Op::Add
            } else if self.match_token(&TokenKind::Minus) {
                crate::ast::Op::Sub
            } else {
                break;
            };
            e = Expr::BinOp(op, Box::new(e), Box::new(self.parse_factor()?));
        }
        Ok(e)
    }
    fn parse_comp(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_term()?;
        loop {
            let op = if self.match_token(&TokenKind::Eqeq) {
                crate::ast::Op::Eq
            } else if self.match_token(&TokenKind::Ne) {
                crate::ast::Op::Ne
            } else if self.match_token(&TokenKind::Lt) {
                crate::ast::Op::Lt
            } else if self.match_token(&TokenKind::Le) {
                crate::ast::Op::Le
            } else if self.match_token(&TokenKind::Gt) {
                crate::ast::Op::Gt
            } else if self.match_token(&TokenKind::Ge) {
                crate::ast::Op::Ge
            } else {
                break;
            };
            e = Expr::Compare(op, Box::new(e), Box::new(self.parse_term()?));
        }
        Ok(e)
    }
    fn parse_not(&mut self) -> Result<Expr, String> {
        if self.match_token(&TokenKind::Not) {
            Ok(Expr::UnaryOp(
                crate::ast::Op::Not,
                Box::new(self.parse_not()?),
            ))
        } else {
            self.parse_comp()
        }
    }
    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_not()?;
        while self.match_token(&TokenKind::And) {
            e = Expr::Logical(
                crate::ast::LogicOp::And,
                Box::new(e),
                Box::new(self.parse_not()?),
            );
        }
        Ok(e)
    }

    pub(crate) fn parse_expr(&mut self) -> Result<Expr, String> {
        if self.match_token(&TokenKind::Lambda) {
            let mut p = Vec::new();
            if !self.match_token(&TokenKind::Colon) {
                loop {
                    if let TokenKind::Name(pn) = &self
                        .expect(TokenKind::Name("".into()), "expected param")?
                        .kind
                    {
                        p.push(pn.clone());
                    }
                    if !self.match_token(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::Colon, "expected ':'")?;
            }
            return Ok(Expr::Lambda(p, Box::new(self.parse_expr()?)));
        }
        let mut e = self.parse_and()?;
        while self.match_token(&TokenKind::Or) {
            e = Expr::Logical(
                crate::ast::LogicOp::Or,
                Box::new(e),
                Box::new(self.parse_and()?),
            );
        }
        Ok(e)
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        if self.match_token(&TokenKind::Newline) {
            self.expect(TokenKind::Indent, "expected indent")?;
            self.skip_newlines();
            let mut b = Vec::new();
            while self.peek().kind != TokenKind::Dedent && self.peek().kind != TokenKind::Eof {
                b.push(self.parse_stmt()?);
                self.skip_newlines();
            }
            self.expect(TokenKind::Dedent, "expected dedent")?;
            Ok(b)
        } else {
            let stmt = self.parse_stmt()?;
            Ok(vec![stmt])
        }
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        if self.match_token(&TokenKind::Import) {
            let n = self.parse_dotted_name()?;
            self.expect(TokenKind::Newline, "expected newline")?;
            return Ok(Stmt::Import(n));
        }
        if self.match_token(&TokenKind::From) {
            let mut level = 0;
            while self.match_token(&TokenKind::Dot) {
                level += 1;
            }
            let mod_n = self.parse_dotted_name()?;
            self.expect(TokenKind::Import, "expected 'import'")?;
            let mut names = Vec::new();
            loop {
                if let TokenKind::Name(n) = &self
                    .expect(TokenKind::Name("".into()), "expected name")?
                    .kind
                {
                    names.push(n.clone());
                }
                if !self.match_token(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::Newline, "expected newline")?;
            return Ok(Stmt::FromImport(mod_n, names, level));
        }
        if self.match_token(&TokenKind::Def) {
            let n = if let TokenKind::Name(n) = &self
                .expect(TokenKind::Name("".into()), "expected name")?
                .kind
            {
                n.clone()
            } else {
                unreachable!()
            };
            self.expect(TokenKind::Lparen, "expected '('")?;
            let mut p = Vec::new();
            let mut vararg = None;
            let mut kwarg = None;
            if !self.match_token(&TokenKind::Rparen) {
                loop {
                    if self.match_token(&TokenKind::Star) {
                        if self.match_token(&TokenKind::Star) {
                            if let TokenKind::Name(pn) = &self
                                .expect(TokenKind::Name("".into()), "expected kwarg name")?
                                .kind
                            {
                                kwarg = Some(pn.clone());
                            }
                            self.maybe_skip_type_annotation();
                        } else {
                            if let TokenKind::Name(pn) = &self
                                .expect(TokenKind::Name("".into()), "expected vararg name")?
                                .kind
                            {
                                vararg = Some(pn.clone());
                            }
                            self.maybe_skip_type_annotation();
                        }
                        if self.match_token(&TokenKind::Comma) {}
                        if self.peek().kind == TokenKind::Rparen {
                            break;
                        }
                        continue;
                    } else if let TokenKind::Name(pn) = &self.peek().kind.clone() {
                        self.pos += 1;
                        self.maybe_skip_type_annotation();
                        let def_val = if self.match_token(&TokenKind::Equal) {
                            Some(self.parse_expr()?)
                        } else {
                            None
                        };
                        p.push((pn.clone(), def_val));
                    } else {
                        return Err(format!(
                            "{}:{}:{}: expected parameter name",
                            self.filename,
                            self.peek().line,
                            self.peek().col
                        ));
                    }
                    if !self.match_token(&TokenKind::Comma) || self.peek().kind == TokenKind::Rparen
                    {
                        break;
                    }
                }
                self.expect(TokenKind::Rparen, "expected ')'")?;
            }
            if self.match_token(&TokenKind::Rarrow) {
                let _ = self.parse_expr();
            }
            self.expect(TokenKind::Colon, "expected ':'")?;
            return Ok(Stmt::FunctionDef(n, p, vararg, kwarg, self.parse_block()?));
        }
        if self.match_token(&TokenKind::Class) {
            let n = if let TokenKind::Name(n) = &self
                .expect(TokenKind::Name("".into()), "expected class name")?
                .kind
            {
                n.clone()
            } else {
                unreachable!()
            };
            let mut base_expr = None;
            if self.match_token(&TokenKind::Lparen) {
                base_expr = Some(self.parse_expr()?);
                self.expect(TokenKind::Rparen, "expected ')'")?;
            }
            self.expect(TokenKind::Colon, "expected ':'")?;
            return Ok(Stmt::ClassDef(n, base_expr, self.parse_block()?));
        }
        if self.match_token(&TokenKind::Try) {
            self.expect(TokenKind::Colon, "expected ':'")?;
            let body = self.parse_block()?;
            self.skip_newlines();
            let mut handlers = Vec::new();
            while self.match_token(&TokenKind::Except) {
                let mut exc_types = Vec::new();
                let mut exc_as = None;
                if self.match_token(&TokenKind::Lparen) {
                    loop {
                        if let TokenKind::Name(n) = &self
                            .expect(TokenKind::Name("".into()), "expected exc name")?
                            .kind
                        {
                            exc_types.push(n.clone());
                        }
                        if !self.match_token(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(TokenKind::Rparen, "expected ')'")?;
                } else if let TokenKind::Name(n) = &self.peek().kind.clone() {
                    exc_types.push(n.clone());
                    self.pos += 1;
                }
                if !exc_types.is_empty() && self.match_token(&TokenKind::As) {
                    if let TokenKind::Name(a) = &self
                        .expect(TokenKind::Name("".into()), "expected var")?
                        .kind
                    {
                        exc_as = Some(a.clone());
                    }
                }
                self.expect(TokenKind::Colon, "expected ':'")?;
                handlers.push((exc_types, exc_as, self.parse_block()?));
                self.skip_newlines();
            }
            if handlers.is_empty() {
                return Err("expected 'except' block".into());
            }
            return Ok(Stmt::Try(body, handlers));
        }
        if self.match_token(&TokenKind::Raise) {
            let e = self.parse_expr()?;
            self.expect(TokenKind::Newline, "expected newline")?;
            return Ok(Stmt::Raise(e));
        }
        if self.match_token(&TokenKind::If) {
            let test = self.parse_expr()?;
            self.expect(TokenKind::Colon, "expected ':'")?;
            let body = self.parse_block()?;
            self.skip_newlines();
            let mut elifs = Vec::new();
            while self.match_token(&TokenKind::Elif) {
                let t = self.parse_expr()?;
                self.expect(TokenKind::Colon, "expected ':'")?;
                elifs.push((t, self.parse_block()?));
                self.skip_newlines();
            }
            let mut els = if self.match_token(&TokenKind::Else) {
                self.expect(TokenKind::Colon, "expected ':'")?;
                self.parse_block()?
            } else {
                vec![]
            };
            for (t, b) in elifs.into_iter().rev() {
                els = vec![Stmt::If(t, b, els)];
            }
            return Ok(Stmt::If(test, body, els));
        }
        if self.match_token(&TokenKind::While) {
            let test = self.parse_expr()?;
            self.expect(TokenKind::Colon, "expected ':'")?;
            return Ok(Stmt::While(test, self.parse_block()?));
        }
        if self.match_token(&TokenKind::For) {
            let target = self.parse_expr_list()?;
            self.expect(TokenKind::In, "expected 'in'")?;
            let iter = self.parse_expr_list()?;
            self.expect(TokenKind::Colon, "expected ':'")?;
            return Ok(Stmt::For(target, iter, self.parse_block()?));
        }
        if self.match_token(&TokenKind::Return) {
            if self.match_token(&TokenKind::Newline) {
                return Ok(Stmt::Return(None));
            }
            let e = self.parse_expr_list()?;
            self.expect(TokenKind::Newline, "expected newline")?;
            return Ok(Stmt::Return(Some(e)));
        }
        if self.match_token(&TokenKind::Break) {
            self.expect(TokenKind::Newline, "expected newline")?;
            return Ok(Stmt::Break);
        }
        if self.match_token(&TokenKind::Continue) {
            self.expect(TokenKind::Newline, "expected newline")?;
            return Ok(Stmt::Continue);
        }
        if self.match_token(&TokenKind::Pass) {
            self.expect(TokenKind::Newline, "expected newline")?;
            return Ok(Stmt::Pass);
        }

        let expr = self.parse_expr_list()?;
        if self.match_token(&TokenKind::Equal)
            || self.match_token(&TokenKind::PlusEq)
            || self.match_token(&TokenKind::MinusEq)
        {
            let is_aug =
                self.prev().kind == TokenKind::PlusEq || self.prev().kind == TokenKind::MinusEq;
            let op = if self.prev().kind == TokenKind::PlusEq {
                crate::ast::Op::Add
            } else {
                crate::ast::Op::Sub
            };
            let parsed_val = self.parse_expr_list()?;
            self.expect(TokenKind::Newline, "expected newline")?;
            let final_val = if is_aug {
                if matches!(expr, Expr::Tuple(_) | Expr::List(_)) {
                    return Err("SyntaxError: illegal target for augmentation".into());
                }
                Expr::BinOp(op, Box::new(expr.clone()), Box::new(parsed_val))
            } else {
                parsed_val
            };
            return Ok(Stmt::Assign(expr, final_val));
        }
        if self.match_token(&TokenKind::Colon) {
            if self.peek().kind == TokenKind::Equal
                || self.peek().kind == TokenKind::PlusEq
                || self.peek().kind == TokenKind::MinusEq
            {
                let _ = self.parse_expr();
                let is_aug = self.match_token(&TokenKind::PlusEq)
                    || self.match_token(&TokenKind::MinusEq);
                let op = if is_aug {
                    if self.prev().kind == TokenKind::PlusEq {
                        crate::ast::Op::Add
                    } else {
                        crate::ast::Op::Sub
                    }
                } else {
                    crate::ast::Op::Add
                };
                let parsed_val = self.parse_expr_list()?;
                self.expect(TokenKind::Newline, "expected newline")?;
                let final_val = if is_aug {
                    if matches!(expr, Expr::Tuple(_) | Expr::List(_)) {
                        return Err("SyntaxError: illegal target for augmentation".into());
                    }
                    Expr::BinOp(op, Box::new(expr.clone()), Box::new(parsed_val))
                } else {
                    parsed_val
                };
                return Ok(Stmt::Assign(expr, final_val));
            } else {
                let _ = self.parse_expr();
                if self.match_token(&TokenKind::Equal)
                    || self.match_token(&TokenKind::PlusEq)
                    || self.match_token(&TokenKind::MinusEq)
                {
                    let is_aug = self.prev().kind == TokenKind::PlusEq
                        || self.prev().kind == TokenKind::MinusEq;
                    let op = if self.prev().kind == TokenKind::PlusEq {
                        crate::ast::Op::Add
                    } else {
                        crate::ast::Op::Sub
                    };
                    let parsed_val = self.parse_expr_list()?;
                    self.expect(TokenKind::Newline, "expected newline")?;
                    let final_val = if is_aug {
                        if matches!(expr, Expr::Tuple(_) | Expr::List(_)) {
                            return Err("SyntaxError: illegal target for augmentation".into());
                        }
                        Expr::BinOp(op, Box::new(expr.clone()), Box::new(parsed_val))
                    } else {
                        parsed_val
                    };
                    return Ok(Stmt::Assign(expr, final_val));
                }
            }
        }
        self.expect(TokenKind::Newline, "expected newline")?;
        Ok(Stmt::Expr(expr))
    }
    pub(crate) fn parse_module(&mut self) -> Result<Vec<Stmt>, String> {
        let mut b = Vec::new();
        self.skip_newlines();
        while self.peek().kind != TokenKind::Eof {
            b.push(self.parse_stmt()?);
            self.skip_newlines();
        }
        Ok(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens_to_expr(source: &str) -> Expr {
        let tokens = crate::lexer::lex_source(source).unwrap();
        let mut parser = Parser::new(&tokens, "<test>");
        parser.parse_expr().unwrap()
    }

    fn tokens_to_module(source: &str) -> Vec<Stmt> {
        let tokens = crate::lexer::lex_source(source).unwrap();
        let mut parser = Parser::new(&tokens, "<test>");
        parser.parse_module().unwrap()
    }

    #[test]
    fn test_parse_int() {
        let expr = tokens_to_expr("42");
        assert_eq!(expr, Expr::Int(42));
    }

    #[test]
    fn test_parse_float() {
        let expr = tokens_to_expr("3.14");
        assert_eq!(expr, Expr::Float(3.14));
    }

    #[test]
    fn test_parse_string() {
        let expr = tokens_to_expr("'hello'");
        assert_eq!(expr, Expr::String("hello".into()));
    }

    #[test]
    fn test_parse_bool_true() {
        let expr = tokens_to_expr("True");
        assert_eq!(expr, Expr::Bool(true));
    }

    #[test]
    fn test_parse_bool_false() {
        let expr = tokens_to_expr("False");
        assert_eq!(expr, Expr::Bool(false));
    }

    #[test]
    fn test_parse_none() {
        let expr = tokens_to_expr("None");
        assert_eq!(expr, Expr::NoneVal);
    }

    #[test]
    fn test_parse_name() {
        let expr = tokens_to_expr("x");
        assert_eq!(expr, Expr::Name("x".into()));
    }

    #[test]
    fn test_parse_tuple() {
        let expr = tokens_to_expr("(1, 2, 3)");
        assert_eq!(expr, Expr::Tuple(vec![Expr::Int(1), Expr::Int(2), Expr::Int(3)]));
    }

    #[test]
    fn test_parse_empty_tuple() {
        let expr = tokens_to_expr("()");
        assert_eq!(expr, Expr::Tuple(vec![]));
    }

    #[test]
    fn test_parse_single_element_tuple() {
        let expr = tokens_to_expr("(42,)");
        assert_eq!(expr, Expr::Tuple(vec![Expr::Int(42)]));
    }

    #[test]
    fn test_parse_list() {
        let expr = tokens_to_expr("[1, 2, 3]");
        assert_eq!(expr, Expr::List(vec![Expr::Int(1), Expr::Int(2), Expr::Int(3)]));
    }

    #[test]
    fn test_parse_empty_list() {
        let expr = tokens_to_expr("[]");
        assert_eq!(expr, Expr::List(vec![]));
    }

    #[test]
    fn test_parse_dict() {
        let expr = tokens_to_expr("{'a': 1, 'b': 2}");
        assert!(matches!(expr, Expr::Dict(_)));
    }

    #[test]
    fn test_parse_empty_dict() {
        let expr = tokens_to_expr("{}");
        assert!(matches!(expr, Expr::Dict(_)));
        if let Expr::Dict(pairs) = expr {
            assert!(pairs.is_empty());
        }
    }

    #[test]
    fn test_parse_add() {
        let expr = tokens_to_expr("1 + 2");
        assert_eq!(
            expr,
            Expr::BinOp(
                crate::ast::Op::Add,
                Box::new(Expr::Int(1)),
                Box::new(Expr::Int(2))
            )
        );
    }

    #[test]
    fn test_parse_subtract() {
        let expr = tokens_to_expr("5 - 3");
        assert_eq!(
            expr,
            Expr::BinOp(
                crate::ast::Op::Sub,
                Box::new(Expr::Int(5)),
                Box::new(Expr::Int(3))
            )
        );
    }

    #[test]
    fn test_parse_multiply() {
        let expr = tokens_to_expr("4 * 2");
        assert_eq!(
            expr,
            Expr::BinOp(
                crate::ast::Op::Mul,
                Box::new(Expr::Int(4)),
                Box::new(Expr::Int(2))
            )
        );
    }

    #[test]
    fn test_parse_divide() {
        let expr = tokens_to_expr("10 / 2");
        assert_eq!(
            expr,
            Expr::BinOp(
                crate::ast::Op::Div,
                Box::new(Expr::Int(10)),
                Box::new(Expr::Int(2))
            )
        );
    }

    #[test]
    fn test_parse_modulo() {
        let expr = tokens_to_expr("10 % 3");
        assert_eq!(
            expr,
            Expr::BinOp(
                crate::ast::Op::Mod,
                Box::new(Expr::Int(10)),
                Box::new(Expr::Int(3))
            )
        );
    }

    #[test]
    fn test_parse_unary_neg() {
        let expr = tokens_to_expr("-5");
        assert_eq!(
            expr,
            Expr::UnaryOp(crate::ast::Op::Neg, Box::new(Expr::Int(5)))
        );
    }

    #[test]
    fn test_parse_unary_not() {
        let expr = tokens_to_expr("not True");
        assert_eq!(
            expr,
            Expr::UnaryOp(crate::ast::Op::Not, Box::new(Expr::Bool(true)))
        );
    }

    #[test]
    fn test_parse_comparison_eq() {
        let expr = tokens_to_expr("1 == 2");
        assert_eq!(
            expr,
            Expr::Compare(
                crate::ast::Op::Eq,
                Box::new(Expr::Int(1)),
                Box::new(Expr::Int(2))
            )
        );
    }

    #[test]
    fn test_parse_comparison_ne() {
        let expr = tokens_to_expr("1 != 2");
        assert_eq!(
            expr,
            Expr::Compare(
                crate::ast::Op::Ne,
                Box::new(Expr::Int(1)),
                Box::new(Expr::Int(2))
            )
        );
    }

    #[test]
    fn test_parse_comparison_lt() {
        let expr = tokens_to_expr("1 < 2");
        assert_eq!(
            expr,
            Expr::Compare(
                crate::ast::Op::Lt,
                Box::new(Expr::Int(1)),
                Box::new(Expr::Int(2))
            )
        );
    }

    #[test]
    fn test_parse_comparison_le() {
        let expr = tokens_to_expr("1 <= 2");
        assert_eq!(
            expr,
            Expr::Compare(
                crate::ast::Op::Le,
                Box::new(Expr::Int(1)),
                Box::new(Expr::Int(2))
            )
        );
    }

    #[test]
    fn test_parse_comparison_gt() {
        let expr = tokens_to_expr("2 > 1");
        assert_eq!(
            expr,
            Expr::Compare(
                crate::ast::Op::Gt,
                Box::new(Expr::Int(2)),
                Box::new(Expr::Int(1))
            )
        );
    }

    #[test]
    fn test_parse_comparison_ge() {
        let expr = tokens_to_expr("2 >= 1");
        assert_eq!(
            expr,
            Expr::Compare(
                crate::ast::Op::Ge,
                Box::new(Expr::Int(2)),
                Box::new(Expr::Int(1))
            )
        );
    }

    #[test]
    fn test_parse_logical_and() {
        let expr = tokens_to_expr("True and False");
        assert_eq!(
            expr,
            Expr::Logical(
                crate::ast::LogicOp::And,
                Box::new(Expr::Bool(true)),
                Box::new(Expr::Bool(false))
            )
        );
    }

    #[test]
    fn test_parse_logical_or() {
        let expr = tokens_to_expr("True or False");
        assert_eq!(
            expr,
            Expr::Logical(
                crate::ast::LogicOp::Or,
                Box::new(Expr::Bool(true)),
                Box::new(Expr::Bool(false))
            )
        );
    }

    #[test]
    fn test_parse_precedence_mul_over_add() {
        let expr = tokens_to_expr("1 + 2 * 3");
        assert_eq!(
            expr,
            Expr::BinOp(
                crate::ast::Op::Add,
                Box::new(Expr::Int(1)),
                Box::new(Expr::BinOp(
                    crate::ast::Op::Mul,
                    Box::new(Expr::Int(2)),
                    Box::new(Expr::Int(3))
                ))
            )
        );
    }

    #[test]
    fn test_parse_lambda() {
        let expr = tokens_to_expr("lambda x: x + 1");
        assert_eq!(
            expr,
            Expr::Lambda(
                vec!["x".into()],
                Box::new(Expr::BinOp(
                    crate::ast::Op::Add,
                    Box::new(Expr::Name("x".into())),
                    Box::new(Expr::Int(1))
                ))
            )
        );
    }

    #[test]
    fn test_parse_lambda_multi_param() {
        let expr = tokens_to_expr("lambda x, y: x + y");
        assert_eq!(
            expr,
            Expr::Lambda(
                vec!["x".into(), "y".into()],
                Box::new(Expr::BinOp(
                    crate::ast::Op::Add,
                    Box::new(Expr::Name("x".into())),
                    Box::new(Expr::Name("y".into()))
                ))
            )
        );
    }

    #[test]
    fn test_parse_function_call() {
        let expr = tokens_to_expr("foo()");
        assert!(matches!(expr, Expr::Call(_, _, _)));
    }

    #[test]
    fn test_parse_function_call_with_args() {
        let expr = tokens_to_expr("foo(1, 2, 3)");
        if let Expr::Call(func, args, _) = expr {
            assert_eq!(func.as_ref(), &Expr::Name("foo".into()));
            assert_eq!(args.len(), 3);
        } else {
            panic!("Expected Call expression");
        }
    }

    #[test]
    fn test_parse_attribute_access() {
        let expr = tokens_to_expr("obj.attr");
        assert_eq!(
            expr,
            Expr::Attribute(Box::new(Expr::Name("obj".into())), "attr".into())
        );
    }

    #[test]
    fn test_parse_subscript() {
        let expr = tokens_to_expr("arr[0]");
        if let Expr::Subscript(obj, idx) = expr {
            assert_eq!(obj.as_ref(), &Expr::Name("arr".into()));
            assert_eq!(idx.as_ref(), &Expr::Int(0));
        } else {
            panic!("Expected Subscript expression");
        }
    }

    #[test]
    fn test_parse_list_comprehension() {
        let expr = tokens_to_expr("[x for x in items]");
        assert!(matches!(expr, Expr::ListComp(_, _, _, _)));
    }

    #[test]
    fn test_parse_list_comprehension_with_filter() {
        let expr = tokens_to_expr("[x for x in items if x > 0]");
        assert!(matches!(expr, Expr::ListComp(_, _, _, Some(_))));
    }

    #[test]
    fn test_parse_assignment() {
        let stmts = tokens_to_module("x = 5");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(stmts[0], Stmt::Assign(_, _)));
    }

    #[test]
    fn test_parse_if_statement() {
        let stmts = tokens_to_module("if x:\n    pass");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::If(_, _, _)));
    }

    #[test]
    fn test_parse_while_statement() {
        let stmts = tokens_to_module("while True:\n    pass");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::While(_, _)));
    }

    #[test]
    fn test_parse_for_statement() {
        let stmts = tokens_to_module("for x in items:\n    pass");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::For(_, _, _)));
    }

    #[test]
    fn test_parse_function_def() {
        let stmts = tokens_to_module("def foo():\n    pass");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::FunctionDef(_, _, _, _, _)));
    }

    #[test]
    fn test_parse_function_def_with_params() {
        let stmts = tokens_to_module("def foo(x, y):\n    pass");
        if let Stmt::FunctionDef(name, params, _, _, _) = &stmts[0] {
            assert_eq!(name, "foo");
            assert_eq!(params.len(), 2);
        } else {
            panic!("Expected FunctionDef");
        }
    }

    #[test]
    fn test_parse_function_def_with_return_type() {
        let stmts = tokens_to_module("def foo() -> int:\n    pass");
        if let Stmt::FunctionDef(name, _, _, _, _) = &stmts[0] {
            assert_eq!(name, "foo");
        } else {
            panic!("Expected FunctionDef");
        }
    }

    #[test]
    fn test_parse_class_def() {
        let stmts = tokens_to_module("class Foo:\n    pass");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::ClassDef(_, _, _)));
    }

    #[test]
    fn test_parse_class_def_with_base() {
        let stmts = tokens_to_module("class Foo(Bar):\n    pass");
        if let Stmt::ClassDef(name, base, _) = &stmts[0] {
            assert_eq!(name, "Foo");
            assert!(base.is_some());
        } else {
            panic!("Expected ClassDef");
        }
    }

    #[test]
    fn test_parse_return() {
        let stmts = tokens_to_module("return 42");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::Return(_)));
    }

    #[test]
    fn test_parse_return_none() {
        let stmts = tokens_to_module("return");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::Return(None)));
    }

    #[test]
    fn test_parse_break() {
        let stmts = tokens_to_module("break");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::Break));
    }

    #[test]
    fn test_parse_continue() {
        let stmts = tokens_to_module("continue");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::Continue));
    }

    #[test]
    fn test_parse_pass() {
        let stmts = tokens_to_module("pass");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::Pass));
    }

    #[test]
    fn test_parse_try_except() {
        let stmts = tokens_to_module("try:\n    pass\nexcept:\n    pass");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::Try(_, _)));
    }

    #[test]
    fn test_parse_raise() {
        let stmts = tokens_to_module("raise Exception('error')");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::Raise(_)));
    }

    #[test]
    fn test_parse_import() {
        let stmts = tokens_to_module("import os");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::Import(_)));
    }

    #[test]
    fn test_parse_from_import() {
        let stmts = tokens_to_module("from os import path");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::FromImport(_, _, _)));
    }

    #[test]
    fn test_parse_augmented_assignment_plus() {
        let stmts = tokens_to_module("x += 1");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::Assign(Expr::Name(_), _)));
    }

    #[test]
    fn test_parse_augmented_assignment_minus() {
        let stmts = tokens_to_module("x -= 1");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::Assign(Expr::Name(_), _)));
    }

    #[test]
    fn test_parse_multiple_statements() {
        let stmts = tokens_to_module("x = 1\ny = 2\nz = 3");
        assert_eq!(stmts.len(), 3);
    }

    #[test]
    fn test_parse_expression_statement() {
        let stmts = tokens_to_module("foo()");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::Expr(_)));
    }

    #[test]
    fn test_parse_nested_call() {
        let expr = tokens_to_expr("foo(bar(x))");
        if let Expr::Call(_, args, _) = expr {
            if let Expr::Call(_, _, _) = args[0] {
                // OK
            } else {
                panic!("Expected nested call");
            }
        } else {
            panic!("Expected Call expression");
        }
    }

    #[test]
    fn test_parse_chained_attribute() {
        let expr = tokens_to_expr("a.b.c");
        let expected = Expr::Attribute(
            Box::new(Expr::Attribute(Box::new(Expr::Name("a".into())), "b".into())),
            "c".into(),
        );
        assert_eq!(expr, expected);
    }

    #[test]
    fn test_parse_chained_comparison() {
        let expr = tokens_to_expr("1 < 2 < 3");
        if let Expr::Compare(op, _left, _right) = expr {
            assert_eq!(op, crate::ast::Op::Lt);
        } else {
            panic!("Expected Compare expression, got {:?}", expr);
        }
    }
}