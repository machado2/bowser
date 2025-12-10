# Prism Language Specification v1.0

## 1. Overview

Prism is a declarative language for building distributed applications. It unifies layout, styling, state management, and interactivity into a single, human-readable format.

### 1.1 Design Goals

1. **Human-First** - Readable and writable in any text editor
2. **Zero Build** - No compilation, transpilation, or bundling
3. **Reactive** - State changes automatically propagate to the UI
4. **Secure** - Sandboxed execution with no implicit capabilities

## 2. File Format

### 2.1 Extension

Prism files use the `.prism` extension.

### 2.2 Encoding

Files MUST be encoded in UTF-8.

### 2.3 Structure

A Prism file consists of four sections:

```
┌─────────────────────────┐
│  Directives (@app, etc) │
├─────────────────────────┤
│  state { ... }          │
├─────────────────────────┤
│  view { ... }           │
├─────────────────────────┤
│  actions { ... }        │
└─────────────────────────┘
```

## 3. Lexical Structure

### 3.1 Comments

Line comments begin with `--` and extend to end of line:

```prism
-- This is a comment
state {
  count: 0  -- inline comment
}
```

### 3.2 Whitespace

Whitespace (spaces, tabs, newlines) is used to separate tokens. Indentation is stylistic, not semantic (unlike Python/YAML).

### 3.3 Identifiers

Identifiers start with a letter or underscore, followed by letters, digits, or underscores:

```
identifier := [a-zA-Z_][a-zA-Z0-9_]*
```

### 3.4 Literals

#### String Literals
```prism
"Hello, World!"
"Line 1\nLine 2"  -- escape sequences: \n, \t, \\, \"
```

#### Numeric Literals
```prism
42        -- integer
3.14      -- float
-17       -- negative integer
```

#### Boolean Literals
```prism
true
false
```

#### Color Literals
```prism
#FF0000   -- 6-digit hex (RGB)
#F00      -- 3-digit hex shorthand
```

## 4. Directives

Directives begin with `@` and appear at the top of the file.

### 4.1 @app

Declares the application name (required):

```prism
@app "My Application"
```

### 4.2 @version

Declares the Prism specification version (required):

```prism
@version 1
```

## 5. State Block

The `state` block declares reactive state variables:

```prism
state {
  count: 0
  name: ""
  is_active: false
  price: 19.99
}
```

### 5.1 Supported Types

| Type | Example | Description |
|------|---------|-------------|
| Integer | `0`, `-5`, `100` | 64-bit signed integer |
| Float | `3.14`, `-0.5` | 64-bit floating point |
| String | `"hello"` | UTF-8 string |
| Boolean | `true`, `false` | Boolean value |
| Null | `null` | Absence of value |

## 6. View Block

The `view` block declares the UI tree:

```prism
view {
  column {
    text "Hello" {
      size: 24
    }
  }
}
```

### 6.1 Node Types

#### column
Vertical stack layout.
```prism
column {
  padding: 20
  gap: 10
  background: #F0F0F0
  
  -- children --
}
```

#### row
Horizontal stack layout.
```prism
row {
  padding: 10
  gap: 8
  
  -- children --
}
```

#### text
Displays text content.
```prism
text "Static text" {
  size: 16
  color: #333333
}

text "{variable}" {
  size: 16
}
```

#### button
Interactive button element.
```prism
button "Click Me" {
  on_click: action_name
  background: #4285F4
  color: #FFFFFF
}
```

#### input
Text input field.
```prism
input {
  placeholder: "Enter text..."
  bind: state_variable
}
```

#### box
Generic container.
```prism
box {
  padding: 16
  background: #EEEEEE
  
  -- children --
}
```

#### spacer
Flexible space element.
```prism
spacer {}
```

### 6.2 Common Properties

| Property | Type | Description |
|----------|------|-------------|
| `padding` | Integer | Inner padding in pixels |
| `gap` | Integer | Space between children |
| `background` | Color | Background color |
| `color` | Color | Text/foreground color |
| `size` | Integer | Font size for text |
| `visible` | Expression | Conditional visibility |

### 6.3 Text Interpolation

Dynamic values can be embedded in text using `{variable}`:

```prism
text "Count: {count}" {}
text "Hello, {name}!" {}
```

## 7. Actions Block

Actions define state mutations triggered by user interaction:

```prism
actions {
  increment {
    count: count + 1
  }
  
  reset {
    count: 0
    name: ""
  }
}
```

### 7.1 Action Syntax

```
action_name {
  target: expression
  target2: expression2
}
```

Each line in an action sets a state variable to the result of an expression.

## 8. Expressions

Expressions compute values from state and literals.

### 8.1 Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Addition/Concatenation | `count + 1` |
| `-` | Subtraction | `count - 1` |
| `*` | Multiplication | `price * quantity` |
| `/` | Division | `total / count` |
| `==` | Equality | `status == "active"` |
| `!=` | Inequality | `name != ""` |
| `<` | Less than | `count < 10` |
| `>` | Greater than | `count > 0` |
| `<=` | Less or equal | `age <= 18` |
| `>=` | Greater or equal | `score >= 100` |
| `and` | Logical AND | `a and b` |
| `or` | Logical OR | `a or b` |

