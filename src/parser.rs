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
            return Ok(Stmt::FromImport(mod_n, names));
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
                        } else {
                            if let TokenKind::Name(pn) = &self
                                .expect(TokenKind::Name("".into()), "expected vararg name")?
                                .kind
                            {
                                vararg = Some(pn.clone());
                            }
                        }
                        if self.match_token(&TokenKind::Comma) {}
                        if self.peek().kind == TokenKind::Rparen {
                            break;
                        }
                        continue;
                    } else if let TokenKind::Name(pn) = &self.peek().kind.clone() {
                        self.pos += 1;
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