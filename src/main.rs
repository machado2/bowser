//! Prism Browser - A minimal browser for the Prism web format
//!
//! Usage: prism [file.prism]
//! If no file is specified, opens the home page.

mod ast;
mod parser;
mod state;
mod sandbox;
mod renderer;
mod runtime;

use renderer::FrameBuffer;
use runtime::Runtime;
use sandbox::Sandbox;
use std::path::PathBuf;
use fontdue::{Font, FontSettings};
use std::sync::OnceLock;
use fontdue::layout::{Layout, LayoutSettings, TextStyle, CoordinateSystem};
use reqwest::blocking;
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, KeyboardInput, ModifiersState, MouseButton, MouseScrollDelta, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit::window::CursorIcon;
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::time::{Duration, Instant};

const DEFAULT_WIDTH: usize = 1024;
const DEFAULT_HEIGHT: usize = 768;
const CHROME_HEIGHT: usize = 50;

static UI_FONT: OnceLock<Font> = OnceLock::new();

fn ui_font() -> &'static Font {
    UI_FONT.get_or_init(|| {
        Font::from_bytes(include_bytes!("../assets/Inter-Regular.ttf") as &[u8], FontSettings::default())
            .expect("Failed to load UI font")
    })
}

/// Browser state
struct Browser {
    runtime: Option<Runtime>,
    current_path: String,
    history: Vec<String>,
    history_index: usize,
    address_focused: bool,
    address_text: String,
    address_cursor: usize,
    cursor_blink_timer: u32,
    cursor_visible: bool,
    last_error: Option<String>,
    scroll_y: i32,
    max_scroll_y: i32,
    base_dir: PathBuf,
}

impl Browser {
    fn new(base_dir: PathBuf) -> Self {
        Self {
            runtime: None,
            current_path: String::new(),
            history: vec![],
            history_index: 0,
            address_focused: false,
            address_text: String::new(),
            address_cursor: 0,
            cursor_blink_timer: 0,
            cursor_visible: true,
            last_error: None,
            scroll_y: 0,
            max_scroll_y: 0,
            base_dir,
        }
    }

    fn tick_cursor(&mut self) {
        if self.address_focused {
            self.cursor_blink_timer += 1;
            if self.cursor_blink_timer >= 30 {
                self.cursor_visible = !self.cursor_visible;
                self.cursor_blink_timer = 0;
            }
        }
    }

    fn reset_cursor_blink(&mut self) {
        self.cursor_visible = true;
        self.cursor_blink_timer = 0;
    }

    fn insert_char(&mut self, ch: char) {
        let byte_pos = self.char_to_byte_pos(self.address_cursor);
        self.address_text.insert(byte_pos, ch);
        self.address_cursor += 1;
        self.reset_cursor_blink();
    }

    fn delete_char_before(&mut self) {
        if self.address_cursor > 0 {
            self.address_cursor -= 1;
            let byte_pos = self.char_to_byte_pos(self.address_cursor);
            let next_byte = self.char_to_byte_pos(self.address_cursor + 1);
            self.address_text.drain(byte_pos..next_byte);
            self.reset_cursor_blink();
        }
    }

    fn delete_char_after(&mut self) {
        let char_count = self.address_text.chars().count();
        if self.address_cursor < char_count {
            let byte_pos = self.char_to_byte_pos(self.address_cursor);
            let next_byte = self.char_to_byte_pos(self.address_cursor + 1);
            self.address_text.drain(byte_pos..next_byte);
            self.reset_cursor_blink();
        }
    }

    fn move_cursor_left(&mut self) {
        if self.address_cursor > 0 {
            self.address_cursor -= 1;
            self.reset_cursor_blink();
        }
    }

    fn move_cursor_right(&mut self) {
        let char_count = self.address_text.chars().count();
        if self.address_cursor < char_count {
            self.address_cursor += 1;
            self.reset_cursor_blink();
        }
    }

    fn move_cursor_home(&mut self) {
        self.address_cursor = 0;
        self.reset_cursor_blink();
    }

    fn move_cursor_end(&mut self) {
        self.address_cursor = self.address_text.chars().count();
        self.reset_cursor_blink();
    }

    fn char_to_byte_pos(&self, char_pos: usize) -> usize {
        self.address_text.chars().take(char_pos).map(|c| c.len_utf8()).sum()
    }

    fn navigate(&mut self, path: &str) {
        self.navigate_internal(path, true);
    }

