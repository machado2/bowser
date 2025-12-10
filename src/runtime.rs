//! Runtime orchestration for Prism applications
//!
//! The runtime manages the event loop, state updates, and re-rendering.
//! Extended with full statement execution and control flow.

use crate::ast::{PrismApp, ActionBlock, Statement, AssignTarget, Value};
use crate::state::StateStore;
use crate::renderer::{Renderer, FrameBuffer};
use crate::sandbox::Sandbox;
use std::collections::HashMap;

/// The Prism runtime
pub struct Runtime {
    pub app: PrismApp,
    pub state: StateStore,
    pub renderer: Renderer,
    pub sandbox: Sandbox,
    pub focused_input: Option<String>,
    pub current_route: String,
}

/// Control flow signals for statement execution
enum ControlFlow {
    Continue,
    Break,
    Return(Option<Value>),
}

impl Runtime {
    pub fn new(app: PrismApp) -> Self {
        let mut state = StateStore::new();
        state.init(&app.state);
        state.set_computed(app.computed.clone());

        Self {
            app,
            state,
            renderer: Renderer::new(),
            sandbox: Sandbox::new(),
            focused_input: None,
            current_route: "/".to_string(),
        }
    }

    /// Render the current state to a frame buffer
    pub fn render(&mut self, fb: &mut FrameBuffer) {
        self.renderer.render(fb, &self.app.view, &self.state);
        self.state.mark_clean();
    }

    /// Force a re-render
    pub fn invalidate(&mut self) {
        self.state.invalidate();
    }

    /// Handle a click event at the given coordinates
    pub fn handle_click(&mut self, x: i32, y: i32) -> bool {
        if let Some(layout_box) = self.renderer.hit_test(x, y) {
            // Handle button click
            if let Some(action_name) = &layout_box.action {
                if let Some(action) = self.app.actions.get(action_name).cloned() {
                    self.execute_action(&action, &[]);
                    return true;
                }
            }
            
            // Handle input focus
            if let Some(binding) = &layout_box.input_binding {
                self.focused_input = Some(binding.clone());
                self.state.invalidate();
                return true;
            }
        } else {
            // Clicked outside any interactive element
            self.focused_input = None;
        }
        false
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, key: char) -> bool {
        if let Some(binding) = &self.focused_input {
            let current = self.state.get(binding)
                .map(|v| v.as_string())
                .unwrap_or_default();
            
            let new_value = format!("{}{}", current, key);
            self.state.set(binding, Value::String(new_value));
            return true;
        }
        false
    }

    /// Handle backspace
    pub fn handle_backspace(&mut self) -> bool {
        if let Some(binding) = &self.focused_input {
            let current = self.state.get(binding)
                .map(|v| v.as_string())
                .unwrap_or_default();
            
            if !current.is_empty() {
                let new_value: String = current.chars().take(current.len() - 1).collect();
                self.state.set(binding, Value::String(new_value));
                return true;
            }
        }
        false
    }

    /// Execute an action with arguments
    pub fn execute_action(&mut self, action: &ActionBlock, args: &[Value]) {
        // Bind parameters to arguments
        for (i, param) in action.params.iter().enumerate() {
            let value = args.get(i).cloned().unwrap_or(Value::Null);
            self.state.set_local(param, value);
        }

        // Execute statements
        self.execute_statements(&action.statements);

        // Clear locals after action completes
        self.state.clear_locals();
    }

    /// Execute a list of statements
    fn execute_statements(&mut self, statements: &[Statement]) -> ControlFlow {
        for stmt in statements {
            match self.execute_statement(stmt) {
                ControlFlow::Continue => {}
                flow => return flow,
            }
        }
        ControlFlow::Continue
    }

