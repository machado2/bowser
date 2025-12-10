//! Renderer for Prism applications
//!
//! Converts the view tree into pixels using a simple software renderer.
//! No GPU dependencies for maximum portability and minimal footprint.

use crate::ast::{ViewNode, NodeKind, PropValue, Color, Value};
use crate::state::StateStore;
use fontdue::{Font, FontSettings};
use fontdue::layout::{Layout, TextStyle, CoordinateSystem, LayoutSettings};

fn lerp_color(c1: u32, c2: u32, t: f32) -> u32 {
    let r1 = ((c1 >> 16) & 0xFF) as f32;
    let g1 = ((c1 >> 8) & 0xFF) as f32;
    let b1 = (c1 & 0xFF) as f32;
    let r2 = ((c2 >> 16) & 0xFF) as f32;
    let g2 = ((c2 >> 8) & 0xFF) as f32;
    let b2 = (c2 & 0xFF) as f32;
    let r = (r1 + (r2 - r1) * t).round() as u32;
    let g = (g1 + (g2 - g1) * t).round() as u32;
    let b = (b1 + (b2 - b1) * t).round() as u32;
    (r << 16) | (g << 8) | b
}

fn mix_color(c1: u32, c2: u32, t: f32) -> u32 {
    let r1 = ((c1 >> 16) & 0xFF) as f32;
    let g1 = ((c1 >> 8) & 0xFF) as f32;
    let b1 = (c1 & 0xFF) as f32;
    let r2 = ((c2 >> 16) & 0xFF) as f32;
    let g2 = ((c2 >> 8) & 0xFF) as f32;
    let b2 = (c2 & 0xFF) as f32;
    let r = (r1 + (r2 - r1) * t).round().clamp(0.0, 255.0) as u32;
    let g = (g1 + (g2 - g1) * t).round().clamp(0.0, 255.0) as u32;
    let b = (b1 + (b2 - b1) * t).round().clamp(0.0, 255.0) as u32;
    (r << 16) | (g << 8) | b
}

/// Pixel buffer for rendering
pub struct FrameBuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}

impl FrameBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0xFFFFFF; width * height], // White background
        }
    }

    pub fn clear(&mut self, color: u32) {
        self.pixels.fill(color);
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = color;
        }
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: u32) {
        let x0 = x;
        let y0 = y;
        let x1 = x + w as i32;
        let y1 = y + h as i32;

        if x1 <= 0 || y1 <= 0 || x0 >= self.width as i32 || y0 >= self.height as i32 {
            return;
        }

        let x_start = x0.max(0) as usize;
        let y_start = y0.max(0) as usize;
        let x_end = x1.min(self.width as i32) as usize;
        let y_end = y1.min(self.height as i32) as usize;

        for py in y_start..y_end {
            for px in x_start..x_end {
                self.pixels[py * self.width + px] = color;
            }
        }
    }

    pub fn draw_rect_outline(&mut self, x: i32, y: i32, w: u32, h: u32, color: u32, thickness: u32) {
        // Top
        self.fill_rect(x, y, w, thickness, color);
        // Bottom
        self.fill_rect(x, y + h as i32 - thickness as i32, w, thickness, color);
        // Left
        self.fill_rect(x, y, thickness, h, color);
        // Right
        self.fill_rect(x + w as i32 - thickness as i32, y, thickness, h, color);
    }

    /// Blend a pixel with alpha
    pub fn blend_pixel(&mut self, x: usize, y: usize, color: u32, alpha: u8) {
        if x >= self.width || y >= self.height {
            return;
        }

        let idx = y * self.width + x;
        let bg = self.pixels[idx];

        let bg_r = (bg >> 16) & 0xFF;
        let bg_g = (bg >> 8) & 0xFF;
        let bg_b = bg & 0xFF;

        let fg_r = (color >> 16) & 0xFF;
        let fg_g = (color >> 8) & 0xFF;
        let fg_b = color & 0xFF;

        let a = alpha as u32;
        let inv_a = 255 - a;

        let r = (fg_r * a + bg_r * inv_a) / 255;
        let g = (fg_g * a + bg_g * inv_a) / 255;
        let b = (fg_b * a + bg_b * inv_a) / 255;

        self.pixels[idx] = (r << 16) | (g << 8) | b;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fill_rounded_rect_vertical_gradient(&mut self, x: i32, y: i32, w: u32, h: u32, radius: u32, top_color: u32, bottom_color: u32) {
        if w == 0 || h == 0 {
            return;
        }

        let x_min = 0i32;
        let y_min = 0i32;
        let x_max = self.width as i32 - 1;
        let y_max = self.height as i32 - 1;

        let x0 = x;
        let y0 = y;
        let x1 = x + w as i32 - 1; // inclusive
        let y1 = y + h as i32 - 1; // inclusive

        let r = (radius.min(w / 2).min(h / 2)) as i32;
        for py in (y0.max(y_min))..=(y1.min(y_max)) {
            let t = if h > 1 { ((py - y0) as f32 / (h as f32 - 1.0)).clamp(0.0, 1.0) } else { 0.0 };
            let color = lerp_color(top_color, bottom_color, t);

            let mut left = x0;
            let mut right = x1;
            if r > 0 {
                if py < y0 + r {
                    let dy = (y0 + r - py) as f32;
                    let dx = ((r * r) as f32 - dy * dy).max(0.0).sqrt().floor() as i32;
                    left = x0 + r - dx;
                    right = x1 - r + dx;
                } else if py > y1 - r {
                    let dy = (py - (y1 - r)) as f32;
                    let dx = ((r * r) as f32 - dy * dy).max(0.0).sqrt().floor() as i32;
                    left = x0 + r - dx;
                    right = x1 - r + dx;
                }
            }

            let xs = left.max(x_min);
            let xe = right.min(x_max);
            if xe < xs {
                continue;
            }
            for px in xs..=xe {
                self.set_pixel(px as usize, py as usize, color);
            }
        }
    }
}

/// Layout box for hit testing
#[derive(Debug, Clone)]
pub struct LayoutBox {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub action: Option<String>,
    pub input_binding: Option<String>,
    pub link_href: Option<String>,
}

/// The renderer
pub struct Renderer {
    font: Font,
    layout: Layout,
    pub layout_boxes: Vec<LayoutBox>,
    pub focused_input: Option<String>,
    pub cursor_visible: bool,
    cursor_blink_timer: u32,
    pub log_enabled: bool,
}

