#![allow(dead_code)]
//! Abstract Syntax Tree definitions for Prism
//!
//! The AST represents the parsed structure of a .prism file.
//! Extended for production use with lists, objects, components, and more.

use std::collections::HashMap;
use std::fmt;

// ============================================================================
// CORE APPLICATION STRUCTURE
// ============================================================================

/// Root of a Prism application
#[derive(Debug, Clone)]
pub struct PrismApp {
    pub name: String,
    pub version: u32,
    pub imports: Vec<Import>,
    pub state: StateBlock,
    pub computed: HashMap<String, Expression>,
    pub components: HashMap<String, ComponentDef>,
    pub view: ViewNode,
    pub actions: HashMap<String, ActionBlock>,
    pub routes: HashMap<String, ViewNode>,
}

impl Default for PrismApp {
    fn default() -> Self {
        Self {
            name: "Untitled".to_string(),
            version: 1,
            imports: vec![],
            state: StateBlock::default(),
            computed: HashMap::new(),
            components: HashMap::new(),
            view: ViewNode {
                kind: NodeKind::Column,
                props: HashMap::new(),
                children: vec![],
            },
            actions: HashMap::new(),
            routes: HashMap::new(),
        }
    }
}

/// Import statement for modules
#[derive(Debug, Clone)]
pub struct Import {
    pub path: String,
    pub alias: Option<String>,
}

// ============================================================================
// STATE MANAGEMENT
// ============================================================================

/// State declaration block
#[derive(Debug, Clone, Default)]
pub struct StateBlock {
    pub fields: HashMap<String, Value>,
}

/// A value in the Prism type system - now with Lists and Objects
#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
    Object(HashMap<String, Value>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => (a - b).abs() < f64::EPSILON,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => a == b,
            _ => false,
        }
    }
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::List(_) => "list",
            Value::Object(_) => "object",
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            Value::Null => "".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => {
                if f.fract() == 0.0 {
                    format!("{:.0}", f)
                } else {
                    f.to_string()
                }
            }
            Value::String(s) => s.clone(),
            Value::List(items) => {
                let strs: Vec<String> = items.iter().map(|v| v.as_string()).collect();
                format!("[{}]", strs.join(", "))
            }
            Value::Object(map) => {
                let pairs: Vec<String> = map
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.as_string()))
                    .collect();
                format!("{{{}}}", pairs.join(", "))
            }
        }
    }

    pub fn as_int(&self) -> i64 {
        match self {
            Value::Int(i) => *i,
            Value::Float(f) => *f as i64,
            Value::String(s) => s.parse().unwrap_or(0),
            Value::Bool(b) => if *b { 1 } else { 0 },
            Value::List(l) => l.len() as i64,
            Value::Null | Value::Object(_) => 0,
        }
    }

    pub fn as_float(&self) -> f64 {
        match self {
            Value::Int(i) => *i as f64,
            Value::Float(f) => *f,
            Value::String(s) => s.parse().unwrap_or(0.0),
            Value::Bool(b) => if *b { 1.0 } else { 0.0 },
            Value::List(l) => l.len() as f64,
            Value::Null | Value::Object(_) => 0.0,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
            Value::Object(o) => !o.is_empty(),
            Value::Null => false,
        }
    }

    pub fn as_list(&self) -> Vec<Value> {
        match self {
            Value::List(l) => l.clone(),
            Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
            _ => vec![self.clone()],
        }
    }

    pub fn get(&self, key: &Value) -> Value {
        match (self, key) {
            (Value::List(list), Value::Int(idx)) => {
                let idx = if *idx < 0 {
                    (list.len() as i64 + idx) as usize
                } else {
                    *idx as usize
                };
                list.get(idx).cloned().unwrap_or(Value::Null)
            }
            (Value::Object(map), Value::String(key)) => {
                map.get(key).cloned().unwrap_or(Value::Null)
            }
            (Value::String(s), Value::Int(idx)) => {
                let idx = if *idx < 0 {
                    (s.len() as i64 + idx) as usize
                } else {
                    *idx as usize
                };
                s.chars()
                    .nth(idx)
                    .map(|c| Value::String(c.to_string()))
                    .unwrap_or(Value::Null)
            }
            _ => Value::Null,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Value::List(l) => l.len(),
            Value::String(s) => s.len(),
            Value::Object(o) => o.len(),
            _ => 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_string())
    }
}

// ============================================================================
// COMPONENTS
// ============================================================================

/// Component definition - reusable UI pieces
#[derive(Debug, Clone)]
pub struct ComponentDef {
    pub name: String,
    pub props: Vec<PropDef>,
    pub state: StateBlock,
    pub view: ViewNode,
    pub actions: HashMap<String, ActionBlock>,
}

