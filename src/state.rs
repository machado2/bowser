#![allow(dead_code)]
//! Reactive state management for Prism
//!
//! The state store holds all application state and tracks changes
//! for efficient re-rendering. Extended with list operations, computed values,
//! and full expression evaluation.

use crate::ast::{Value, StateBlock, Expression, BinaryOp, UnaryOp, InterpolationPart};
use std::collections::HashMap;

/// The reactive state store
pub struct StateStore {
    values: HashMap<String, Value>,
    computed: HashMap<String, Expression>,
    locals: HashMap<String, Value>,  // For loop variables, etc.
    dirty: bool,
}

impl StateStore {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
            computed: HashMap::new(),
            locals: HashMap::new(),
            dirty: true,
        }
    }

    /// Initialize state from a StateBlock
    pub fn init(&mut self, block: &StateBlock) {
        for (key, value) in &block.fields {
            self.values.insert(key.clone(), value.clone());
        }
        self.dirty = true;
    }

    /// Set computed values
    pub fn set_computed(&mut self, computed: HashMap<String, Expression>) {
        self.computed = computed;
    }

    /// Get a value from state (checks locals first, then state, then computed)
    pub fn get(&self, key: &str) -> Option<Value> {
        if let Some(v) = self.locals.get(key) {
            return Some(v.clone());
        }
        if let Some(v) = self.values.get(key) {
            return Some(v.clone());
        }
        if let Some(expr) = self.computed.get(key) {
            return Some(self.evaluate(expr));
        }
        None
    }

    /// Get mutable reference to list
    pub fn get_list_mut(&mut self, key: &str) -> Option<&mut Vec<Value>> {
        if let Some(Value::List(list)) = self.values.get_mut(key) {
            self.dirty = true;
            return Some(list);
        }
        None
    }

    /// Get mutable reference to object
    pub fn get_object_mut(&mut self, key: &str) -> Option<&mut HashMap<String, Value>> {
        if let Some(Value::Object(obj)) = self.values.get_mut(key) {
            self.dirty = true;
            return Some(obj);
        }
        None
    }

    /// Set a value in state
    pub fn set(&mut self, key: &str, value: Value) {
        let changed = self.values.get(key) != Some(&value);
        self.values.insert(key.to_string(), value);
        if changed {
            self.dirty = true;
        }
    }

    /// Set a local variable (for loops, etc.)
    pub fn set_local(&mut self, key: &str, value: Value) {
        self.locals.insert(key.to_string(), value);
    }

    /// Clear local variables
    pub fn clear_locals(&mut self) {
        self.locals.clear();
    }

    /// Set a nested value (object property or list index)
    pub fn set_nested(&mut self, path: &[&str], value: Value) {
        if path.is_empty() {
            return;
        }
        
        let key = path[0];
        if path.len() == 1 {
            self.set(key, value);
            return;
        }

        // Deep set - simplified for now
        self.dirty = true;
    }

    /// Check if state has changed since last render
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark state as clean (after render)
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Force a re-render
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// Evaluate an expression against current state
    pub fn evaluate(&self, expr: &Expression) -> Value {
        match expr {
            Expression::Literal(v) => v.clone(),
            
            Expression::Variable(name) => {
                self.get(name).unwrap_or(Value::Null)
            }
            
            Expression::Binary { left, op, right } => {
                let left_val = self.evaluate(left);
                let right_val = self.evaluate(right);
                self.apply_binary_op(&left_val, op, &right_val)
            }
            
            Expression::Unary { op, operand } => {
                let val = self.evaluate(operand);
                self.apply_unary_op(op, &val)
            }
            
            Expression::Interpolation(parts) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        InterpolationPart::Literal(s) => result.push_str(s),
                        InterpolationPart::Expression(expr) => {
                            let val = self.evaluate(expr);
                            result.push_str(&val.as_string());
                        }
                    }
                }
                Value::String(result)
            }
            
            Expression::PropertyAccess { object, property } => {
                let obj = self.evaluate(object);
                let prop = self.evaluate(property);
                obj.get(&prop)
            }
            
            Expression::IndexAccess { object, index } => {
                let obj = self.evaluate(object);
                let idx = self.evaluate(index);
                obj.get(&idx)
            }
            
            Expression::Conditional { condition, then_expr, else_expr } => {
                let cond = self.evaluate(condition);
                if cond.as_bool() {
                    self.evaluate(then_expr)
                } else {
                    self.evaluate(else_expr)
                }
            }
            
            Expression::Call { function, args } => {
                let evaluated_args: Vec<Value> = args.iter().map(|a| self.evaluate(a)).collect();
                self.call_builtin(function, &evaluated_args)
            }
            
            Expression::MethodCall { object, method, args } => {
                let obj = self.evaluate(object);
                let evaluated_args: Vec<Value> = args.iter().map(|a| self.evaluate(a)).collect();
                self.call_method(&obj, method, &evaluated_args)
            }
            
            Expression::ListLiteral(items) => {
                let values: Vec<Value> = items.iter().map(|e| self.evaluate(e)).collect();
                Value::List(values)
            }
            
            Expression::ObjectLiteral(pairs) => {
                let mut map = HashMap::new();
                for (key, expr) in pairs {
                    map.insert(key.clone(), self.evaluate(expr));
                }
                Value::Object(map)
            }
            
            Expression::Lambda { .. } => {
                // Lambdas are evaluated when called, return as-is for now
                Value::Null
            }
            
            Expression::Range { start, end, inclusive } => {
                let start_val = self.evaluate(start).as_int();
                let end_val = self.evaluate(end).as_int();
                let range: Vec<Value> = if *inclusive {
                    (start_val..=end_val).map(Value::Int).collect()
                } else {
                    (start_val..end_val).map(Value::Int).collect()
                };
                Value::List(range)
            }
            
            Expression::NullCoalesce { value, default } => {
                let val = self.evaluate(value);
                if matches!(val, Value::Null) {
                    self.evaluate(default)
                } else {
                    val
                }
            }
            
            Expression::Spread(expr) => {
                // Spread returns the inner list as-is
                self.evaluate(expr)
            }
            
            Expression::Pipe { value, transform: _ } => {
                // Pipe passes value to transform
                let val = self.evaluate(value);
                // For now, treat as identity - full implementation would substitute
                val
            }
        }
    }

    fn apply_unary_op(&self, op: &UnaryOp, val: &Value) -> Value {
        match op {
            UnaryOp::Not => Value::Bool(!val.as_bool()),
            UnaryOp::Neg => match val {
                Value::Int(i) => Value::Int(-i),
                Value::Float(f) => Value::Float(-f),
                _ => Value::Int(-val.as_int()),
            },
            UnaryOp::Typeof => Value::String(val.type_name().to_string()),
            UnaryOp::Len => Value::Int(val.len() as i64),
        }
    }

    fn apply_binary_op(&self, left: &Value, op: &BinaryOp, right: &Value) -> Value {
        match op {
            BinaryOp::Add => {
                match (left, right) {
                    (Value::Int(a), Value::Int(b)) => Value::Int(a + b),
                    (Value::Float(a), Value::Float(b)) => Value::Float(a + b),
                    (Value::Int(a), Value::Float(b)) => Value::Float(*a as f64 + b),
                    (Value::Float(a), Value::Int(b)) => Value::Float(a + *b as f64),
                    (Value::String(a), Value::String(b)) => Value::String(format!("{}{}", a, b)),
                    (Value::String(a), b) => Value::String(format!("{}{}", a, b.as_string())),
                    (a, Value::String(b)) => Value::String(format!("{}{}", a.as_string(), b)),
                    (Value::List(a), Value::List(b)) => {
                        let mut result = a.clone();
                        result.extend(b.clone());
                        Value::List(result)
                    }
                    _ => Value::Int(left.as_int() + right.as_int()),
                }
            }
            BinaryOp::Sub => {
                match (left, right) {
                    (Value::Int(a), Value::Int(b)) => Value::Int(a - b),
                    (Value::Float(a), Value::Float(b)) => Value::Float(a - b),
                    (Value::Int(a), Value::Float(b)) => Value::Float(*a as f64 - b),
                    (Value::Float(a), Value::Int(b)) => Value::Float(a - *b as f64),
                    _ => Value::Int(left.as_int() - right.as_int()),
                }
            }
            BinaryOp::Mul => {
                match (left, right) {
                    (Value::Int(a), Value::Int(b)) => Value::Int(a * b),
                    (Value::Float(a), Value::Float(b)) => Value::Float(a * b),
                    (Value::Int(a), Value::Float(b)) => Value::Float(*a as f64 * b),
                    (Value::Float(a), Value::Int(b)) => Value::Float(a * *b as f64),
                    (Value::String(s), Value::Int(n)) | (Value::Int(n), Value::String(s)) => {
                        Value::String(s.repeat(*n as usize))
                    }
                    _ => Value::Int(left.as_int() * right.as_int()),
                }
            }
            BinaryOp::Div => {
                match (left, right) {
                    (Value::Int(a), Value::Int(b)) if *b != 0 => Value::Int(a / b),
                    (Value::Float(a), Value::Float(b)) if *b != 0.0 => Value::Float(a / b),
                    (Value::Int(a), Value::Float(b)) if *b != 0.0 => Value::Float(*a as f64 / b),
                    (Value::Float(a), Value::Int(b)) if *b != 0 => Value::Float(a / *b as f64),
                    _ => Value::Int(0),
                }
            }
            BinaryOp::Mod => {
                match (left, right) {
                    (Value::Int(a), Value::Int(b)) if *b != 0 => Value::Int(a % b),
                    (Value::Float(a), Value::Float(b)) if *b != 0.0 => Value::Float(a % b),
                    _ => Value::Int(0),
                }
            }
            BinaryOp::Pow => {
                match (left, right) {
                    (Value::Int(a), Value::Int(b)) => Value::Int(a.pow(*b as u32)),
                    (Value::Float(a), Value::Float(b)) => Value::Float(a.powf(*b)),
                    (Value::Int(a), Value::Float(b)) => Value::Float((*a as f64).powf(*b)),
                    (Value::Float(a), Value::Int(b)) => Value::Float(a.powi(*b as i32)),
                    _ => Value::Int(0),
                }
            }
            BinaryOp::Eq => Value::Bool(left == right),
            BinaryOp::Ne => Value::Bool(left != right),
            BinaryOp::Lt => Value::Bool(left.as_float() < right.as_float()),
            BinaryOp::Gt => Value::Bool(left.as_float() > right.as_float()),
            BinaryOp::Le => Value::Bool(left.as_float() <= right.as_float()),
            BinaryOp::Ge => Value::Bool(left.as_float() >= right.as_float()),
            BinaryOp::And => Value::Bool(left.as_bool() && right.as_bool()),
            BinaryOp::Or => Value::Bool(left.as_bool() || right.as_bool()),
            BinaryOp::Concat => Value::String(format!("{}{}", left.as_string(), right.as_string())),
            BinaryOp::In => {
                match right {
                    Value::List(list) => Value::Bool(list.contains(left)),
                    Value::String(s) => Value::Bool(s.contains(&left.as_string())),
                    Value::Object(obj) => Value::Bool(obj.contains_key(&left.as_string())),
                    _ => Value::Bool(false),
                }
            }
            BinaryOp::NotIn => {
                match right {
                    Value::List(list) => Value::Bool(!list.contains(left)),
                    Value::String(s) => Value::Bool(!s.contains(&left.as_string())),
                    Value::Object(obj) => Value::Bool(!obj.contains_key(&left.as_string())),
                    _ => Value::Bool(true),
                }
            }
        }
    }

    /// Call a built-in function
    fn call_builtin(&self, name: &str, args: &[Value]) -> Value {
        match name {
            // Math
            "abs" => args.first().map(|v| match v {
                Value::Int(i) => Value::Int(i.abs()),
                Value::Float(f) => Value::Float(f.abs()),
                _ => Value::Int(v.as_int().abs()),
            }).unwrap_or(Value::Null),
            "min" => {
                if args.len() < 2 { return Value::Null; }
                let a = args[0].as_float();
                let b = args[1].as_float();
                Value::Float(a.min(b))
            }
            "max" => {
                if args.len() < 2 { return Value::Null; }
                let a = args[0].as_float();
                let b = args[1].as_float();
                Value::Float(a.max(b))
            }
            "floor" => args.first().map(|v| Value::Int(v.as_float().floor() as i64)).unwrap_or(Value::Null),
            "ceil" => args.first().map(|v| Value::Int(v.as_float().ceil() as i64)).unwrap_or(Value::Null),
            "round" => args.first().map(|v| Value::Int(v.as_float().round() as i64)).unwrap_or(Value::Null),
            "sqrt" => args.first().map(|v| Value::Float(v.as_float().sqrt())).unwrap_or(Value::Null),
            
            // String
            "len" => args.first().map(|v| Value::Int(v.len() as i64)).unwrap_or(Value::Null),
            "str" => args.first().map(|v| Value::String(v.as_string())).unwrap_or(Value::Null),
            "int" => args.first().map(|v| Value::Int(v.as_int())).unwrap_or(Value::Null),
            "float" => args.first().map(|v| Value::Float(v.as_float())).unwrap_or(Value::Null),
            "bool" => args.first().map(|v| Value::Bool(v.as_bool())).unwrap_or(Value::Null),
            
            // Type checking
            "type" => args.first().map(|v| Value::String(v.type_name().to_string())).unwrap_or(Value::Null),
            "is_null" => args.first().map(|v| Value::Bool(matches!(v, Value::Null))).unwrap_or(Value::Bool(true)),
            "is_list" => args.first().map(|v| Value::Bool(matches!(v, Value::List(_)))).unwrap_or(Value::Bool(false)),
            "is_object" => args.first().map(|v| Value::Bool(matches!(v, Value::Object(_)))).unwrap_or(Value::Bool(false)),
            
            // List creation
            "list" => Value::List(args.to_vec()),
            "range" => {
                let start = args.first().map(|v| v.as_int()).unwrap_or(0);
                let end = args.get(1).map(|v| v.as_int()).unwrap_or(start);
                let step = args.get(2).map(|v| v.as_int()).unwrap_or(1);
                let (start, end) = if args.len() == 1 { (0, start) } else { (start, end) };
                if step <= 0 {
                    return Value::List(vec![]);
                }
                let range: Vec<Value> = (start..end).step_by(step as usize).map(Value::Int).collect();
                Value::List(range)
            }
            
            // Object
            "keys" => args.first().map(|v| match v {
                Value::Object(obj) => Value::List(obj.keys().cloned().map(Value::String).collect()),
                _ => Value::List(vec![]),
            }).unwrap_or(Value::List(vec![])),
            "values" => args.first().map(|v| match v {
                Value::Object(obj) => Value::List(obj.values().cloned().collect()),
                _ => Value::List(vec![]),
            }).unwrap_or(Value::List(vec![])),
            
            // JSON
            "json_encode" => args.first().map(|v| Value::String(self.to_json(v))).unwrap_or(Value::Null),
            
            _ => Value::Null,
        }
    }

    /// Call a method on a value
    fn call_method(&self, obj: &Value, method: &str, args: &[Value]) -> Value {
        match (obj, method) {
            // String methods
            (Value::String(s), "upper") => Value::String(s.to_uppercase()),
            (Value::String(s), "lower") => Value::String(s.to_lowercase()),
            (Value::String(s), "trim") => Value::String(s.trim().to_string()),
            (Value::String(s), "trim_start") => Value::String(s.trim_start().to_string()),
            (Value::String(s), "trim_end") => Value::String(s.trim_end().to_string()),
            (Value::String(s), "len") => Value::Int(s.len() as i64),
            (Value::String(s), "chars") => Value::Int(s.chars().count() as i64),
            (Value::String(s), "split") => {
                let sep = args.first().map(|v| v.as_string()).unwrap_or_else(|| " ".to_string());
                let parts: Vec<Value> = s.split(&sep).map(|p| Value::String(p.to_string())).collect();
                Value::List(parts)
            }
            (Value::String(s), "join") => {
                // This is inverted - usually called on separator with list arg
                let list = args.first();
                match list {
                    Some(Value::List(items)) => {
                        let strs: Vec<String> = items.iter().map(|v| v.as_string()).collect();
                        Value::String(strs.join(s))
                    }
                    _ => Value::String(s.clone()),
                }
            }
            (Value::String(s), "replace") => {
                if args.len() < 2 { return Value::String(s.clone()); }
                let from = args[0].as_string();
                let to = args[1].as_string();
                Value::String(s.replace(&from, &to))
            }
            (Value::String(s), "starts_with") => {
                let prefix = args.first().map(|v| v.as_string()).unwrap_or_default();
                Value::Bool(s.starts_with(&prefix))
            }
            (Value::String(s), "ends_with") => {
                let suffix = args.first().map(|v| v.as_string()).unwrap_or_default();
                Value::Bool(s.ends_with(&suffix))
            }
            (Value::String(s), "contains") => {
                let needle = args.first().map(|v| v.as_string()).unwrap_or_default();
                Value::Bool(s.contains(&needle))
            }
            (Value::String(s), "slice") => {
                let start = args.first().map(|v| v.as_int()).unwrap_or(0) as usize;
                let end = args.get(1).map(|v| v.as_int() as usize).unwrap_or(s.len());
                let chars: Vec<char> = s.chars().collect();
                let sliced: String = chars.get(start..end.min(chars.len())).unwrap_or(&[]).iter().collect();
                Value::String(sliced)
            }
            (Value::String(s), "repeat") => {
                let n = args.first().map(|v| v.as_int()).unwrap_or(1) as usize;
                Value::String(s.repeat(n))
            }
            (Value::String(s), "pad_start") => {
                let len = args.first().map(|v| v.as_int()).unwrap_or(0) as usize;
                let pad = args.get(1).map(|v| v.as_string()).unwrap_or_else(|| " ".to_string());
                let pad_char = pad.chars().next().unwrap_or(' ');
                let current_len = s.chars().count();
                if current_len >= len {
                    Value::String(s.clone())
                } else {
                    let padding: String = std::iter::repeat(pad_char).take(len - current_len).collect();
                    Value::String(format!("{}{}", padding, s))
                }
            }
            (Value::String(s), "pad_end") => {
                let len = args.first().map(|v| v.as_int()).unwrap_or(0) as usize;
                let pad = args.get(1).map(|v| v.as_string()).unwrap_or_else(|| " ".to_string());
                let pad_char = pad.chars().next().unwrap_or(' ');
                let current_len = s.chars().count();
                if current_len >= len {
                    Value::String(s.clone())
                } else {
                    let padding: String = std::iter::repeat(pad_char).take(len - current_len).collect();
                    Value::String(format!("{}{}", s, padding))
                }
            }
            
            // List methods
            (Value::List(list), "len") => Value::Int(list.len() as i64),
            (Value::List(list), "first") => list.first().cloned().unwrap_or(Value::Null),
            (Value::List(list), "last") => list.last().cloned().unwrap_or(Value::Null),
            (Value::List(list), "get") => {
                let idx = args.first().map(|v| v.as_int()).unwrap_or(0);
                let idx = if idx < 0 { (list.len() as i64 + idx) as usize } else { idx as usize };
                list.get(idx).cloned().unwrap_or(Value::Null)
            }
            (Value::List(list), "slice") => {
                let start = args.first().map(|v| v.as_int()).unwrap_or(0) as usize;
                let end = args.get(1).map(|v| v.as_int() as usize).unwrap_or(list.len());
                Value::List(list.get(start..end.min(list.len())).unwrap_or(&[]).to_vec())
            }
            (Value::List(list), "contains") => {
                let item = args.first().cloned().unwrap_or(Value::Null);
                Value::Bool(list.contains(&item))
            }
            (Value::List(list), "index_of") => {
                let item = args.first().cloned().unwrap_or(Value::Null);
                Value::Int(list.iter().position(|v| v == &item).map(|i| i as i64).unwrap_or(-1))
            }
            (Value::List(list), "join") => {
                let sep = args.first().map(|v| v.as_string()).unwrap_or_else(|| ",".to_string());
                let strs: Vec<String> = list.iter().map(|v| v.as_string()).collect();
                Value::String(strs.join(&sep))
            }
            (Value::List(list), "reverse") => {
                let mut reversed = list.clone();
                reversed.reverse();
                Value::List(reversed)
            }
            (Value::List(list), "sort") => {
                let mut sorted = list.clone();
                sorted.sort_by(|a, b| a.as_float().partial_cmp(&b.as_float()).unwrap_or(std::cmp::Ordering::Equal));
                Value::List(sorted)
            }
            (Value::List(list), "unique") => {
                let mut seen = vec![];
                let mut unique = vec![];
                for item in list {
                    if !seen.contains(item) {
                        seen.push(item.clone());
                        unique.push(item.clone());
                    }
                }
                Value::List(unique)
            }
            (Value::List(list), "flatten") => {
                let mut flat = vec![];
                for item in list {
                    match item {
                        Value::List(inner) => flat.extend(inner.clone()),
                        _ => flat.push(item.clone()),
                    }
                }
                Value::List(flat)
            }
            (Value::List(list), "sum") => {
                let sum: f64 = list.iter().map(|v| v.as_float()).sum();
                if sum.fract() == 0.0 {
                    Value::Int(sum as i64)
                } else {
                    Value::Float(sum)
                }
            }
            (Value::List(list), "min") => {
                list.iter()
                    .min_by(|a, b| a.as_float().partial_cmp(&b.as_float()).unwrap_or(std::cmp::Ordering::Equal))
                    .cloned()
                    .unwrap_or(Value::Null)
            }
            (Value::List(list), "max") => {
                list.iter()
                    .max_by(|a, b| a.as_float().partial_cmp(&b.as_float()).unwrap_or(std::cmp::Ordering::Equal))
                    .cloned()
                    .unwrap_or(Value::Null)
            }
            (Value::List(list), "avg") => {
                if list.is_empty() {
                    Value::Null
                } else {
                    let sum: f64 = list.iter().map(|v| v.as_float()).sum();
                    Value::Float(sum / list.len() as f64)
                }
            }
            
            // Object methods
            (Value::Object(obj), "keys") => {
                Value::List(obj.keys().cloned().map(Value::String).collect())
            }
            (Value::Object(obj), "values") => {
                Value::List(obj.values().cloned().collect())
            }
            (Value::Object(obj), "entries") => {
                Value::List(obj.iter().map(|(k, v)| {
                    Value::List(vec![Value::String(k.clone()), v.clone()])
                }).collect())
            }
            (Value::Object(obj), "has") => {
                let key = args.first().map(|v| v.as_string()).unwrap_or_default();
                Value::Bool(obj.contains_key(&key))
            }
            (Value::Object(obj), "get") => {
                let key = args.first().map(|v| v.as_string()).unwrap_or_default();
                let default = args.get(1).cloned().unwrap_or(Value::Null);
                obj.get(&key).cloned().unwrap_or(default)
            }
            
            // Number methods
            (Value::Int(n), "abs") => Value::Int(n.abs()),
            (Value::Float(n), "abs") => Value::Float(n.abs()),
            (_, "to_string") => Value::String(obj.as_string()),
            
            _ => Value::Null,
        }
    }

    /// Convert value to JSON-like string
    fn to_json(&self, value: &Value) -> String {
        match value {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            Value::List(items) => {
                let strs: Vec<String> = items.iter().map(|v| self.to_json(v)).collect();
                format!("[{}]", strs.join(","))
            }
            Value::Object(obj) => {
                let pairs: Vec<String> = obj.iter()
                    .map(|(k, v)| format!("\"{}\":{}", k, self.to_json(v)))
                    .collect();
                format!("{{{}}}", pairs.join(","))
            }
        }
    }
}