impl Renderer {
    pub fn new() -> Self {
        // Use embedded font data for a clean sans-serif look
        let font_data = include_bytes!("../assets/Inter-Regular.ttf");
        let font = Font::from_bytes(font_data as &[u8], FontSettings {
            scale: 40.0,
            ..FontSettings::default()
        }).expect("Failed to load embedded font");
        
        Self {
            font,
            layout: Layout::new(CoordinateSystem::PositiveYDown),
            layout_boxes: vec![],
            focused_input: None,
            cursor_visible: true,
            cursor_blink_timer: 0,
            log_enabled: false,
        }
    }

    /// Update cursor blink state (call each frame)
    pub fn tick(&mut self) {
        self.cursor_blink_timer += 1;
        if self.cursor_blink_timer >= 30 {  // Toggle every 30 frames (~0.5s at 60fps)
            self.cursor_visible = !self.cursor_visible;
            self.cursor_blink_timer = 0;
        }
    }

    /// Set which input is focused
    pub fn set_focus(&mut self, binding: Option<String>) {
        if self.focused_input != binding {
            self.focused_input = binding;
            self.cursor_visible = true;
            self.cursor_blink_timer = 0;
        }
    }

    pub fn render(&mut self, fb: &mut FrameBuffer, view: &ViewNode, state: &StateStore, scroll_y: i32) {
        fb.clear(0xFFFFFF);
        self.layout_boxes.clear();
        
        let ctx = RenderContext {
            x: 0,
            y: -scroll_y,
            width: fb.width as u32,
            height: fb.height as u32,
        };

        self.render_node(fb, view, state, &ctx);
    }

    pub fn total_content_height(&mut self, view: &ViewNode, state: &StateStore, width: u32) -> u32 {
        let (_, h) = self.measure_node(view, state, width);
        h
    }

    pub fn print_layout_report(&mut self, view: &ViewNode, state: &StateStore, width: u32) {
        self.log_enabled = true;
        self.report_node(view, state, width, 0);
    }

    fn report_node(&mut self, node: &ViewNode, state: &StateStore, width_limit: u32, indent: usize) {
        let (w, h) = self.measure_node(node, state, width_limit);
        let name = match node.kind { 
            NodeKind::Column => "Column",
            NodeKind::Row => "Row",
            NodeKind::Stack => "Stack",
            NodeKind::Grid => "Grid",
            NodeKind::Box => "Box",
            NodeKind::Center => "Center",
            NodeKind::Scroll => "Scroll",
            NodeKind::Text => "Text",
            NodeKind::Markdown => "Markdown",
            NodeKind::Link => "Link",
            NodeKind::Button => "Button",
            NodeKind::Input => "Input",
            NodeKind::TextArea => "TextArea",
            NodeKind::Divider => "Divider",
            NodeKind::Spacer => "Spacer",
            NodeKind::Checkbox => "Checkbox",
            NodeKind::Toggle => "Toggle",
            NodeKind::Radio => "Radio",
            NodeKind::Select => "Select",
            NodeKind::Slider => "Slider",
            NodeKind::Image => "Image",
            NodeKind::Icon => "Icon",
            NodeKind::Video => "Video",
            NodeKind::Audio => "Audio",
            NodeKind::Table => "Table",
            NodeKind::List => "List",
            NodeKind::Card => "Card",
            NodeKind::Badge => "Badge",
            NodeKind::Progress => "Progress",
            NodeKind::Avatar => "Avatar",
            NodeKind::Modal => "Modal",
            NodeKind::Toast => "Toast",
            NodeKind::Tooltip => "Tooltip",
            NodeKind::Popover => "Popover",
            NodeKind::Each => "Each",
            NodeKind::If => "If",
            NodeKind::Show => "Show",
            NodeKind::Switch => "Switch",
            NodeKind::Slot => "Slot",
            NodeKind::Component(_) => "Component",
        };
        let prefix = " ".repeat(indent);
        let extra = if let NodeKind::Button = node.kind { 
            let content = self.get_string_prop(node, "content", state, "");
            let tw = self.line_pixel_width(&content, 14.0).max(self.text_width(&content, 14.0));
            format!(" content='{}' tw={}", content, tw)
        } else if let NodeKind::Text = node.kind { 
            let content = self.get_string_prop(node, "content", state, "");
            format!(" content='{}'", content)
        } else { String::new() };
        println!("{}{} width_limit={} -> (w={}, h={}){}", prefix, name, width_limit, w, h, extra);

        let child_limit = match node.kind {
            NodeKind::Column | NodeKind::Box | NodeKind::Stack | NodeKind::Scroll => {
                let padding = self.get_int_prop(node, "padding", state, 0) as u32;
                width_limit.saturating_sub(padding * 2)
            }
            NodeKind::Row => width_limit,
            NodeKind::Grid => width_limit,
            _ => width_limit,
        };
        for child in &node.children {
            if !self.is_visible(child, state) { continue; }
            self.report_node(child, state, child_limit, indent + 2);
        }
    }

