//! Parser for the Prism format
//!
//! Converts .prism text files into an AST.
//! The parser is hand-written for simplicity and zero dependencies.

use crate::ast::*;
use std::collections::HashMap;
use std::iter::Peekable;
use std::str::Chars;

pub struct Parser<'a> {
    input: &'a str,
    chars: Peekable<Chars<'a>>,
    pos: usize,
    line: usize,
    col: usize,
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parse error at {}:{}: {}", self.line, self.col, self.message)
    }
}

type Result<T> = std::result::Result<T, ParseError>;

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.chars().peekable(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn parse(mut self) -> Result<PrismApp> {
        let mut name = String::from("Untitled");
        let mut version = 1u32;
        let mut state = StateBlock::default();
        let mut view = ViewNode {
            kind: NodeKind::Column,
            props: HashMap::new(),
            children: vec![],
        };
        let mut actions = HashMap::new();

        self.skip_whitespace_and_comments();

        while self.peek().is_some() {
            self.skip_whitespace_and_comments();
            
            if self.peek() == Some('@') {
                self.advance();
                let directive = self.parse_identifier()?;
                self.skip_horizontal_whitespace();
                
                match directive.as_str() {
                    "app" => {
                        name = self.parse_string_literal()?;
                    }
                    "version" => {
                        let v = self.parse_number()?;
                        version = v.as_int() as u32;
                    }
                    _ => {
                        return Err(self.error(&format!("Unknown directive: @{}", directive)));
                    }
                }
            } else if self.check_keyword("state") {
                self.consume_keyword("state")?;
                state = self.parse_state_block()?;
            } else if self.check_keyword("view") {
                self.consume_keyword("view")?;
                view = self.parse_view_block()?;
            } else if self.check_keyword("actions") {
                self.consume_keyword("actions")?;
                actions = self.parse_actions_block()?;
            } else if self.peek() == Some('-') {
                // Comment line like "-- State Declaration --"
                self.skip_line();
            } else if self.peek().map(|c| c.is_whitespace()).unwrap_or(true) {
                self.advance();
            } else {
                let c = self.peek().unwrap_or(' ');
                return Err(self.error(&format!("Unexpected character: '{}'", c)));
            }
            
            self.skip_whitespace_and_comments();
        }

        Ok(PrismApp {
            name,
            version,
            imports: vec![],
            state,
            computed: HashMap::new(),
            components: HashMap::new(),
            view,
            actions,
            routes: HashMap::new(),
        })
    }

    fn parse_state_block(&mut self) -> Result<StateBlock> {
        self.skip_whitespace_and_comments();
        self.expect('{')?;
        self.skip_whitespace_and_comments();

        let mut fields = HashMap::new();

        while self.peek() != Some('}') {
            self.skip_whitespace_and_comments();
            if self.peek() == Some('}') {
                break;
            }

            let field_name = self.parse_identifier()?;
            self.skip_horizontal_whitespace();
            self.expect(':')?;
            self.skip_horizontal_whitespace();
            let value = self.parse_value()?;
            fields.insert(field_name, value);
            
            self.skip_whitespace_and_comments();
        }

        self.expect('}')?;
        Ok(StateBlock { fields })
    }

    fn parse_view_block(&mut self) -> Result<ViewNode> {
        self.skip_whitespace_and_comments();
        self.expect('{')?;
        self.skip_whitespace_and_comments();

        let node = self.parse_view_node()?;

        self.skip_whitespace_and_comments();
        self.expect('}')?;

        Ok(node)
    }

    fn parse_view_node(&mut self) -> Result<ViewNode> {
        self.skip_whitespace_and_comments();
        
        let kind_str = self.parse_identifier()?;
        let kind = match kind_str.as_str() {
            // Layout
            "column" => NodeKind::Column,
            "row" => NodeKind::Row,
            "stack" => NodeKind::Stack,
            "grid" => NodeKind::Grid,
            "scroll" => NodeKind::Scroll,
            "center" => NodeKind::Center,
            // Basic
            "box" => NodeKind::Box,
            "spacer" => NodeKind::Spacer,
            "divider" => NodeKind::Divider,
            // Text
            "text" => NodeKind::Text,
            "link" => NodeKind::Link,
            "markdown" => NodeKind::Markdown,
            // Interactive
            "button" => NodeKind::Button,
            "input" => NodeKind::Input,
            "textarea" => NodeKind::TextArea,
            "checkbox" => NodeKind::Checkbox,
            "radio" => NodeKind::Radio,
            "select" => NodeKind::Select,
            "slider" => NodeKind::Slider,
            "toggle" => NodeKind::Toggle,
            // Media
            "image" => NodeKind::Image,
            "icon" => NodeKind::Icon,
            "video" => NodeKind::Video,
            "audio" => NodeKind::Audio,
            // Data Display
            "table" => NodeKind::Table,
            "list" => NodeKind::List,
            "card" => NodeKind::Card,
            "badge" => NodeKind::Badge,
            "progress" => NodeKind::Progress,
            "avatar" => NodeKind::Avatar,
            // Feedback
            "modal" => NodeKind::Modal,
            "toast" => NodeKind::Toast,
            "tooltip" => NodeKind::Tooltip,
            "popover" => NodeKind::Popover,
            // Control Flow
            "each" => NodeKind::Each,
            "if" => NodeKind::If,
            "show" => NodeKind::Show,
            "switch" => NodeKind::Switch,
            "slot" => NodeKind::Slot,
            // Custom component
            _ => NodeKind::Component(kind_str.clone()),
        };

        self.skip_horizontal_whitespace();

        // Optional inline text content
        let mut props = HashMap::new();
        if self.peek() == Some('"') {
            let content = self.parse_string_literal()?;
            // Check if it contains interpolation
            if content.contains('{') && content.contains('}') {
                props.insert("content".to_string(), PropValue::Expression(
                    self.parse_interpolation(&content)?
                ));
            } else {
                props.insert("content".to_string(), PropValue::Static(Value::String(content)));
            }
            self.skip_horizontal_whitespace();
        }

        let mut children = vec![];

        // Optional property block
        if self.peek() == Some('{') {
            self.advance();
            self.skip_whitespace_and_comments();

            while self.peek() != Some('}') {
                self.skip_whitespace_and_comments();
                if self.peek() == Some('}') {
                    break;
                }

                // Check if this is a child node or a property
                let saved_pos = self.pos;
                let saved_line = self.line;
                let saved_col = self.col;
                let saved_chars = self.chars.clone();

                let ident = self.parse_identifier()?;
                self.skip_horizontal_whitespace();

                if self.is_node_kind(&ident) || self.peek() == Some('"') && self.is_node_kind(&ident) {
                    // This is a child node, restore position and parse as node
                    self.pos = saved_pos;
                    self.line = saved_line;
                    self.col = saved_col;
                    self.chars = saved_chars;
                    
                    let child = self.parse_view_node()?;
                    children.push(child);
                } else if self.peek() == Some(':') {
                    // This is a property
                    self.advance();
                    self.skip_horizontal_whitespace();
                    let prop_value = self.parse_prop_value()?;
                    props.insert(ident, prop_value);
                } else if self.peek() == Some('"') || self.peek() == Some('{') {
                    // This is a child node with content
                    self.pos = saved_pos;
                    self.line = saved_line;
                    self.col = saved_col;
                    self.chars = saved_chars;
                    
                    let child = self.parse_view_node()?;
                    children.push(child);
                } else {
                    return Err(self.error(&format!("Expected ':' after property name '{}' or a child node", ident)));
                }

                self.skip_whitespace_and_comments();
            }

            self.expect('}')?;
        }

        Ok(ViewNode { kind, props, children })
    }

    fn is_node_kind(&self, s: &str) -> bool {
        matches!(s, "column" | "row" | "text" | "button" | "input" | "box" | "spacer" | 
            "stack" | "grid" | "scroll" | "center" | "divider" | "link" | "markdown" |
            "textarea" | "checkbox" | "radio" | "select" | "slider" | "toggle" |
            "image" | "icon" | "video" | "audio" | "table" | "list" | "card" |
            "badge" | "progress" | "avatar" | "modal" | "toast" | "tooltip" | "popover" |
            "each" | "if" | "show" | "switch" | "slot")
    }

    fn parse_prop_value(&mut self) -> Result<PropValue> {
        self.skip_horizontal_whitespace();

        if self.peek() == Some('#') {
            // Color
            self.advance();
            let mut hex = String::new();
            while self.peek().map(|c| c.is_ascii_hexdigit()).unwrap_or(false) {
                hex.push(self.advance().unwrap());
            }
            let color = Color::from_hex(&hex)
                .ok_or_else(|| self.error(&format!("Invalid hex color: #{}", hex)))?;
            return Ok(PropValue::Color(color));
        }

        if self.peek() == Some('"') {
            let s = self.parse_string_literal()?;
            if s.contains('{') && s.contains('}') {
                return Ok(PropValue::Expression(self.parse_interpolation(&s)?));
            }
            return Ok(PropValue::Static(Value::String(s)));
        }

        // Try to parse as expression or identifier
        let expr = self.parse_expression()?;
        
        // Check if it's a simple identifier (could be action handler)
        if let Expression::Variable(name) = &expr {
            // If it contains operators, treat as expression, otherwise as handler
            return Ok(PropValue::Handler(name.clone()));
        }

        Ok(PropValue::Expression(expr))
    }

    fn parse_expression(&mut self) -> Result<Expression> {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> Result<Expression> {
        let mut left = self.parse_and_expr()?;
        
        self.skip_horizontal_whitespace();
        while self.check_keyword("or") {
            self.consume_keyword("or")?;
            self.skip_horizontal_whitespace();
            let right = self.parse_and_expr()?;
            left = Expression::Binary {
                left: Box::new(left),
                op: BinaryOp::Or,
                right: Box::new(right),
            };
            self.skip_horizontal_whitespace();
        }
        
        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<Expression> {
        let mut left = self.parse_comparison()?;
        
        self.skip_horizontal_whitespace();
        while self.check_keyword("and") {
            self.consume_keyword("and")?;
            self.skip_horizontal_whitespace();
            let right = self.parse_comparison()?;
            left = Expression::Binary {
                left: Box::new(left),
                op: BinaryOp::And,
                right: Box::new(right),
            };
            self.skip_horizontal_whitespace();
        }
        
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expression> {
        let mut left = self.parse_additive()?;
        
        self.skip_horizontal_whitespace();
        loop {
            let op = if self.try_consume("==") {
                Some(BinaryOp::Eq)
            } else if self.try_consume("!=") {
                Some(BinaryOp::Ne)
            } else if self.try_consume("<=") {
                Some(BinaryOp::Le)
            } else if self.try_consume(">=") {
                Some(BinaryOp::Ge)
            } else if self.try_consume("<") {
                Some(BinaryOp::Lt)
            } else if self.try_consume(">") {
                Some(BinaryOp::Gt)
            } else {
                None
            };
            
            if let Some(op) = op {
                self.skip_horizontal_whitespace();
                let right = self.parse_additive()?;
                left = Expression::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                };
                self.skip_horizontal_whitespace();
            } else {
                break;
            }
        }
        
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expression> {
        let mut left = self.parse_multiplicative()?;
        
        self.skip_horizontal_whitespace();
        loop {
            let op = if self.peek() == Some('+') {
                self.advance();
                Some(BinaryOp::Add)
            } else if self.peek() == Some('-') {
                self.advance();
                Some(BinaryOp::Sub)
            } else {
                None
            };
            
            if let Some(op) = op {
                self.skip_horizontal_whitespace();
                let right = self.parse_multiplicative()?;
                left = Expression::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                };
                self.skip_horizontal_whitespace();
            } else {
                break;
            }
        }
        
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expression> {
        let mut left = self.parse_primary()?;
        
        self.skip_horizontal_whitespace();
        loop {
            let op = if self.peek() == Some('*') {
                self.advance();
                Some(BinaryOp::Mul)
            } else if self.peek() == Some('/') {
                self.advance();
                Some(BinaryOp::Div)
            } else {
                None
            };
            
            if let Some(op) = op {
                self.skip_horizontal_whitespace();
                let right = self.parse_primary()?;
                left = Expression::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                };
                self.skip_horizontal_whitespace();
            } else {
                break;
            }
        }
        
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expression> {
        self.skip_horizontal_whitespace();
        
        if self.peek() == Some('"') {
            let s = self.parse_string_literal()?;
            return Ok(Expression::Literal(Value::String(s)));
        }

        if self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            let n = self.parse_number()?;
            return Ok(Expression::Literal(n));
        }

        if self.check_keyword("true") {
            self.consume_keyword("true")?;
            return Ok(Expression::Literal(Value::Bool(true)));
        }

        if self.check_keyword("false") {
            self.consume_keyword("false")?;
            return Ok(Expression::Literal(Value::Bool(false)));
        }

        if self.peek() == Some('(') {
            self.advance();
            let expr = self.parse_expression()?;
            self.skip_horizontal_whitespace();
            self.expect(')')?;
            return Ok(expr);
        }

        // Variable
        let name = self.parse_identifier()?;
        Ok(Expression::Variable(name))
    }

    fn parse_interpolation(&self, s: &str) -> Result<Expression> {
        let mut parts = vec![];
        let mut current = String::new();
        let mut in_var = false;
        let mut var_name = String::new();

        for c in s.chars() {
            if c == '{' && !in_var {
                if !current.is_empty() {
                    parts.push(InterpolationPart::Literal(current.clone()));
                    current.clear();
                }
                in_var = true;
            } else if c == '}' && in_var {
                if !var_name.is_empty() {
                    parts.push(InterpolationPart::Expression(Box::new(Expression::Variable(var_name.clone()))));
                    var_name.clear();
                }
                in_var = false;
            } else if in_var {
                var_name.push(c);
            } else {
                current.push(c);
            }
        }

        if !current.is_empty() {
            parts.push(InterpolationPart::Literal(current));
        }

        Ok(Expression::Interpolation(parts))
    }

    fn parse_actions_block(&mut self) -> Result<HashMap<String, ActionBlock>> {
        self.skip_whitespace_and_comments();
        self.expect('{')?;
        self.skip_whitespace_and_comments();

        let mut actions = HashMap::new();

        while self.peek() != Some('}') {
            self.skip_whitespace_and_comments();
            if self.peek() == Some('}') {
                break;
            }

            let name = self.parse_identifier()?;
            self.skip_whitespace_and_comments();
            self.expect('{')?;
            self.skip_whitespace_and_comments();

            let mut statements = vec![];

            while self.peek() != Some('}') {
                self.skip_whitespace_and_comments();
                if self.peek() == Some('}') {
                    break;
                }

                let target = self.parse_identifier()?;
                self.skip_horizontal_whitespace();
                self.expect(':')?;
                self.skip_horizontal_whitespace();
                let value = self.parse_expression()?;
                statements.push(Statement::Assign {
                    target: AssignTarget::Variable(target),
                    value,
                });

                self.skip_whitespace_and_comments();
            }

            self.expect('}')?;
            actions.insert(name, ActionBlock { params: vec![], statements });

            self.skip_whitespace_and_comments();
        }

        self.expect('}')?;
        Ok(actions)
    }

    fn parse_value(&mut self) -> Result<Value> {
        self.skip_horizontal_whitespace();

        if self.peek() == Some('"') {
            let s = self.parse_string_literal()?;
            return Ok(Value::String(s));
        }

        if self.check_keyword("true") {
            self.consume_keyword("true")?;
            return Ok(Value::Bool(true));
        }

        if self.check_keyword("false") {
            self.consume_keyword("false")?;
            return Ok(Value::Bool(false));
        }

        if self.check_keyword("null") {
            self.consume_keyword("null")?;
            return Ok(Value::Null);
        }

        // List literal: [1, 2, 3]
        if self.peek() == Some('[') {
            return self.parse_list_value();
        }

        // Object literal: { key: value }
        if self.peek() == Some('{') {
            return self.parse_object_value();
        }

        if self.peek().map(|c| c.is_ascii_digit() || c == '-').unwrap_or(false) {
            return self.parse_number();
        }

        Err(self.error("Expected a value"))
    }

    fn parse_list_value(&mut self) -> Result<Value> {
        self.expect('[')?;
        self.skip_whitespace_and_comments();

        let mut items = vec![];

        while self.peek() != Some(']') {
            self.skip_whitespace_and_comments();
            if self.peek() == Some(']') {
                break;
            }

            let value = self.parse_value()?;
            items.push(value);

            self.skip_whitespace_and_comments();
            if self.peek() == Some(',') {
                self.advance();
            }
            self.skip_whitespace_and_comments();
        }

        self.expect(']')?;
        Ok(Value::List(items))
    }

    fn parse_object_value(&mut self) -> Result<Value> {
        self.expect('{')?;
        self.skip_whitespace_and_comments();

        let mut map = std::collections::HashMap::new();

        while self.peek() != Some('}') {
            self.skip_whitespace_and_comments();
            if self.peek() == Some('}') {
                break;
            }

            let key = self.parse_identifier()?;
            self.skip_horizontal_whitespace();
            self.expect(':')?;
            self.skip_horizontal_whitespace();
            let value = self.parse_value()?;
            map.insert(key, value);

            self.skip_whitespace_and_comments();
            if self.peek() == Some(',') {
                self.advance();
            }
            self.skip_whitespace_and_comments();
        }

        self.expect('}')?;
        Ok(Value::Object(map))
    }

    fn parse_string_literal(&mut self) -> Result<String> {
        self.expect('"')?;
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == '"' {
                break;
            }
            if c == '\\' {
                self.advance();
                match self.advance() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some(c) => s.push(c),
                    None => return Err(self.error("Unexpected end of input in string")),
                }
            } else {
                s.push(self.advance().unwrap());
            }
        }
        self.expect('"')?;
        Ok(s)
    }

    fn parse_number(&mut self) -> Result<Value> {
        let mut s = String::new();
        let mut is_float = false;

        if self.peek() == Some('-') {
            s.push(self.advance().unwrap());
        }

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(self.advance().unwrap());
            } else if c == '.' && !is_float {
                is_float = true;
                s.push(self.advance().unwrap());
            } else {
                break;
            }
        }

        if is_float {
            let f: f64 = s.parse().map_err(|_| self.error("Invalid float"))?;
            Ok(Value::Float(f))
        } else {
            let i: i64 = s.parse().map_err(|_| self.error("Invalid integer"))?;
            Ok(Value::Int(i))
        }
    }

    fn parse_identifier(&mut self) -> Result<String> {
        let mut s = String::new();
        
        if let Some(c) = self.peek() {
            if c.is_alphabetic() || c == '_' {
                s.push(self.advance().unwrap());
            } else {
                return Err(self.error(&format!("Expected identifier, found '{}'", c)));
            }
        } else {
            return Err(self.error("Expected identifier, found end of input"));
        }

        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(self.advance().unwrap());
            } else {
                break;
            }
        }

        Ok(s)
    }

    fn check_keyword(&self, kw: &str) -> bool {
        self.input[self.pos..].starts_with(kw)
            && self.input[self.pos..].chars().nth(kw.len()).map(|c| !c.is_alphanumeric() && c != '_').unwrap_or(true)
    }

    fn consume_keyword(&mut self, kw: &str) -> Result<()> {
        if self.check_keyword(kw) {
            for _ in 0..kw.len() {
                self.advance();
            }
            Ok(())
        } else {
            Err(self.error(&format!("Expected keyword '{}'", kw)))
        }
    }

    fn try_consume(&mut self, s: &str) -> bool {
        if self.input[self.pos..].starts_with(s) {
            for _ in 0..s.len() {
                self.advance();
            }
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.input[self.pos..].chars().next()?;
        self.pos += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn expect(&mut self, expected: char) -> Result<()> {
        match self.peek() {
            Some(c) if c == expected => {
                self.advance();
                Ok(())
            }
            Some(c) => Err(self.error(&format!("Expected '{}', found '{}'", expected, c))),
            None => Err(self.error(&format!("Expected '{}', found end of input", expected))),
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            self.skip_horizontal_whitespace();
            
            // Skip newlines
            while self.peek() == Some('\n') || self.peek() == Some('\r') {
                self.advance();
                self.skip_horizontal_whitespace();
            }
            
            // Skip line comments
            if self.peek() == Some('-') && self.input[self.pos..].starts_with("--") {
                self.skip_line();
            } else {
                break;
            }
        }
    }

    fn skip_horizontal_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_line(&mut self) {
        while let Some(c) = self.peek() {
            self.advance();
            if c == '\n' {
                break;
            }
        }
    }

    fn error(&self, message: &str) -> ParseError {
        ParseError {
            message: message.to_string(),
            line: self.line,
            col: self.col,
        }
    }
}

pub fn parse(input: &str) -> Result<PrismApp> {
    Parser::new(input).parse()
}