    fn go_back(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            let path = self.history[self.history_index].clone();
            self.navigate_without_history(&path);
        }
    }

    fn go_forward(&mut self) {
        if self.history_index + 1 < self.history.len() {
            self.history_index += 1;
            let path = self.history[self.history_index].clone();
            self.navigate_without_history(&path);
        }
    }

    fn navigate_without_history(&mut self, path: &str) {
        self.navigate_internal(path, false);
    }

    fn navigate_internal(&mut self, path: &str, update_history: bool) {
        if path.starts_with("http://") || path.starts_with("https://") {
            self.navigate_url(path, update_history);
            return;
        }

        let full_path = if path.starts_with('/') || path.contains(':') {
            PathBuf::from(path)
        } else {
            self.base_dir.join(path)
        };

        let path_str = full_path.to_string_lossy().to_string();

        // Validate path
        let sandbox = Sandbox::new();
        if let Err(e) = sandbox.validate_file_path(&full_path) {
            eprintln!("Security error: {}", e);
            self.current_path = path_str.clone();
            self.address_text = path_str.clone();
            self.address_cursor = path_str.chars().count();
            self.runtime = None;
            self.last_error = Some(format!("Security error: {}", e));
            return;
        }

        // Load file
        let source = match std::fs::read_to_string(&full_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to load {}: {}", full_path.display(), e);
                self.current_path = path_str.clone();
                self.address_text = path_str.clone();
                self.address_cursor = path_str.chars().count();
                self.runtime = None;
                self.last_error = Some(format!("Failed to load {}: {}", full_path.display(), e));
                return;
            }
        };

        // Parse
        let app = match parser::parse(&source) {
            Ok(app) => app,
            Err(e) => {
                eprintln!("Parse error in {}: {}", full_path.display(), e);
                self.current_path = path_str.clone();
                self.address_text = path_str.clone();
                self.address_cursor = path_str.chars().count();
                self.runtime = None;
                self.last_error = Some(format!("Parse error in {}: {}", full_path.display(), e));
                return;
            }
        };

        println!("Loaded: {} (v{})", app.name, app.version);

        // Update history
        if update_history {
            if self.history.is_empty() || self.history[self.history_index] != path_str {
                // Truncate forward history if navigating from middle
                self.history.truncate(self.history_index + 1);
                self.history.push(path_str.clone());
                self.history_index = self.history.len() - 1;
            }
        }

        self.current_path = path_str.clone();
        self.address_text = path_str.clone();
        self.address_cursor = path_str.chars().count();
        self.runtime = Some(Runtime::new(app));
        self.last_error = None;
        self.scroll_y = 0;
        self.max_scroll_y = 0;
    }

    fn navigate_url(&mut self, url: &str, update_history: bool) {
        let url_str = url.to_string();

        // Allow http:// only for localhost during development; require https:// for remote hosts
        let is_local = url_str.starts_with("http://localhost") || url_str.starts_with("http://127.0.0.1");
        if url_str.starts_with("http://") && !is_local {
            let msg = "Only https:// is allowed for remote URLs (http:// is limited to localhost)".to_string();
            eprintln!("Network error: {}", msg);
            self.current_path = url_str.clone();
            self.address_text = url_str.clone();
            self.address_cursor = url_str.chars().count();
            self.runtime = None;
            self.last_error = Some(msg);
            return;
        }

        let response = match blocking::get(url) {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!("Network error while loading {}: {}", url, e);
                self.current_path = url_str.clone();
                self.address_text = url_str.clone();
                self.address_cursor = url_str.chars().count();
                self.runtime = None;
                self.last_error = Some(format!("Network error while loading {}: {}", url, e));
                return;
            }
        };

        let status = response.status();
        if !status.is_success() {
            eprintln!("HTTP error {} while loading {}", status, url);
            self.current_path = url_str.clone();
            self.address_text = url_str.clone();
            self.address_cursor = url_str.chars().count();
            self.runtime = None;
            self.last_error = Some(format!("HTTP error {} while loading {}", status, url));
            return;
        }

        let source = match response.text() {
            Ok(text) => text,
            Err(e) => {
                eprintln!("Failed to read response body from {}: {}", url, e);
                self.current_path = url_str.clone();
                self.address_text = url_str.clone();
                self.address_cursor = url_str.chars().count();
                self.runtime = None;
                self.last_error = Some(format!("Failed to read response body from {}: {}", url, e));
                return;
            }
        };

        let app = match parser::parse(&source) {
            Ok(app) => app,
            Err(e) => {
                eprintln!("Parse error in {}: {}", url, e);
                self.current_path = url_str.clone();
                self.address_text = url_str.clone();
                self.address_cursor = url_str.chars().count();
                self.runtime = None;
                self.last_error = Some(format!("Parse error in {}: {}", url, e));
                return;
            }
        };

        println!("Loaded: {} (v{})", app.name, app.version);

        if update_history {
            if self.history.is_empty() || self.history[self.history_index] != url_str {
                self.history.truncate(self.history_index + 1);
                self.history.push(url_str.clone());
                self.history_index = self.history.len() - 1;
            }
        }

        self.current_path = url_str.clone();
        self.address_text = url_str.clone();
        self.address_cursor = url_str.chars().count();
        self.runtime = Some(Runtime::new(app));
        self.last_error = None;
        self.scroll_y = 0;
        self.max_scroll_y = 0;
    }

    fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    fn can_go_forward(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut layout_log = false;
    let mut file_arg: Option<String> = None;
    for a in args.iter().skip(1) {
        if a == "--layout-log" { layout_log = true; } else if a.ends_with(".prism") { file_arg = Some(a.clone()); }
    }

    // Determine base directory
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    let base_dir = std::env::current_dir().unwrap_or(exe_dir);

    // Create browser
    let mut browser = Browser::new(base_dir.clone());

    if layout_log {
        let target = file_arg.unwrap_or_else(|| {
            base_dir.join("examples").join("counter.prism").to_string_lossy().into()
        });
        let full_path = if target.starts_with('/') || target.contains(':') { std::path::PathBuf::from(&target) } else { base_dir.join(&target) };
        let source = std::fs::read_to_string(&full_path).expect("Failed to read prism file");
        let app = parser::parse(&source).expect("Failed to parse prism file");
        let mut rt = Runtime::new(app);
        rt.renderer.print_layout_report(&rt.app.view, &rt.state, DEFAULT_WIDTH as u32);
        return;
    }

    // Load initial page
    if args.len() >= 2 {
        browser.navigate(&args[1]);
    } else {
        // Try to load home page
        let home_path = base_dir.join("examples").join("home.prism");
        if home_path.exists() {
            browser.navigate(&home_path.to_string_lossy());
        } else {
            eprintln!("Prism Browser v0.1.0");
            eprintln!("Usage: {} [file.prism]", args[0]);
            eprintln!();
            eprintln!("No home page found. Create examples/home.prism or specify a file.");
        }
    }

    // Create window and graphics context
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(DEFAULT_WIDTH as u32, DEFAULT_HEIGHT as u32))
        .with_title("Prism Browser")
        .build(&event_loop)
        .expect("Failed to create window");

    let context = unsafe { Context::new(&window) }.expect("Failed to create softbuffer context");
    let mut surface = unsafe { Surface::new(&context, &window) }.expect("Failed to create surface");

    let size = window.inner_size();
    let mut fb = FrameBuffer::new(size.width as usize, size.height as usize);

    let mut needs_redraw = true;
    let mut last_mouse_pos: Option<(i32, i32)> = None;
    let mut modifiers = ModifiersState::empty();
    let mut last_tick = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::Resized(new_size) => {
                    let width = NonZeroU32::new(new_size.width.max(1)).expect("width nonzero");
                    let height = NonZeroU32::new(new_size.height.max(1)).expect("height nonzero");
                    surface
                        .resize(width, height)
                        .expect("Failed to resize surface");
                    fb = FrameBuffer::new(new_size.width as usize, new_size.height as usize);
                    if let Some(ref mut rt) = browser.runtime {
                        rt.invalidate();
                    }
                    needs_redraw = true;
                }
                WindowEvent::ModifiersChanged(m) => {
                    modifiers = m;
                }
                WindowEvent::CursorMoved { position, .. } => {
                    last_mouse_pos = Some((position.x as i32, position.y as i32));
                    let (mx, my) = (position.x as i32, position.y as i32);
                    let mut hand = false;
                    if my < CHROME_HEIGHT as i32 {
                        if (mx >= 10 && mx <= 38 && my >= 12 && my <= 40 && browser.can_go_back()) ||
                           (mx >= 45 && mx <= 73 && my >= 12 && my <= 40 && browser.can_go_forward()) {
                            hand = true;
                        }
                    } else if let Some(ref mut rt) = browser.runtime {
                        let content_y = my - CHROME_HEIGHT as i32;
                        if let Some(layout_box) = rt.renderer.hit_test(mx, content_y) {
                            if layout_box.action.is_some() || layout_box.link_href.is_some() {
                                hand = true;
                            }
                        }
                    }
                    window.set_cursor_icon(if hand { CursorIcon::Hand } else { CursorIcon::Default });
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if button == MouseButton::Left && state == ElementState::Pressed {
                        if let Some((mx, my)) = last_mouse_pos {
                            if my < CHROME_HEIGHT as i32 {
                                handle_chrome_click(&mut browser, mx, my, fb.width);
                                needs_redraw = true;
                            } else if let Some(ref mut rt) = browser.runtime {
                                let content_y = my - CHROME_HEIGHT as i32;
                                let mut nav_target: Option<String> = None;
                                if let Some(layout_box) = rt.renderer.hit_test(mx, content_y) {
                                    if let Some(ref href) = layout_box.link_href {
                                        nav_target = Some(href.clone());
                                    }
                                }
                                if let Some(href) = nav_target {
                                    browser.navigate(&href);
                                } else {
                                    rt.handle_click(mx, content_y);
                                    rt.renderer.set_focus(rt.focused_input.clone());
                                }
                                needs_redraw = true;
                            }
                        }
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    if browser.runtime.is_some() {
                        let scroll_delta = match delta {
                            MouseScrollDelta::LineDelta(_, y) => (y * 40.0) as i32,
                            MouseScrollDelta::PixelDelta(pos) => pos.y as i32,
                        };
                        let mut new_scroll = browser.scroll_y - scroll_delta;
                        if new_scroll < 0 {
                            new_scroll = 0;
                        }
                        if new_scroll > browser.max_scroll_y {
                            new_scroll = browser.max_scroll_y;
                        }
                        if new_scroll != browser.scroll_y {
                            browser.scroll_y = new_scroll;
                            needs_redraw = true;
                        }
                    }
                }
                WindowEvent::KeyboardInput { input, .. } => {
                    if input.state == ElementState::Pressed {
                        if handle_key_input(&mut browser, &input, modifiers) {
                            needs_redraw = true;
                        }
                    }
                }
                WindowEvent::ReceivedCharacter(ch) => {
                    if handle_received_char(&mut browser, ch) {
                        needs_redraw = true;
                    }
                }
                _ => {}
            },
            Event::MainEventsCleared => {
                // Tick cursor blink at ~60fps
                let now = Instant::now();
                if now.duration_since(last_tick) >= Duration::from_millis(16) {
                    let old_visible = browser.cursor_visible;
                    browser.tick_cursor();
                    if browser.address_focused && browser.cursor_visible != old_visible {
                        needs_redraw = true;
                    }
                    if let Some(ref mut rt) = browser.runtime {
                        rt.renderer.tick();
                    }
                    last_tick = now;
                }
                if needs_redraw || browser.runtime.as_ref().map(|r| r.state.is_dirty()).unwrap_or(false) {
                    window.request_redraw();
                }
            }
            Event::RedrawRequested(_) => {
                render_browser(&mut fb, &mut browser);

                // Present framebuffer
                let mut buffer = surface.buffer_mut().expect("buffer mut");
                debug_assert_eq!(buffer.len(), fb.pixels.len());
                buffer.copy_from_slice(&fb.pixels);
                buffer.present().expect("present");
                needs_redraw = false;
            }
            _ => {}
        }
    });
}