    fn render_node(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        // Check visibility
        if let Some(PropValue::Expression(expr)) = node.props.get("visible") {
            let val = state.evaluate(expr);
            if !val.as_bool() {
                return;
            }
        }

        let padding = self.get_int_prop(node, "padding", state, 0) as u32;
        let gap = self.get_int_prop(node, "gap", state, 0) as u32;
        
        // Get background color
        let bg_color = self.get_color_prop(node, "background", Color::WHITE);

        // Inner context after padding
        let inner = RenderContext {
            x: ctx.x + padding as i32,
            y: ctx.y + padding as i32,
            width: ctx.width.saturating_sub(padding * 2),
            height: ctx.height.saturating_sub(padding * 2),
        };

        // Draw background if not white
        if bg_color != Color::WHITE {
            fb.fill_rect(ctx.x, ctx.y, ctx.width, ctx.height, bg_color.to_u32());
        }

        match &node.kind {
            // Layout nodes
            NodeKind::Column | NodeKind::Stack => {
                self.render_column(fb, node, state, &inner, gap);
            }
            NodeKind::Row => {
                self.render_row(fb, node, state, &inner, gap);
            }
            NodeKind::Grid => {
                self.render_grid(fb, node, state, &inner, gap);
            }
            NodeKind::Center => {
                self.render_center(fb, node, state, &inner);
            }
            NodeKind::Scroll => {
                // Scroll just renders children for now
                for child in &node.children {
                    self.render_node(fb, child, state, &inner);
                }
            }

            // Basic nodes
            NodeKind::Box => {
                for child in &node.children {
                    self.render_node(fb, child, state, &inner);
                }
            }
            NodeKind::Spacer => {
                // Just takes up space
            }
            NodeKind::Divider => {
                self.render_divider(fb, node, state, ctx);
            }

            // Text nodes
            NodeKind::Text | NodeKind::Markdown => {
                self.render_text(fb, node, state, &inner);
            }
            NodeKind::Link => {
                self.render_link(fb, node, state, &inner);
            }

            // Interactive nodes
            NodeKind::Button => {
                self.render_button(fb, node, state, ctx);
            }
            NodeKind::Input => {
                self.render_input(fb, node, state, ctx);
            }
            NodeKind::TextArea => {
                self.render_textarea(fb, node, state, ctx);
            }
            NodeKind::Checkbox => {
                self.render_checkbox(fb, node, state, ctx);
            }
            NodeKind::Toggle => {
                self.render_toggle(fb, node, state, ctx);
            }
            NodeKind::Radio => {
                self.render_radio(fb, node, state, ctx);
            }
            NodeKind::Select => {
                self.render_select(fb, node, state, ctx);
            }
            NodeKind::Slider => {
                self.render_slider(fb, node, state, ctx);
            }

            // Media nodes
            NodeKind::Image => {
                self.render_image(fb, node, state, ctx);
            }
            NodeKind::Icon => {
                self.render_icon(fb, node, state, ctx);
            }
            NodeKind::Video | NodeKind::Audio => {
                self.render_media_placeholder(fb, node, state, ctx);
            }

            // Data display nodes
            NodeKind::Card => {
                self.render_card(fb, node, state, &inner);
            }
            NodeKind::Badge => {
                self.render_badge(fb, node, state, ctx);
            }
            NodeKind::Progress => {
                self.render_progress(fb, node, state, ctx);
            }
            NodeKind::Avatar => {
                self.render_avatar(fb, node, state, ctx);
            }
            NodeKind::Table => {
                self.render_table(fb, node, state, &inner);
            }
            NodeKind::List => {
                self.render_list(fb, node, state, &inner, gap);
            }

            // Feedback nodes
            NodeKind::Modal => {
                self.render_modal(fb, node, state);
            }
            NodeKind::Toast | NodeKind::Tooltip | NodeKind::Popover => {
                // These are typically rendered as overlays - simplified here
                for child in &node.children {
                    self.render_node(fb, child, state, &inner);
                }
            }

            // Control flow nodes
            NodeKind::Each => {
                self.render_each(fb, node, state, &inner, gap);
            }
            NodeKind::If => {
                self.render_if(fb, node, state, &inner);
            }
            NodeKind::Show => {
                // Show always renders (visibility check done above)
                for child in &node.children {
                    self.render_node(fb, child, state, &inner);
                }
            }
            NodeKind::Switch => {
                self.render_switch(fb, node, state, &inner);
            }
            NodeKind::Slot => {
                // Slots are filled by parent component
                for child in &node.children {
                    self.render_node(fb, child, state, &inner);
                }
            }

            // Custom components
            NodeKind::Component(_name) => {
                // Component rendering would look up the component def
                for child in &node.children {
                    self.render_node(fb, child, state, &inner);
                }
            }
        }
    }

    fn render_column(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext, gap: u32) {
        let mut y = ctx.y;

        for child in &node.children {
            if !self.is_visible(child, state) {
                continue;
            }

            let (_, child_h) = self.measure_node(child, state, ctx.width);
            let child_ctx = RenderContext {
                x: ctx.x,
                y,
                width: ctx.width,
                height: child_h,
            };

            self.render_node(fb, child, state, &child_ctx);
            y += child_h as i32 + gap as i32;
        }
    }

    fn render_row(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext, gap: u32) {
        let mut max_h = 0u32;
        let mut total_w = 0u32;

        // Pre-measure children to layout naturally
        let mut measures: Vec<(u32, u32, &ViewNode)> = vec![];
        for child in &node.children {
            if !self.is_visible(child, state) {
                continue;
            }
            let (w, h) = self.measure_node(child, state, ctx.width);
            max_h = max_h.max(h);
            total_w += w;
            measures.push((w, h, child));
        }
        if !measures.is_empty() {
            total_w = total_w.saturating_add(gap * (measures.len() as u32 - 1));
        }

        let mut x = ctx.x + ((ctx.width as i32 - total_w as i32) / 2).max(0);

        for (w, h, child) in measures {
            let child_ctx = RenderContext {
                x,
                y: ctx.y + (max_h as i32 - h as i32) / 2,
                width: w,
                height: h,
            };
            self.render_node(fb, child, state, &child_ctx);
            x += w as i32 + gap as i32;
        }
    }

    fn render_text(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let content = self.get_string_prop(node, "content", state, "");
        if content.is_empty() {
            return;
        }

        let size = self.get_int_prop(node, "size", state, 16) as f32;
        let color = self.get_color_prop(node, "color", Color::BLACK);

        let lines = self.wrap_text(&content, size, ctx.width);
        let (asc, desc, gap) = self.line_metrics(size);
        let line_height = asc + desc + gap;
        let mut y = ctx.y;
        for line in lines {
            let baseline = self.baseline_in_box(y, line_height, size);
            self.draw_text(fb, &line, ctx.x, baseline, size, color.to_u32());
            y += line_height;
        }
    }