/// Property definition for components
#[derive(Debug, Clone)]
pub struct PropDef {
    pub name: String,
    pub default: Option<Value>,
    pub required: bool,
}

// ============================================================================
// VIEW TREE
// ============================================================================

/// A node in the view tree
#[derive(Debug, Clone)]
pub struct ViewNode {
    pub kind: NodeKind,
    pub props: HashMap<String, PropValue>,
    pub children: Vec<ViewNode>,
}

/// Types of view nodes - extended for real applications
#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    // Layout
    Column,
    Row,
    Stack,
    Grid,
    Scroll,
    Center,
    
    // Basic
    Box,
    Spacer,
    Divider,
    
    // Text
    Text,
    Link,
    Markdown,
    
    // Interactive
    Button,
    Input,
    TextArea,
    Checkbox,
    Radio,
    Select,
    Slider,
    Toggle,
    
    // Media
    Image,
    Icon,
    Video,
    Audio,
    
    // Data Display
    Table,
    List,
    Card,
    Badge,
    Progress,
    Avatar,
    
    // Feedback
    Modal,
    Toast,
    Tooltip,
    Popover,
    
    // Control Flow
    Each,       // List iteration
    If,         // Conditional rendering
    Show,       // Visibility toggle (keeps in DOM)
    Switch,     // Multi-branch conditional
    Slot,       // Component slot
    
    // Custom
    Component(String),  // User-defined component
}

/// Property values can be static, dynamic, or handlers
#[derive(Debug, Clone)]
pub enum PropValue {
    Static(Value),
    Expression(Expression),
    Color(Color),
    Handler(String),
    EventHandler(EventHandler),
}

/// Event handler with optional parameters
#[derive(Debug, Clone)]
pub struct EventHandler {
    pub action: String,
    pub args: Vec<Expression>,
}

// ============================================================================
// STYLING
// ============================================================================