### 8.2 Operator Precedence (highest to lowest)

1. `*`, `/`
2. `+`, `-`
3. `<`, `>`, `<=`, `>=`
4. `==`, `!=`
5. `and`
6. `or`

### 8.3 Parentheses

Use parentheses to override precedence:

```prism
(count + 1) * 2
```

## 9. Runtime Behavior

### 9.1 Initialization

1. Parse the `.prism` file into an AST
2. Initialize state with declared values
3. Build initial view tree
4. Render to screen

### 9.2 Event Loop

1. Wait for user input (click, keyboard)
2. Determine target element via hit testing
3. Execute associated action (if any)
4. Update state
5. Re-render affected portions of view

### 9.3 Reactivity Model

State changes trigger automatic re-evaluation of:
- Text interpolations containing changed variables
- `visible` expressions depending on changed variables
- Any expression referencing changed variables

## 10. Security Model

### 10.1 Sandbox Constraints

Prism applications run in a strict sandbox:

| Capability | Status | Notes |
|------------|--------|-------|
| File System Access | ❌ DENIED | No read/write to local files |
| Network Requests | ❌ DENIED | No external HTTP requests |
| Persistent Storage | ❌ DENIED | No cookies, localStorage equivalent |
| Device Fingerprinting | ❌ DENIED | No access to hardware identifiers |
| Clipboard | ❌ DENIED | No read/write to clipboard |
| Memory | ⚠️ LIMITED | 16MB per application |

### 10.2 Path Validation

- Only `.prism` files can be loaded
- Path traversal (`..`) is blocked
- Absolute paths outside app directory are blocked

### 10.3 Memory Limits

- Maximum file size: 1MB
- Maximum runtime memory: 16MB
- Exceeding limits terminates the application

## 11. Conformance

A conforming Prism viewer MUST:

1. Parse valid `.prism` files according to this specification
2. Reject files exceeding security limits
3. Implement the reactive update model
4. Provide visual rendering of all node types
5. Handle user input for buttons and inputs

## 12. Future Extensions (Reserved)

The following are reserved for future specification versions:

- `@import` - Module imports
- `@capability` - Explicit capability requests
- `list` - Repeating elements
- `if`/`else` - Conditional nodes
- `fetch` - Sandboxed network requests
- `animation` - Declarative animations

---

## Appendix A: Grammar (EBNF)

```ebnf
program       = { directive } [ state_block ] [ view_block ] [ actions_block ] ;

directive     = "@" identifier ( string_lit | number ) ;

state_block   = "state" "{" { field_decl } "}" ;
field_decl    = identifier ":" value ;

view_block    = "view" "{" view_node "}" ;
view_node     = node_kind [ string_lit ] [ "{" { property | view_node } "}" ] ;
node_kind     = "column" | "row" | "text" | "button" | "input" | "box" | "spacer" ;
property      = identifier ":" prop_value ;
prop_value    = value | color | expression | identifier ;

actions_block = "actions" "{" { action_def } "}" ;
action_def    = identifier "{" { mutation } "}" ;
mutation      = identifier ":" expression ;

expression    = or_expr ;
or_expr       = and_expr { "or" and_expr } ;
and_expr      = compare { "and" compare } ;
compare       = additive { ( "==" | "!=" | "<" | ">" | "<=" | ">=" ) additive } ;
additive      = multiplicative { ( "+" | "-" ) multiplicative } ;
multiplicative = primary { ( "*" | "/" ) primary } ;
primary       = literal | identifier | "(" expression ")" ;

value         = string_lit | number | "true" | "false" | "null" ;
string_lit    = '"' { character } '"' ;
number        = [ "-" ] digit { digit } [ "." digit { digit } ] ;
color         = "#" hex_digit hex_digit hex_digit [ hex_digit hex_digit hex_digit ] ;
identifier    = ( letter | "_" ) { letter | digit | "_" } ;
```

## Appendix B: Complete Example

```prism
@app "Todo List"
@version 1

-- A simple todo application --

state {
  task: ""
  count: 0
  show_completed: true
}

view {
  column {
    padding: 24
    gap: 16
    
    -- Header --
    text "My Todo List" {
      size: 28
      color: #333333
    }
    
    -- Input Section --
    row {
      gap: 12
      
      input {
        placeholder: "What needs to be done?"
        bind: task
      }
      
      button "Add Task" {
        on_click: add_task
        background: #4CAF50
        color: #FFFFFF
      }
    }
    
    -- Stats --
    text "Total tasks: {count}" {
      size: 14
      color: #666666
    }
    
    -- Toggle --
    button "Toggle Completed" {
      on_click: toggle_completed
      background: #9E9E9E
      color: #FFFFFF
    }
  }
}

actions {
  add_task {
    count: count + 1
    task: ""
  }
  
  toggle_completed {
    show_completed: show_completed == false
  }
}
```

---

*Prism Specification v1.0 - Public Domain*