    fn render_button(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let content = self.get_string_prop(node, "content", state, "Button");
        let color = self.get_color_prop(node, "color", Color::BLACK);
        let bg = self.get_color_prop(node, "background", Color::LIGHT_GRAY);
        let btn_height = 36u32;
        let text_size = 14.0;
        let tw = self.line_pixel_width(&content, text_size).max(self.text_width(&content, text_size));
        let mut btn_width = tw.saturating_add(24).max(36).min(ctx.width);
        if content.chars().count() <= 2 { btn_width = 36; }
        let btn_x = ctx.x;
        let btn_y = ctx.y + (ctx.height as i32 - btn_height as i32) / 2;

        let top = bg.to_u32();
        let bottom = bg.to_u32();
        fb.fill_rounded_rect_vertical_gradient(btn_x, btn_y, btn_width, btn_height, 10, top, bottom);
        let top_hl = mix_color(top, 0xFFFFFF, 0.15);
        let bot_sh = mix_color(bottom, 0x000000, 0.12);
        fb.fill_rect(btn_x + 2, btn_y + 1, btn_width.saturating_sub(4), 1, top_hl);
        fb.fill_rect(btn_x + 2, btn_y + btn_height as i32 - 2, btn_width.saturating_sub(4), 1, bot_sh);

        if content.chars().count() <= 2 {
            let size = 16.0;
            let lines = self.wrap_text(&content, size, btn_width);
            if let Some(line) = lines.first() {
                self.layout.reset(&LayoutSettings::default());
                self.layout.append(&[&self.font], &TextStyle::new(line, size, 0));
                let mut min_x = f32::MAX;
                let mut min_y = f32::MAX;
                let mut max_x = f32::MIN;
                let mut max_y = f32::MIN;
                for g in self.layout.glyphs() {
                    let (m, _) = self.font.rasterize_config(g.key);
                    min_x = min_x.min(g.x);
                    min_y = min_y.min(g.y);
                    max_x = max_x.max(g.x + m.width as f32);
                    max_y = max_y.max(g.y + m.height as f32);
                }
                let bw = (max_x - min_x).ceil() as i32;
                let bh = (max_y - min_y).ceil() as i32;
                let left = btn_x + (btn_width as i32 - bw) / 2;
                let top = btn_y + (btn_height as i32 - bh) / 2;
                for g in self.layout.glyphs() {
                    let (m, bitmap) = self.font.rasterize_config(g.key);
                    let gx = left + (g.x - min_x).round() as i32;
                    let gy = top + (g.y - min_y).round() as i32;
                    for (i, alpha) in bitmap.iter().enumerate() {
                        if *alpha == 0 { continue; }
                        let px = gx + (i % m.width) as i32;
                        let py = gy + (i / m.width) as i32;
                        if px >= 0 && py >= 0 { fb.blend_pixel(px as usize, py as usize, color.to_u32(), *alpha); }
                    }
                }
            }
        } else {
            let text_x = btn_x + ((btn_width as i32 - tw as i32) / 2).max(0);
            let text_y = self.baseline_in_box(btn_y, btn_height as i32, text_size);
            self.draw_text(fb, &content, text_x, text_y, text_size, color.to_u32());
        }

        // Register layout box for click handling
        if let Some(PropValue::Handler(action)) = node.props.get("on_click") {
            self.layout_boxes.push(LayoutBox {
                x: btn_x,
                y: btn_y,
                width: btn_width,
                height: btn_height,
                action: Some(action.clone()),
                input_binding: None,
                link_href: None,
            });
        }
    }

    fn render_input(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let placeholder = self.get_string_prop(node, "placeholder", state, "");
        let binding = match node.props.get("bind") {
            Some(PropValue::Handler(b)) => Some(b.clone()),
            _ => None,
        };

        // Get current value from state
        let value = binding.as_ref()
            .and_then(|b| state.get(b))
            .map(|v| v.as_string())
            .unwrap_or_default();

        let input_height = 36u32;
        let input_width = ctx.width.saturating_sub(20).min(280);
        let input_x = ctx.x;
        let input_y = ctx.y + (ctx.height as i32 - input_height as i32) / 2;
        let text_size = 14.0;

        // Check if this input is focused
        let is_focused = binding.as_ref()
            .map(|b| self.focused_input.as_ref() == Some(b))
            .unwrap_or(false);

        // Draw input background
        fb.fill_rect(input_x, input_y, input_width, input_height, 0xFFFFFF);
        
        // Draw border (blue if focused)
        let border_color = if is_focused { 0x4285F4 } else { 0xCCCCCC };
        fb.draw_rect_outline(input_x, input_y, input_width, input_height, border_color, if is_focused { 2 } else { 1 });

        // Calculate text area
        let text_x = input_x + 10;
        let text_y = self.baseline_in_box(input_y, input_height as i32, text_size);
        let max_text_width = input_width.saturating_sub(20) as usize;

        // Draw text or placeholder
        if value.is_empty() && !is_focused {
            // Truncate placeholder if too long
            let display_text: String = placeholder.chars().take(max_text_width / 8).collect();
            self.draw_text(fb, &display_text, text_x, text_y, text_size, 0x999999);
        } else {
            // Truncate value if too long (show end of text)
            let display_text: String = if value.len() * 8 > max_text_width {
                value.chars().skip(value.len().saturating_sub(max_text_width / 8)).collect()
            } else {
                value.clone()
            };
            self.draw_text(fb, &display_text, text_x, text_y, text_size, 0x000000);
            
            // Draw cursor if focused
            if is_focused && self.cursor_visible {
                let cursor_x = text_x + self.text_width(&display_text, text_size) as i32;
                let (_, descent, _) = self.line_metrics(text_size);
                let cursor_height = (text_size as i32 + descent).max(14);
                fb.fill_rect(cursor_x, text_y - (text_size as i32), 2, cursor_height as u32, 0x000000);
            }
        }

        // Register layout box for input
        self.layout_boxes.push(LayoutBox {
            x: input_x,
            y: input_y,
            width: input_width,
            height: input_height,
            action: None,
            input_binding: binding,
            link_href: None,
        });
    }

    // ========================================================================
    // Additional render methods for new node types
    // ========================================================================