fn render_browser(fb: &mut FrameBuffer, browser: &mut Browser) {
    fb.clear(0xFFFFFF);
    draw_chrome(fb, browser);

    if let Some(ref mut rt) = browser.runtime {
        let viewport_height = fb.height.saturating_sub(CHROME_HEIGHT).max(1);
        let mut content_fb = FrameBuffer::new(fb.width, viewport_height);

        let full_height = rt.content_height(fb.width as u32) as i32;
        browser.max_scroll_y = (full_height - viewport_height as i32).max(0);
        if browser.scroll_y > browser.max_scroll_y {
            browser.scroll_y = browser.max_scroll_y;
        }
        if browser.scroll_y < 0 {
            browser.scroll_y = 0;
        }

        rt.render(&mut content_fb, browser.scroll_y);
        for y in 0..viewport_height {
            let dst_start = (y + CHROME_HEIGHT) * fb.width;
            let src_start = y * fb.width;
            fb.pixels[dst_start..dst_start + fb.width]
                .copy_from_slice(&content_fb.pixels[src_start..src_start + fb.width]);
        }

        let effective_full_height = full_height.max(viewport_height as i32);
        draw_scrollbar(fb, viewport_height, effective_full_height, browser.scroll_y, browser.max_scroll_y);
    } else if let Some(ref err) = browser.last_error {
        draw_error(fb, err);
    } else {
        draw_welcome(fb);
    }
}