    /// Execute a single statement
    fn execute_statement(&mut self, stmt: &Statement) -> ControlFlow {
        match stmt {
            Statement::Assign { target, value } => {
                let evaluated = self.state.evaluate(value);
                match target {
                    AssignTarget::Variable(name) => {
                        self.state.set(name, evaluated);
                    }
                    AssignTarget::Index { object, index } => {
                        let idx = self.state.evaluate(index);
                        if let Some(list) = self.state.get_list_mut(object) {
                            let idx = idx.as_int() as usize;
                            if idx < list.len() {
                                list[idx] = evaluated;
                            }
                        }
                    }
                    AssignTarget::Property { object, property } => {
                        if let Some(obj) = self.state.get_object_mut(object) {
                            obj.insert(property.clone(), evaluated);
                        }
                    }
                }
                ControlFlow::Continue
            }

            Statement::If { condition, then_block, else_block } => {
                let cond = self.state.evaluate(condition);
                if cond.as_bool() {
                    self.execute_statements(then_block)
                } else {
                    self.execute_statements(else_block)
                }
            }

            Statement::ForEach { item, index, collection, body } => {
                let list = self.state.evaluate(collection).as_list();
                for (i, val) in list.into_iter().enumerate() {
                    self.state.set_local(item, val);
                    if let Some(idx_name) = index {
                        self.state.set_local(idx_name, Value::Int(i as i64));
                    }
                    match self.execute_statements(body) {
                        ControlFlow::Break => break,
                        ControlFlow::Return(v) => return ControlFlow::Return(v),
                        ControlFlow::Continue => {}
                    }
                }
                ControlFlow::Continue
            }

            Statement::While { condition, body } => {
                loop {
                    let cond = self.state.evaluate(condition);
                    if !cond.as_bool() {
                        break;
                    }
                    match self.execute_statements(body) {
                        ControlFlow::Break => break,
                        ControlFlow::Return(v) => return ControlFlow::Return(v),
                        ControlFlow::Continue => {}
                    }
                }
                ControlFlow::Continue
            }

            Statement::Return(expr) => {
                let value = expr.as_ref().map(|e| self.state.evaluate(e));
                ControlFlow::Return(value)
            }

            Statement::Break => ControlFlow::Break,
            Statement::Continue => ControlFlow::Continue,

            Statement::Call { action, args } => {
                let evaluated_args: Vec<Value> = args.iter()
                    .map(|a| self.state.evaluate(a))
                    .collect();
                if let Some(action_block) = self.app.actions.get(action).cloned() {
                    self.execute_action(&action_block, &evaluated_args);
                }
                ControlFlow::Continue
            }

            Statement::Log(expr) => {
                let value = self.state.evaluate(expr);
                println!("[PRISM LOG] {}", value.as_string());
                ControlFlow::Continue
            }

            Statement::Emit { event, data } => {
                let data_val = data.as_ref().map(|e| self.state.evaluate(e));
                println!("[PRISM EVENT] {}: {:?}", event, data_val);
                ControlFlow::Continue
            }

            Statement::Navigate(expr) => {
                let route = self.state.evaluate(expr).as_string();
                self.current_route = route;
                self.state.invalidate();
                ControlFlow::Continue
            }

            Statement::Fetch { url, method, body, headers: _, on_success, on_error } => {
                // Sandboxed - log but don't actually fetch for now
                let url_val = self.state.evaluate(url).as_string();
                let body_val = body.as_ref().map(|b| self.state.evaluate(b));
                println!("[PRISM FETCH] {:?} {} body={:?}", method, url_val, body_val);
                println!("  on_success: {}, on_error: {}", on_success, on_error);
                // In a real implementation, this would be async
                ControlFlow::Continue
            }

            Statement::Delay { ms, then } => {
                let ms_val = self.state.evaluate(ms).as_int();
                println!("[PRISM DELAY] {}ms (simulated)", ms_val);
                // Execute 'then' immediately in this simple implementation
                self.execute_statements(then)
            }

            Statement::ListPush { target, value } => {
                let val = self.state.evaluate(value);
                if let Some(list) = self.state.get_list_mut(target) {
                    list.push(val);
                }
                ControlFlow::Continue
            }

            Statement::ListPop { target } => {
                if let Some(list) = self.state.get_list_mut(target) {
                    list.pop();
                }
                ControlFlow::Continue
            }

            Statement::ListInsert { target, index, value } => {
                let idx = self.state.evaluate(index).as_int() as usize;
                let val = self.state.evaluate(value);
                if let Some(list) = self.state.get_list_mut(target) {
                    if idx <= list.len() {
                        list.insert(idx, val);
                    }
                }
                ControlFlow::Continue
            }

            Statement::ListRemove { target, index } => {
                let idx = self.state.evaluate(index).as_int() as usize;
                if let Some(list) = self.state.get_list_mut(target) {
                    if idx < list.len() {
                        list.remove(idx);
                    }
                }
                ControlFlow::Continue
            }

            Statement::ListClear { target } => {
                if let Some(list) = self.state.get_list_mut(target) {
                    list.clear();
                }
                ControlFlow::Continue
            }
        }
    }

    /// Get the app title
    pub fn title(&self) -> &str {
        &self.app.name
    }

    /// Get actions (for debugging)
    #[allow(dead_code)]
    pub fn actions(&self) -> &HashMap<String, ActionBlock> {
        &self.app.actions
    }

    /// Get current route
    pub fn route(&self) -> &str {
        &self.current_route
    }
}