    fn render_grid(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext, gap: u32) {
        let cols = self.get_int_prop(node, "columns", state, 2) as usize;
        let visible: Vec<&ViewNode> = node.children.iter()
            .filter(|c| self.is_visible(c, state))
            .collect();
        
        if visible.is_empty() || cols == 0 {
            return;
        }

        let rows = visible.len().div_ceil(cols);
        let cell_width = (ctx.width - gap * (cols as u32 - 1)) / cols as u32;
        let cell_height = (ctx.height - gap * (rows as u32 - 1)) / rows as u32;

        for (i, child) in visible.into_iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let child_ctx = RenderContext {
                x: ctx.x + (col as u32 * (cell_width + gap)) as i32,
                y: ctx.y + (row as u32 * (cell_height + gap)) as i32,
                width: cell_width,
                height: cell_height,
            };
            self.render_node(fb, child, state, &child_ctx);
        }
    }

    fn render_center(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        for child in &node.children {
            let (cw, ch) = self.measure_node(child, state, ctx.width);
            let centered = RenderContext {
                x: ctx.x + ((ctx.width as i32 - cw as i32) / 2).max(0),
                y: ctx.y,
                width: cw,
                height: ch,
            };
            self.render_node(fb, child, state, &centered);
        }
    }

    fn render_divider(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let color = self.get_color_prop(node, "color", Color::LIGHT_GRAY);
        let vertical = self.get_string_prop(node, "direction", state, "horizontal") == "vertical";
        
        if vertical {
            let x = ctx.x + ctx.width as i32 / 2;
            fb.fill_rect(x, ctx.y, 1, ctx.height, color.to_u32());
        } else {
            let y = ctx.y + ctx.height as i32 / 2;
            fb.fill_rect(ctx.x, y, ctx.width, 1, color.to_u32());
        }
    }

    fn render_link(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let content = self.get_string_prop(node, "content", state, "Link");
        let href = self.get_string_prop(node, "href", state, "");
        let size = self.get_int_prop(node, "size", state, 16) as f32;
        
        // Links rendered in blue
        let lines = self.wrap_text(&content, size, ctx.width);
        let (ascent, descent, gap) = self.line_metrics(size);
        let line_height = ascent + descent + gap;
        let mut y = ctx.y;
        let mut max_w = 0u32;
        for line in &lines {
            let w = self.line_pixel_width(line, size).min(ctx.width);
            max_w = max_w.max(w);
            let baseline = self.baseline_in_box(y, line_height, size);
            self.draw_text(fb, line, ctx.x, baseline, size, 0x1976D2);
            fb.fill_rect(ctx.x, baseline + 2, w, 1, 0x1976D2);
            y += line_height;
        }
        let link_height = (lines.len() as u32 * line_height as u32).max(16);
        
        // Register as clickable if has href
        if !href.is_empty() {
            self.layout_boxes.push(LayoutBox {
                x: ctx.x,
                y: ctx.y,
                width: max_w.max(20),
                height: link_height,
                action: None,
                input_binding: None,
                link_href: Some(href),
            });
        }
    }

    fn render_textarea(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let placeholder = self.get_string_prop(node, "placeholder", state, "");
        let binding = match node.props.get("bind") {
            Some(PropValue::Handler(b)) => Some(b.clone()),
            _ => None,
        };

        let value = binding.as_ref()
            .and_then(|b| state.get(b))
            .map(|v| v.as_string())
            .unwrap_or_default();

        let area_height = self.get_int_prop(node, "height", state, 100) as u32;
        let area_width = ctx.width.min(400);

        fb.fill_rect(ctx.x, ctx.y, area_width, area_height, 0xFFFFFF);
        fb.draw_rect_outline(ctx.x, ctx.y, area_width, area_height, 0xCCCCCC, 1);

        let text = if value.is_empty() { &placeholder } else { &value };
        let color = if value.is_empty() { 0x999999 } else { 0x000000 };
        self.draw_text(fb, text, ctx.x + 8, ctx.y + 8, 14.0, color);

        self.layout_boxes.push(LayoutBox {
            x: ctx.x,
            y: ctx.y,
            width: area_width,
            height: area_height,
            action: None,
            input_binding: binding,
            link_href: None,
        });
    }

    fn render_checkbox(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let checked = self.get_bool_prop(node, "checked", state, false);
        let label = self.get_string_prop(node, "label", state, "");
        
        let box_size = 20u32;
        let box_y = ctx.y + (ctx.height as i32 - box_size as i32) / 2;

        // Draw checkbox
        fb.draw_rect_outline(ctx.x, box_y, box_size, box_size, 0x666666, 1);
        if checked {
            fb.fill_rect(ctx.x + 4, box_y + 4, box_size - 8, box_size - 8, 0x4285F4);
        }

        // Draw label
        if !label.is_empty() {
            self.draw_text(fb, &label, ctx.x + box_size as i32 + 8, box_y + 3, 14.0, 0x333333);
        }

        if let Some(PropValue::Handler(action)) = node.props.get("on_change") {
            self.layout_boxes.push(LayoutBox {
                x: ctx.x,
                y: box_y,
                width: box_size + 8 + (label.len() as u32 * 8),
                height: box_size,
                action: Some(action.clone()),
                input_binding: None,
                link_href: None,
            });
        }
    }

    fn render_toggle(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let on = self.get_bool_prop(node, "value", state, false);
        
        let track_width = 44u32;
        let track_height = 24u32;
        let track_y = ctx.y + (ctx.height as i32 - track_height as i32) / 2;

        // Track
        let track_color = if on { 0x4285F4 } else { 0xCCCCCC };
        fb.fill_rect(ctx.x, track_y, track_width, track_height, track_color);

        // Thumb
        let thumb_x = if on { ctx.x + track_width as i32 - 22 } else { ctx.x + 2 };
        fb.fill_rect(thumb_x, track_y + 2, 20, 20, 0xFFFFFF);

        if let Some(PropValue::Handler(action)) = node.props.get("on_change") {
            self.layout_boxes.push(LayoutBox {
                x: ctx.x,
                y: track_y,
                width: track_width,
                height: track_height,
                action: Some(action.clone()),
                input_binding: None,
                link_href: None,
            });
        }
    }

    fn render_radio(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let selected = self.get_bool_prop(node, "selected", state, false);
        let label = self.get_string_prop(node, "label", state, "");
        
        let radius = 10i32;
        let cy = ctx.y + ctx.height as i32 / 2;

        // Draw circle (simplified as square for now)
        fb.draw_rect_outline(ctx.x, cy - radius, radius as u32 * 2, radius as u32 * 2, 0x666666, 1);
        if selected {
            fb.fill_rect(ctx.x + 5, cy - 5, 10, 10, 0x4285F4);
        }

        if !label.is_empty() {
            self.draw_text(fb, &label, ctx.x + radius * 2 + 8, cy - 7, 14.0, 0x333333);
        }
    }

    fn render_select(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let value = self.get_string_prop(node, "value", state, "Select...");
        
        let select_height = 36u32;
        let select_width = ctx.width.min(200);

        fb.fill_rect(ctx.x, ctx.y, select_width, select_height, 0xFFFFFF);
        fb.draw_rect_outline(ctx.x, ctx.y, select_width, select_height, 0xCCCCCC, 1);
        self.draw_text(fb, &value, ctx.x + 8, ctx.y + 10, 14.0, 0x333333);
        // Arrow indicator
        self.draw_text(fb, "▼", ctx.x + select_width as i32 - 20, ctx.y + 10, 12.0, 0x666666);
    }

    fn render_slider(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let value = self.get_int_prop(node, "value", state, 50) as f32;
        let min = self.get_int_prop(node, "min", state, 0) as f32;
        let max = self.get_int_prop(node, "max", state, 100) as f32;
        
        let track_height = 4u32;
        let track_y = ctx.y + ctx.height as i32 / 2 - 2;
        let track_width = ctx.width.min(200);

        // Track
        fb.fill_rect(ctx.x, track_y, track_width, track_height, 0xE0E0E0);

        // Filled portion
        let ratio = ((value - min) / (max - min)).clamp(0.0, 1.0);
        let filled_width = (track_width as f32 * ratio) as u32;
        fb.fill_rect(ctx.x, track_y, filled_width, track_height, 0x4285F4);

        // Thumb
        let thumb_x = ctx.x + filled_width as i32 - 8;
        fb.fill_rect(thumb_x, track_y - 6, 16, 16, 0x4285F4);
    }

    fn render_image(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let alt = self.get_string_prop(node, "alt", state, "Image");
        let width = self.get_int_prop(node, "width", state, 100) as u32;
        let height = self.get_int_prop(node, "height", state, 100) as u32;

        // Placeholder for image
        fb.fill_rect(ctx.x, ctx.y, width.min(ctx.width), height.min(ctx.height), 0xE0E0E0);
        self.draw_text(fb, &alt, ctx.x + 8, ctx.y + 8, 12.0, 0x666666);
    }

    fn render_icon(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let name = self.get_string_prop(node, "name", state, "?");
        let size = self.get_int_prop(node, "size", state, 24) as f32;
        let color = self.get_color_prop(node, "color", Color::BLACK);
        
        // Render icon name as placeholder
        self.draw_text(fb, &name, ctx.x, ctx.y, size, color.to_u32());
    }

    fn render_media_placeholder(&mut self, fb: &mut FrameBuffer, _node: &ViewNode, _state: &StateStore, ctx: &RenderContext) {
        fb.fill_rect(ctx.x, ctx.y, ctx.width.min(320), ctx.height.min(180), 0x333333);
        self.draw_text(fb, "▶ Media", ctx.x + 10, ctx.y + 10, 14.0, 0xFFFFFF);
    }

    fn render_card(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        // Card with shadow effect (simplified)
        fb.fill_rect(ctx.x + 2, ctx.y + 2, ctx.width, ctx.height, 0xDDDDDD); // Shadow
        fb.fill_rect(ctx.x, ctx.y, ctx.width, ctx.height, 0xFFFFFF);
        fb.draw_rect_outline(ctx.x, ctx.y, ctx.width, ctx.height, 0xE0E0E0, 1);
        
        let inner = RenderContext {
            x: ctx.x + 16,
            y: ctx.y + 16,
            width: ctx.width.saturating_sub(32),
            height: ctx.height.saturating_sub(32),
        };
        for child in &node.children {
            self.render_node(fb, child, state, &inner);
        }
    }

    fn render_badge(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let content = self.get_string_prop(node, "content", state, "0");
        let bg = self.get_color_prop(node, "background", Color::RED);
        
        let badge_width = (content.len() as u32 * 10 + 16).max(28);
        let badge_height = 24u32;
        let badge_y = ctx.y + (ctx.height as i32 - badge_height as i32) / 2;
        
        fb.fill_rect(ctx.x, badge_y, badge_width, badge_height, bg.to_u32());
        self.draw_text(fb, &content, ctx.x + 8, badge_y + 5, 14.0, 0xFFFFFF);
    }

    fn render_progress(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let value = self.get_int_prop(node, "value", state, 0) as f32;
        let max = self.get_int_prop(node, "max", state, 100) as f32;
        
        let bar_height = 8u32;
        let bar_y = ctx.y + ctx.height as i32 / 2 - 4;

        fb.fill_rect(ctx.x, bar_y, ctx.width, bar_height, 0xE0E0E0);
        
        // Avoid division by zero - if max is 0, show empty bar
        let ratio = if max > 0.0 { (value / max).clamp(0.0, 1.0) } else { 0.0 };
        let filled = (ctx.width as f32 * ratio) as u32;
        if filled > 0 {
            fb.fill_rect(ctx.x, bar_y, filled, bar_height, 0x4CAF50);
        }
    }

    fn render_avatar(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        let size = self.get_int_prop(node, "size", state, 40) as u32;
        let name = self.get_string_prop(node, "name", state, "?");
        let initial = name.chars().next().unwrap_or('?').to_uppercase().to_string();

        // Circle placeholder (rendered as rounded rect)
        fb.fill_rect(ctx.x, ctx.y, size, size, 0x9E9E9E);
        self.draw_text(fb, &initial, ctx.x + size as i32 / 3, ctx.y + size as i32 / 4, size as f32 / 2.0, 0xFFFFFF);
    }

    fn render_table(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        // Simplified table - render children as rows
        let mut y = ctx.y;
        let row_height = 36u32;
        
        for child in &node.children {
            let row_ctx = RenderContext {
                x: ctx.x,
                y,
                width: ctx.width,
                height: row_height,
            };
            fb.draw_rect_outline(ctx.x, y, ctx.width, row_height, 0xE0E0E0, 1);
            self.render_node(fb, child, state, &row_ctx);
            y += row_height as i32;
        }
    }

    fn render_list(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext, gap: u32) {
        // List renders like column
        self.render_column(fb, node, state, ctx, gap);
    }

    fn render_modal(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore) {
        let visible = self.get_bool_prop(node, "open", state, false);
        if !visible {
            return;
        }

        // Overlay
        for pixel in fb.pixels.iter_mut() {
            let r = ((*pixel >> 16) & 0xFF) / 2;
            let g = ((*pixel >> 8) & 0xFF) / 2;
            let b = (*pixel & 0xFF) / 2;
            *pixel = (r << 16) | (g << 8) | b;
        }

        // Modal box
        let modal_width = 400u32.min(fb.width as u32 - 40);
        let modal_height = 300u32.min(fb.height as u32 - 40);
        let modal_x = (fb.width as i32 - modal_width as i32) / 2;
        let modal_y = (fb.height as i32 - modal_height as i32) / 2;

        fb.fill_rect(modal_x, modal_y, modal_width, modal_height, 0xFFFFFF);
        fb.draw_rect_outline(modal_x, modal_y, modal_width, modal_height, 0xCCCCCC, 1);

        let inner = RenderContext {
            x: modal_x + 20,
            y: modal_y + 20,
            width: modal_width - 40,
            height: modal_height - 40,
        };
        for child in &node.children {
            self.render_node(fb, child, state, &inner);
        }
    }

    fn render_each(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext, gap: u32) {
        // Each iterates over a list and renders children for each item
        // This requires special handling with the state store
        // For now, just render children
        self.render_column(fb, node, state, ctx, gap);
    }

    fn render_if(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        // Condition is checked in visibility, render children
        for child in &node.children {
            self.render_node(fb, child, state, ctx);
        }
    }

    fn render_switch(&mut self, fb: &mut FrameBuffer, node: &ViewNode, state: &StateStore, ctx: &RenderContext) {
        // Switch renders first matching case
        // Simplified - render first child
        if let Some(child) = node.children.first() {
            self.render_node(fb, child, state, ctx);
        }
    }

    fn get_bool_prop(&self, node: &ViewNode, name: &str, state: &StateStore, default: bool) -> bool {
        match node.props.get(name) {
            Some(PropValue::Static(Value::Bool(b))) => *b,
            Some(PropValue::Expression(expr)) => state.evaluate(expr).as_bool(),
            _ => default,
        }
    }

    fn draw_text(&mut self, fb: &mut FrameBuffer, text: &str, x: i32, y: i32, size: f32, color: u32) {
        self.layout.reset(&LayoutSettings {
            x: x as f32,
            y: 0.0,
            ..LayoutSettings::default()
        });
        self.layout.append(&[&self.font], &TextStyle::new(text, size, 0));
        let baseline_in_layout = self
            .layout
            .lines()
            .and_then(|lines| lines.first().map(|l| l.baseline_y.round() as i32))
            .unwrap_or(0);
        let dy = y - baseline_in_layout;

        for glyph in self.layout.glyphs() {
            let (metrics, bitmap) = self.font.rasterize_config(glyph.key);
            let gx = glyph.x.round() as i32;
            let gy = glyph.y.round() as i32 + dy;

            for (i, alpha) in bitmap.iter().enumerate() {
                if *alpha == 0 {
                    continue;
                }
                let px = gx + (i % metrics.width) as i32;
                let py = gy + (i / metrics.width) as i32;
                if px >= 0 && py >= 0 {
                    fb.blend_pixel(px as usize, py as usize, color, *alpha);
                }
            }
        }
    }

    fn get_int_prop(&self, node: &ViewNode, name: &str, state: &StateStore, default: i64) -> i64 {
        match node.props.get(name) {
            Some(PropValue::Static(Value::Int(i))) => *i,
            Some(PropValue::Static(Value::Float(f))) => *f as i64,
            Some(PropValue::Expression(expr)) => state.evaluate(expr).as_int(),
            _ => default,
        }
    }

    fn get_string_prop(&self, node: &ViewNode, name: &str, state: &StateStore, default: &str) -> String {
        match node.props.get(name) {
            Some(PropValue::Static(Value::String(s))) => s.clone(),
            Some(PropValue::Expression(expr)) => state.evaluate(expr).as_string(),
            _ => default.to_string(),
        }
    }

    fn get_color_prop(&self, node: &ViewNode, name: &str, default: Color) -> Color {
        match node.props.get(name) {
            Some(PropValue::Color(c)) => *c,
            Some(PropValue::Static(Value::String(s))) => {
                Color::from_hex(s).unwrap_or(default)
            }
            _ => default,
        }
    }

    fn is_visible(&self, node: &ViewNode, state: &StateStore) -> bool {
        match node.props.get("visible") {
            Some(PropValue::Expression(expr)) => state.evaluate(expr).as_bool(),
            Some(PropValue::Static(Value::Bool(b))) => *b,
            _ => true,
        }
    }

    /// Find what was clicked at given coordinates
    pub fn hit_test(&self, x: i32, y: i32) -> Option<&LayoutBox> {
        self.layout_boxes.iter().find(|&layout_box| x >= layout_box.x
                && x < layout_box.x + layout_box.width as i32
                && y >= layout_box.y
                && y < layout_box.y + layout_box.height as i32).map(|v| v as _)
    }

    /// Rough measurement for node size to drive layout without overlapping
    fn measure_node(&self, node: &ViewNode, state: &StateStore, width_limit: u32) -> (u32, u32) {
        match node.kind {
            // Layout nodes - derive from children
            NodeKind::Column | NodeKind::Box | NodeKind::Stack | NodeKind::Scroll => {
                let gap = self.get_int_prop(node, "gap", state, 0) as u32;
                let padding = self.get_int_prop(node, "padding", state, 0) as u32;
                let mut total_h = padding * 2;
                let mut max_w = 0u32;
                let mut count = 0;
                for child in &node.children {
                    if !self.is_visible(child, state) {
                        continue;
                    }
                    count += 1;
                    let (cw, ch) = self.measure_node(child, state, width_limit.saturating_sub(padding * 2));
                    max_w = max_w.max(cw);
                    total_h += ch;
                }
                if count > 0 {
                    total_h += gap * (count - 1) as u32;
                }
                (max_w + padding * 2, total_h)
            }
            NodeKind::Row => {
                let gap = self.get_int_prop(node, "gap", state, 0) as u32;
                let padding = self.get_int_prop(node, "padding", state, 0) as u32;
                let mut total_w = padding * 2;
                let mut max_h = 0u32;
                let mut count = 0;
                for child in &node.children {
                    if !self.is_visible(child, state) {
                        continue;
                    }
                    count += 1;
                    let (cw, ch) = self.measure_node(child, state, width_limit.saturating_sub(padding * 2));
                    total_w += cw;
                    max_h = max_h.max(ch);
                }
                if count > 0 {
                    total_w += gap * (count - 1) as u32;
                }
                (total_w, max_h + padding * 2)
            }
            NodeKind::Grid => {
                let cols = self.get_int_prop(node, "columns", state, 2).max(1) as usize;
                let gap = self.get_int_prop(node, "gap", state, 0) as u32;
                let padding = self.get_int_prop(node, "padding", state, 0) as u32;
                let mut child_sizes: Vec<(u32, u32)> = vec![];
                for child in &node.children {
                    if !self.is_visible(child, state) {
                        continue;
                    }
                    child_sizes.push(self.measure_node(child, state, width_limit.saturating_sub(padding * 2)));
                }
                if child_sizes.is_empty() {
                    return (0, 0);
                }
                let rows = child_sizes.len().div_ceil(cols);
                let max_w = child_sizes.iter().map(|(w, _)| *w).max().unwrap_or(0);
                let max_h = child_sizes.iter().map(|(_, h)| *h).max().unwrap_or(0);
                let total_w = max_w * cols as u32 + gap * (cols.saturating_sub(1) as u32) + padding * 2;
                let total_h = max_h * rows as u32 + gap * (rows.saturating_sub(1) as u32) + padding * 2;
                (total_w.min(width_limit), total_h)
            }
            // Basic nodes
            NodeKind::Divider => (width_limit, 1),
            NodeKind::Spacer => (0, 0),
            // Text nodes
            NodeKind::Text | NodeKind::Markdown => {
                let content = self.get_string_prop(node, "content", state, "");
                let size = self.get_int_prop(node, "size", state, 16) as f32;
                let lines = self.wrap_text(&content, size, width_limit);
                let line_height = size as u32 + 6;
                let line_count = lines.len().max(1) as u32;
                let mut max_w = 0u32;
                for line in &lines {
                    max_w = max_w.max(self.text_width(line, size).min(width_limit));
                }
                let height = line_height * line_count;
                (max_w, height)
            }
            NodeKind::Link => {
                let content = self.get_string_prop(node, "content", state, "Link");
                let size = self.get_int_prop(node, "size", state, 16) as f32;
                let lines = self.wrap_text(&content, size, width_limit);
                let line_height = size as u32 + 6;
                let line_count = lines.len().max(1) as u32;
                let mut max_w = 0u32;
                for line in &lines {
                    max_w = max_w.max(self.text_width(line, size).min(width_limit));
                }
                let height = line_height * line_count;
                (max_w, height)
            }
            // Interactive nodes
            NodeKind::Button => {
                let content = self.get_string_prop(node, "content", state, "Button");
                let size = 14.0;
                let base_w = self.text_width(&content, size);
                let mut w = base_w.saturating_add(24).max(36).min(width_limit);
                if content.chars().count() <= 2 { w = 36; }
                if self.log_enabled { println!("measure Button content='{}' base_w={} limit={} -> w={}", content, base_w, width_limit, w); }
                (w, 36)
            }
            NodeKind::Input => (width_limit.min(280), 36),
            NodeKind::TextArea => {
                let h = self.get_int_prop(node, "height", state, 100) as u32;
                (width_limit.min(400), h)
            }
            NodeKind::Checkbox | NodeKind::Toggle | NodeKind::Radio => {
                let label = self.get_string_prop(node, "label", state, "");
                let w = (label.len() as u32 * 8 + 32).min(width_limit);
                (w, 24)
            }
            NodeKind::Select | NodeKind::Slider => (width_limit.min(240), 32),
            // Media/Data display/feedback defaults
            NodeKind::Image | NodeKind::Icon | NodeKind::Avatar => (64, 64),
            NodeKind::Video | NodeKind::Audio => (width_limit, 120),
            NodeKind::Table | NodeKind::List | NodeKind::Card => (width_limit, 120),
            NodeKind::Badge => (48, 24),
            NodeKind::Progress => (width_limit, 16),
            NodeKind::Modal | NodeKind::Toast | NodeKind::Tooltip | NodeKind::Popover => (width_limit, 40),
            // Control flow nodes: measure children
            NodeKind::Each | NodeKind::If | NodeKind::Show | NodeKind::Switch | NodeKind::Slot => {
                let mut max_w = 0;
                let mut total_h = 0;
                let mut count = 0;
                for child in &node.children {
                    if !self.is_visible(child, state) {
                        continue;
                    }
                    count += 1;
                    let (cw, ch) = self.measure_node(child, state, width_limit);
                    max_w = max_w.max(cw);
                    total_h += ch;
                }
                if count > 1 {
                    total_h += (count - 1) as u32 * 4;
                }
                (max_w, total_h)
            }
            NodeKind::Center => (width_limit, 0),
            NodeKind::Component(_) => (width_limit, 0),
        }
    }

    fn text_width(&self, content: &str, size: f32) -> u32 {
        let avg = size * 0.55;
        ((content.len() as f32 * avg) as u32).saturating_add(4)
    }

    fn line_pixel_width(&mut self, content: &str, size: f32) -> u32 {
        if content.is_empty() {
            return 0;
        }

        self.layout.reset(&LayoutSettings::default());
        self.layout.append(&[&self.font], &TextStyle::new(content, size, 0));
        let glyphs = self.layout.glyphs();
        if glyphs.is_empty() {
            return 0;
        }

        let first = &glyphs[0];
        let last = &glyphs[glyphs.len() - 1];
        let start_x = first.x.floor() as i32;
        let end_x = (last.x + last.width as f32).ceil() as i32;
        if end_x <= start_x {
            0
        } else {
            (end_x - start_x) as u32
        }
    }

    fn line_metrics(&self, size: f32) -> (i32, i32, i32) {
        // Try to reuse the renderer's font metrics if available
        if let Some(m) = self.font.horizontal_line_metrics(size) {
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

    fn baseline_in_box(&self, top: i32, height: i32, size: f32) -> i32 {
        let (ascent, descent_abs, line_gap) = self.line_metrics(size);
        let line_h = ascent + descent_abs + line_gap;
        let offset = (height - line_h).max(0) / 2;
        top + offset + ascent
    }

    /// Simple word-wrapping helper
    fn wrap_text(&self, content: &str, size: f32, width_limit: u32) -> Vec<String> {
        if content.is_empty() || width_limit == 0 {
            return vec![];
        }

        let mut lines: Vec<String> = vec![];
        let mut current = String::new();
        let mut current_width = 0u32;
        let space_width = self.text_width(" ", size);

        for word in content.split_whitespace() {
            let word_width = self.text_width(word, size);
            if current.is_empty() {
                current.push_str(word);
                current_width = word_width;
            } else if current_width + space_width + word_width <= width_limit {
                current.push(' ');
                current.push_str(word);
                current_width += space_width + word_width;
            } else {
                lines.push(current);
                current = word.to_string();
                current_width = word_width;
            }
        }

        if !current.is_empty() {
            lines.push(current);
        }

        lines
    }
}

/// Context for rendering, defines the available space
#[derive(Clone)]
struct RenderContext {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}
