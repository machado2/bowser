# Prism: A New Web From First Principles

> *"The best interface is no interface. The best framework is no framework."*

## The Manifesto

### The Problem We're Solving

The modern web is a testament to accidental complexity. What began as a document format has been tortured into an application platform through decades of backwards-compatible patches. The result:

- **React**: 42KB minified, requires JSX transpilation, virtual DOM diffing, 1000+ npm dependencies for "hello world"
- **A typical "modern" web page**: 3-5MB of JavaScript, 200+ network requests, 10-second load times
- **The toolchain**: Node.js, npm, webpack/vite/parcel, babel, typescript, postcss, eslint, prettier...

We asked: **What if we threw it all away and started over?**

---

## Prism: The Specification

### Core Philosophy

1. **One Language** - Not HTML + CSS + JavaScript. One unified declarative format.
2. **No Build Step** - Write in Notepad. Open. It works.
3. **Reactive by Default** - State changes automatically propagate. No manual DOM manipulation.
4. **Sandboxed** - No file system access. No persistent identifiers. Privacy is structural.

### The Format: `.prism`

Prism uses a human-readable, indentation-based syntax inspired by YAML but designed for UI:

```prism
@app "My Application"
@version 1

-- State Declaration --
state {
  count: 0
  theme: "light"
  name: ""
}

-- Layout Declaration --
view {
  column {
    padding: 20
    gap: 10
    
    text "Welcome to Prism" {
      size: 24
      weight: bold
      color: #333
    }
    
    row {
      gap: 8
      
      button "−" {
        on_click: decrement
      }
      
      text "{count}" {
        size: 18
      }
      
      button "+" {
        on_click: increment
      }
    }
    
    input {
      placeholder: "Enter your name"
      bind: name
    }
    
    text "Hello, {name}!" {
      visible: name != ""
    }
  }
}

-- Actions --
actions {
  increment {
    count: count + 1
  }
  
  decrement {
    count: count - 1
  }
}
```

### How It Works

```
┌─────────────────────────────────────────────────────────────────┐
│                        PRISM RUNTIME                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
│  │  Parser  │───▶│   AST    │───▶│  State   │───▶│ Renderer │  │
│  └──────────┘    └──────────┘    │  Store   │    └──────────┘  │
│       │                          └──────────┘         │        │
│       │                               │               │        │
│       ▼                               ▼               ▼        │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    VIRTUAL TREE                          │  │
│  │  - Diffing algorithm compares old/new trees              │  │
│  │  - Only changed nodes trigger re-render                  │  │
│  │  - O(n) complexity for tree updates                      │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│                     SECURITY SANDBOX                            │
│  ✗ No file system access                                        │
│  ✗ No persistent cookies/localStorage equivalent                │
│  ✗ No device fingerprinting APIs                                │
│  ✓ Network: fetch() to same origin only (explicit CORS)         │
│  ✓ Memory: Hard cap per application                             │
└─────────────────────────────────────────────────────────────────┘
```

### Why This Kills Frameworks

| React/Vue/Angular | Prism |
|-------------------|-------|
| JSX compilation | Plain text |
| Virtual DOM library | Built into runtime |
| State management (Redux, Vuex, etc.) | Built-in `state {}` |
| CSS-in-JS / preprocessors | Inline styling, unified |
| Event system | `on_click:` declarative |
| Component lifecycle | Automatic |
| npm install | None |
| node_modules (500MB) | 0 bytes |

---

## Building & Running

### Prerequisites

- Rust (stable recommended). Install via `rustup`.

### Build the Prism Viewer

```bash
cargo build
```

### Run a Prism Application

```bash
cargo run -- examples/counter.prism
```

Other examples:

- `cargo run -- examples/home.prism`
- `cargo run -- examples/layout.prism`
- `cargo run -- examples/interactive.prism`
- `cargo run -- examples/todo.prism`

### CLI Options

- `--layout-log <file.prism>`: prints a layout report and exits. Useful for debugging sizing/centering.

Example:

```bash
cargo run -- --layout-log examples/counter.prism
```

---

## Architecture

```
prism/
├── src/
│   ├── main.rs           # Entry point, window + chrome + event loop
│   ├── parser.rs         # .prism format parser
│   ├── ast.rs            # Abstract Syntax Tree definitions
│   ├── state.rs          # Reactive state management
│   ├── renderer.rs       # Layout + rendering to framebuffer
│   ├── sandbox.rs        # Security restrictions
│   └── runtime.rs        # Orchestration (render, invalidate, content height)
├── assets/               # UI font + optional icons
│   ├── Inter-Regular.ttf
│   ├── icon_back.svg
│   └── icon_forward.svg
├── examples/
│   ├── counter.prism     # Simple counter demo
│   ├── todo.prism        # Todo list demo
│   └── layout.prism      # Layout demonstration
├── Cargo.toml
└── README.md
```

---

## UI & Controls

- Toolbar: back (`‹`) and forward (`›`) buttons, an address bar for opening files.
- Hover feedback: cursor changes to a hand when over links or buttons.
- Buttons: rounded, centered glyphs; neutral background by default.
- Links: baseline-aligned underline and accurate hit target.

## The Future

Prism is not just a format—it's a philosophy. We believe:

1. **Complexity is a choice.** The web chose wrong.
2. **Privacy should be structural**, not policy-based.
3. **The best tools are invisible.** No npm. No webpack. Just text.

*The web was supposed to be simple. Prism makes it simple again.*

---

## License

Public Domain. This is how the web should have been.