fn draw_scrollbar(fb: &mut FrameBuffer, viewport_height: usize, full_height: i32, scroll_y: i32, max_scroll_y: i32) {
    if full_height <= viewport_height as i32 {
        return;
    }

    let track_width = 8u32;
    let track_x = fb.width as i32 - track_width as i32;
    if track_x < 0 {
        return;
    }

    let track_y = CHROME_HEIGHT as i32;
    let track_height = viewport_height as u32;

    fb.fill_rect(track_x, track_y, track_width, track_height, 0xF0F0F0);

    let ratio = viewport_height as f32 / full_height as f32;
    let min_thumb = 20u32;
    let thumb_height = ((track_height as f32 * ratio) as u32).max(min_thumb).min(track_height);

    let scroll_ratio = if max_scroll_y > 0 { scroll_y as f32 / max_scroll_y as f32 } else { 0.0 };
    let movable = track_height.saturating_sub(thumb_height);
    let thumb_offset = (movable as f32 * scroll_ratio) as u32;
    let thumb_y = track_y + thumb_offset as i32;

    fb.fill_rect(track_x, thumb_y, track_width, thumb_height, 0xC0C0C0);
}

fn draw_chrome(fb: &mut FrameBuffer, browser: &Browser) {
    let width = fb.width as u32;
    fb.fill_rounded_rect_vertical_gradient(0, 0, width, CHROME_HEIGHT as u32, 0, 0xFBFCFE, 0xF3F5F8);
    fb.fill_rect(0, CHROME_HEIGHT as i32 - 1, width, 1, 0xDDDDDD);

    let back_color = if browser.can_go_back() { 0x333333 } else { 0x999999 };
    fb.fill_rounded_rect_vertical_gradient(10, 12, 28, 28, 6, 0xEDEFF4, 0xD8DDE6);
    {
        let size = 16.0;
        let base = baseline_for_box(12, 28, size);
        let w = measure_text_width("‹", size);
        let x = 12 + (28 - w) as i32 / 2;
        draw_text_fb(fb, "‹", x, base, size, back_color);
    }

    let fwd_color = if browser.can_go_forward() { 0x333333 } else { 0x999999 };
    fb.fill_rounded_rect_vertical_gradient(45, 12, 28, 28, 6, 0xEDEFF4, 0xD8DDE6);
    {
        let size = 16.0;
        let base = baseline_for_box(12, 28, size);
        let w = measure_text_width("›", size);
        let x = 47 + (28 - w) as i32 / 2;
        draw_text_fb(fb, "›", x, base, size, fwd_color);
    }

    let addr_x = 80 + 12;
    let addr_width = (width as i32 - addr_x - 20).max(200) as u32;
    let border_color = if browser.address_focused { 0x4285F4 } else { 0xCCCCCC };
    fb.fill_rounded_rect_vertical_gradient(addr_x, 10, addr_width, 32, 6, 0xFFFFFF, 0xF4F6F8);
    fb.draw_rect_outline(addr_x, 10, addr_width, 32, border_color, 1);

    let text_size = 14.0;
    let text_y = baseline_for_box(10, 32, text_size);
    let text_x = addr_x + 10;

    if browser.address_text.is_empty() && !browser.address_focused {
        draw_text_fb(fb, "Enter path (examples/home.prism)", text_x, text_y, text_size, 0x999999);
    } else {
        draw_text_fb(fb, &browser.address_text, text_x, text_y, text_size, 0x333333);
    }

    if browser.address_focused && browser.cursor_visible {
        let text_before_cursor: String = browser.address_text.chars().take(browser.address_cursor).collect();
        let cursor_x = text_x + measure_text_width(&text_before_cursor, text_size) as i32;
        let (ascent, descent, _) = line_metrics(text_size);
        let cursor_height = (ascent + descent) as u32;
        let cursor_top = text_y - ascent;
        fb.fill_rect(cursor_x, cursor_top, 2, cursor_height, 0x333333);
    }
}