/// Colors in Prism
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Color = Color { r: 0, g: 0, b: 0, a: 0 };
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255, a: 255 };
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0, a: 255 };
    pub const GRAY: Color = Color { r: 128, g: 128, b: 128, a: 255 };
    pub const LIGHT_GRAY: Color = Color { r: 200, g: 200, b: 200, a: 255 };
    pub const DARK_GRAY: Color = Color { r: 64, g: 64, b: 64, a: 255 };
    pub const RED: Color = Color { r: 244, g: 67, b: 54, a: 255 };
    pub const GREEN: Color = Color { r: 76, g: 175, b: 80, a: 255 };
    pub const BLUE: Color = Color { r: 33, g: 150, b: 243, a: 255 };
    pub const YELLOW: Color = Color { r: 255, g: 235, b: 59, a: 255 };
    pub const ORANGE: Color = Color { r: 255, g: 152, b: 0, a: 255 };
    pub const PURPLE: Color = Color { r: 156, g: 39, b: 176, a: 255 };
    pub const CYAN: Color = Color { r: 0, g: 188, b: 212, a: 255 };
    pub const PINK: Color = Color { r: 233, g: 30, b: 99, a: 255 };

    pub fn from_hex(hex: &str) -> Option<Color> {
        let hex = hex.trim_start_matches('#');
        match hex.len() {
            8 => {
                // RRGGBBAA
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Color { r, g, b, a })
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Color { r, g, b, a: 255 })
            }
            4 => {
                // RGBA shorthand
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                let a = u8::from_str_radix(&hex[3..4], 16).ok()? * 17;
                Some(Color { r, g, b, a })
            }
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                Some(Color { r, g, b, a: 255 })
            }
            _ => None,
        }
    }

    pub fn from_rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b, a: 255 }
    }

    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color { r, g, b, a }
    }

    pub fn with_alpha(self, a: u8) -> Color {
        Color { a, ..self }
    }

    pub fn to_u32(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    pub fn to_u32_with_alpha(self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    pub fn blend(&self, other: &Color) -> Color {
        let alpha = other.a as f32 / 255.0;
        let inv_alpha = 1.0 - alpha;
        Color {
            r: (self.r as f32 * inv_alpha + other.r as f32 * alpha) as u8,
            g: (self.g as f32 * inv_alpha + other.g as f32 * alpha) as u8,
            b: (self.b as f32 * inv_alpha + other.b as f32 * alpha) as u8,
            a: 255,
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Color::BLACK
    }
}

// ============================================================================
// EXPRESSIONS
// ============================================================================

/// Expressions for dynamic values - significantly expanded
#[derive(Debug, Clone)]
pub enum Expression {
    // Literals
    Literal(Value),
    
    // Variable access
    Variable(String),
    
    // Property access: obj.prop or obj["prop"]
    PropertyAccess {
        object: Box<Expression>,
        property: Box<Expression>,
    },
    
    // Index access: arr[0] or str[1]
    IndexAccess {
        object: Box<Expression>,
        index: Box<Expression>,
    },
    
    // Binary operations
    Binary {
        left: Box<Expression>,
        op: BinaryOp,
        right: Box<Expression>,
    },
    
    // Unary operations
    Unary {
        op: UnaryOp,
        operand: Box<Expression>,
    },
    
    // Conditional (ternary): condition ? then : else
    Conditional {
        condition: Box<Expression>,
        then_expr: Box<Expression>,
        else_expr: Box<Expression>,
    },
    
    // Function/method calls
    Call {
        function: String,
        args: Vec<Expression>,
    },
    
    // Method calls on values: list.map(fn), str.upper()
    MethodCall {
        object: Box<Expression>,
        method: String,
        args: Vec<Expression>,
    },
    
    // List literal: [1, 2, 3]
    ListLiteral(Vec<Expression>),
    
    // Object literal: { name: "John", age: 30 }
    ObjectLiteral(Vec<(String, Expression)>),
    
    // String interpolation: "Hello, {name}!"
    Interpolation(Vec<InterpolationPart>),
    
    // Lambda/arrow function: |x| x * 2
    Lambda {
        params: Vec<String>,
        body: Box<Expression>,
    },
    
    // Range: 1..10 or 1..=10
    Range {
        start: Box<Expression>,
        end: Box<Expression>,
        inclusive: bool,
    },
    
    // Spread operator: ...list
    Spread(Box<Expression>),
    
    // Pipe operator: value |> transform
    Pipe {
        value: Box<Expression>,
        transform: Box<Expression>,
    },
    
    // Null coalescing: value ?? default
    NullCoalesce {
        value: Box<Expression>,
        default: Box<Expression>,
    },
}

#[derive(Debug, Clone)]
pub enum InterpolationPart {
    Literal(String),
    Expression(Box<Expression>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    
    // Comparison
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    
    // Logical
    And,
    Or,
    
    // String
    Concat,
    
    // Collection
    In,
    NotIn,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Not,
    Neg,
    Typeof,
    Len,
}

// ============================================================================
// ACTIONS
// ============================================================================

/// Action block - state mutations with control flow
#[derive(Debug, Clone)]
pub struct ActionBlock {
    pub params: Vec<String>,
    pub statements: Vec<Statement>,
}

/// Statements within actions
#[derive(Debug, Clone)]
pub enum Statement {
    // Variable assignment
    Assign {
        target: AssignTarget,
        value: Expression,
    },
    
    // Conditional execution
    If {
        condition: Expression,
        then_block: Vec<Statement>,
        else_block: Vec<Statement>,
    },
    
    // Loop over collection
    ForEach {
        item: String,
        index: Option<String>,
        collection: Expression,
        body: Vec<Statement>,
    },
    
    // Conditional loop
    While {
        condition: Expression,
        body: Vec<Statement>,
    },
    
    // Early return
    Return(Option<Expression>),
    
    // Break out of loop
    Break,
    
    // Continue to next iteration
    Continue,
    
    // Call another action
    Call {
        action: String,
        args: Vec<Expression>,
    },
    
    // Log to console (development)
    Log(Expression),
    
    // Trigger event (for parent components)
    Emit {
        event: String,
        data: Option<Expression>,
    },
    
    // Navigate to route
    Navigate(Expression),
    
    // HTTP fetch (sandboxed)
    Fetch {
        url: Expression,
        method: HttpMethod,
        body: Option<Expression>,
        headers: Vec<(String, Expression)>,
        on_success: String,
        on_error: String,
    },
    
    // Delay execution
    Delay {
        ms: Expression,
        then: Vec<Statement>,
    },
    
    // List operations as statements
    ListPush {
        target: String,
        value: Expression,
    },
    ListPop {
        target: String,
    },
    ListInsert {
        target: String,
        index: Expression,
        value: Expression,
    },
    ListRemove {
        target: String,
        index: Expression,
    },
    ListClear {
        target: String,
    },
}

/// Assignment target (can be nested)
#[derive(Debug, Clone)]
pub enum AssignTarget {
    Variable(String),
    Index {
        object: String,
        index: Expression,
    },
    Property {
        object: String,
        property: String,
    },
}

/// HTTP methods for fetch
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

// ============================================================================
// LEGACY SUPPORT
// ============================================================================

/// Legacy mutation format (for backwards compatibility)
#[derive(Debug, Clone)]
pub struct Mutation {
    pub target: String,
    pub value: Expression,
}

impl From<Mutation> for Statement {
    fn from(m: Mutation) -> Self {
        Statement::Assign {
            target: AssignTarget::Variable(m.target),
            value: m.value,
        }
    }
}