fn measure_text_width(text: &str, size: f32) -> u32 {
    if text.is_empty() {
        return 0;
    }
    let font = ui_font();
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings::default());
    layout.append(&[font], &TextStyle::new(text, size, 0));
    let glyphs = layout.glyphs();
    if glyphs.is_empty() {
        return 0;
    }
    let last = &glyphs[glyphs.len() - 1];
    (last.x + last.width as f32).ceil() as u32
}

fn draw_welcome(fb: &mut FrameBuffer) {
    let cx = fb.width as i32 / 2;
    let cy = fb.height as i32 / 2;

    let base1 = baseline_for_box(cy - 40, 20, 16.0);
    let base2 = baseline_for_box(cy, 18, 14.0);
    let base3 = baseline_for_box(cy + 30, 18, 14.0);
    draw_text_fb(fb, "Welcome to Prism Browser", cx - 100, base1, 16.0, 0x333333);
    draw_text_fb(fb, "Open a .prism file to get started", cx - 120, base2, 14.0, 0x666666);
    draw_text_fb(fb, "or create examples/home.prism", cx - 110, base3, 14.0, 0x999999);
}

// removed legacy vector chevron helpers (now using font glyphs)

fn draw_error(fb: &mut FrameBuffer, message: &str) {
    let cx = fb.width as i32 / 2;
    let cy = fb.height as i32 / 2;

    let title = "Navigation error";
    let title_size = 18.0;
    let message_size = 14.0;

    let title_width = measure_text_width(title, title_size) as i32;
    let msg = if message.len() > 160 {
        let mut s = message.to_string();
        s.truncate(160);
        s
    } else {
        message.to_string()
    };
    let msg_width = measure_text_width(&msg, message_size) as i32;

    let title_base = baseline_for_box(cy - 30, 24, title_size);
    let msg_base = baseline_for_box(cy + 10, 18, message_size);

    draw_text_fb(fb, title, cx - title_width / 2, title_base, title_size, 0xCC3333);
    draw_text_fb(fb, &msg, cx - msg_width / 2, msg_base, message_size, 0x666666);
}

fn baseline_for_box(top: i32, height: i32, size: f32) -> i32 {
    let (ascent, descent_abs, line_gap) = line_metrics(size);
    let line_h = ascent + descent_abs + line_gap;
    let offset = (height - line_h).max(0) / 2;
    top + offset + ascent
}

fn line_metrics(size: f32) -> (i32, i32, i32) {
    let font = ui_font();
    if let Some(m) = font.horizontal_line_metrics(size) {
        let ascent = m.ascent.ceil() as i32;
        let descent_abs = (-m.descent).ceil() as i32;
        let gap = m.line_gap.ceil() as i32;
        (ascent, descent_abs, gap)
    } else {
        let ascent = size.ceil() as i32;
        let descent_abs = (size * 0.25).ceil() as i32;
        (ascent, descent_abs, 0)
    }
}

fn draw_text_fb(fb: &mut FrameBuffer, text: &str, x: i32, baseline_y: i32, size: f32, color: u32) {
    let font = ui_font();
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings {
        x: x as f32,
        y: 0.0,
        ..LayoutSettings::default()
    });
    layout.append(&[font], &TextStyle::new(text, size, 0));
    let baseline_in_layout = layout
        .lines()
        .and_then(|lines| lines.first().map(|l| l.baseline_y.round() as i32))
        .unwrap_or(0);
    let dy = baseline_y - baseline_in_layout;

    for glyph in layout.glyphs() {
        let (metrics, bitmap) = font.rasterize_config(glyph.key);
        let gx = glyph.x.round() as i32;
        let gy = glyph.y.round() as i32 + dy;
        let gw = metrics.width as i32;
        let gh = metrics.height as i32;

        for py in 0..gh {
            for px in 0..gw {
                let alpha = bitmap[(py as usize) * metrics.width + px as usize];
                if alpha == 0 {
                    continue;
                }
                let dx = gx + px;
                let dy = gy + py;
                if dx < 0 || dy < 0 || (dx as usize) >= fb.width || (dy as usize) >= fb.height {
                    continue;
                }
                let idx = dy as usize * fb.width + dx as usize;
                let dst = fb.pixels[idx];
                fb.pixels[idx] = alpha_blend(dst, color, alpha);
            }
        }
    }
}

fn alpha_blend(dst: u32, src: u32, alpha: u8) -> u32 {
    let a = alpha as f32 / 255.0;
    let dr = ((dst >> 16) & 0xFF) as f32;
    let dg = ((dst >> 8) & 0xFF) as f32;
    let db = (dst & 0xFF) as f32;

    let sr = ((src >> 16) & 0xFF) as f32;
    let sg = ((src >> 8) & 0xFF) as f32;
    let sb = (src & 0xFF) as f32;

    let r = (sr * a + dr * (1.0 - a)) as u32;
    let g = (sg * a + dg * (1.0 - a)) as u32;
    let b = (sb * a + db * (1.0 - a)) as u32;

    (r << 16) | (g << 8) | b
}

fn handle_chrome_click(browser: &mut Browser, x: i32, _y: i32, _width: usize) {
    if x >= 10 && x < 38 {
        browser.go_back();
        return;
    }
    if x >= 45 && x < 73 {
        browser.go_forward();
        return;
    }
    let home_x = 80;
    let home_width = 48i32;
    if x >= home_x && x < home_x + home_width {
        let home = browser.base_dir.join("examples").join("home.prism");
        if home.exists() {
            browser.navigate(&home.to_string_lossy());
        }
        return;
    }
    let addr_x = home_x + home_width + 12;
    let addr_width = (_width as i32 - addr_x - 20).max(200) as u32;
    if x >= addr_x && x < addr_x + addr_width as i32 {
        browser.address_focused = true;
        browser.reset_cursor_blink();

        let text_size = 14.0;
        let text_x = addr_x + 10;
        let rel_x = (x - text_x).max(0) as u32;

        let mut cursor = 0usize;
        let mut accumulated = String::new();
        let mut prev_width = 0u32;
        for (i, ch) in browser.address_text.chars().enumerate() {
            accumulated.push(ch);
            let w = measure_text_width(&accumulated, text_size);
            let mid = (prev_width + w) / 2;
            if rel_x < mid {
                cursor = i;
                break;
            }
            prev_width = w;
            cursor = i + 1;
        }

        browser.address_cursor = cursor;
    }
}

fn handle_key_input(browser: &mut Browser, input: &KeyboardInput, modifiers: ModifiersState) -> bool {
    let key = match input.virtual_keycode {
        Some(k) => k,
        None => return false,
    };

    if browser.address_focused {
        match key {
            VirtualKeyCode::Return => {
                browser.address_focused = false;
                let path = browser.address_text.clone();
                browser.navigate(&path);
                return true;
            }
            VirtualKeyCode::Escape => {
                browser.address_focused = false;
                browser.address_text = browser.current_path.clone();
                browser.address_cursor = browser.address_text.chars().count();
                return true;
            }
            VirtualKeyCode::Left => {
                browser.move_cursor_left();
                return true;
            }
            VirtualKeyCode::Right => {
                browser.move_cursor_right();
                return true;
            }
            VirtualKeyCode::Home => {
                browser.move_cursor_home();
                return true;
            }
            VirtualKeyCode::End => {
                browser.move_cursor_end();
                return true;
            }
            VirtualKeyCode::Back => {
                browser.delete_char_before();
                return true;
            }
            VirtualKeyCode::Delete => {
                browser.delete_char_after();
                return true;
            }
            _ => {}
        }
        return false;
    }

    if let Some(ref mut rt) = browser.runtime {
        if rt.focused_input.is_some() {
            if let VirtualKeyCode::Back = key {
                rt.handle_backspace();
                return true;
            }
        }
    }

    if modifiers.alt() {
        match key {
            VirtualKeyCode::Left => {
                browser.go_back();
                return true;
            }
            VirtualKeyCode::Right => {
                browser.go_forward();
                return true;
            }
            _ => {}
        }
    }

    if key == VirtualKeyCode::F6 {
        browser.address_focused = true;
        browser.address_cursor = browser.address_text.chars().count();
        browser.reset_cursor_blink();
        return true;
    }

    false
}

fn handle_received_char(browser: &mut Browser, ch: char) -> bool {
    if ch.is_control() {
        return false;
    }

    if browser.address_focused {
        browser.insert_char(ch);
        return true;
    }

    if let Some(ref mut rt) = browser.runtime {
        if rt.focused_input.is_some() {
            rt.handle_key(ch);
            return true;
        }
    }

    false
}
