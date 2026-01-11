use macroquad::{
    color::{Color, DARKGRAY, DARKGREEN, GRAY, LIGHTGRAY, MAGENTA, ORANGE, WHITE},
    input::{
        is_mouse_button_down, is_mouse_button_pressed, mouse_position, mouse_wheel, MouseButton,
    },
    math::Rect,
    prelude::TextDimensions,
    shapes::{draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_lines},
    text::{draw_text_ex, measure_text, Font, TextParams},
    texture::{draw_texture, draw_texture_ex, DrawTextureParams, Texture2D},
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use crate::{
    textures::{DICE_SYMBOL, SHIELD_SYMBOL},
    util::{COL_ALICE, COL_BOB, COL_CLARA},
};

pub trait Drawable {
    fn draw(&self, x: f32, y: f32);
    fn size(&self) -> (f32, f32);

    fn draw_tooltips(&self, _x: f32, _y: f32) {}
}

pub enum Element {
    Empty(f32, f32),
    Texture(Texture2D, Option<(f32, f32)>),
    ResizableTexture(Texture2D, Rc<RefCell<(f32, f32)>>),
    Container(Container),
    Text(TextLine),
    Circle(Circle),
    Rect(Rectangle),
    TabLink(TabLink),
    Box(Box<dyn Drawable>),
    RcRefCell(Rc<RefCell<dyn Drawable>>),
    WeakRefCell(Weak<RefCell<dyn Drawable>>),
    Rc(Rc<dyn Drawable>),
}

impl Element {
    pub fn size(&self) -> (f32, f32) {
        let size = match self {
            Element::Empty(w, h) => (*w, *h),
            Element::Texture(texture, dest_size) => dest_size.unwrap_or(texture.size().into()),
            Element::ResizableTexture(_texture, dest_size) => *dest_size.borrow(),
            Element::Container(container) => container.size(),
            Element::Text(text) => text.size(),
            Element::Circle(circle) => circle.size(),
            Element::Rect(rect) => rect.size,
            Element::TabLink(link) => link.size,
            Element::Box(drawable) => drawable.size(),
            Element::RcRefCell(drawable) => drawable.borrow().size(),
            Element::Rc(drawable) => drawable.size(),
            Element::WeakRefCell(drawable) => drawable.upgrade().unwrap().borrow().size(),
        };

        assert!(size.0.is_finite() && size.1.is_finite());
        size
    }

    pub fn draw(&self, x: f32, y: f32) {
        match self {
            Element::Empty(..) => {}
            Element::Texture(texture, dest_size) => {
                let dest_size = dest_size.map(|s| s.into());
                let params = DrawTextureParams {
                    dest_size,
                    ..Default::default()
                };
                draw_texture_ex(texture, x, y, WHITE, params);
            }
            Element::ResizableTexture(texture, dest_size) => {
                let dest_size = *dest_size.borrow();
                let params = DrawTextureParams {
                    dest_size: Some(dest_size.into()),
                    ..Default::default()
                };
                draw_texture_ex(texture, x, y, WHITE, params);
            }
            Element::Container(container) => {
                container.draw(x, y);
            }
            Element::Text(text) => text.draw(x, y),
            Element::Circle(circle) => circle.draw(x, y),
            Element::Rect(rect) => rect.draw(x, y),
            Element::TabLink(link) => link.draw(x, y),
            Element::Box(drawable) => drawable.draw(x, y),
            Element::RcRefCell(drawable) => drawable.borrow().draw(x, y),
            Element::Rc(drawable) => drawable.draw(x, y),
            Element::WeakRefCell(drawable) => drawable.upgrade().unwrap().borrow().draw(x, y),
        }
    }

    pub fn draw_tooltips(&self, x: f32, y: f32) {
        match self {
            Element::Container(container) => container.draw_tooltips(x, y),
            Element::Box(drawable) => drawable.draw_tooltips(x, y),
            Element::RcRefCell(drawable) => drawable.borrow().draw_tooltips(x, y),
            Element::Rc(drawable) => drawable.draw_tooltips(x, y),
            Element::WeakRefCell(drawable) => {
                drawable.upgrade().unwrap().borrow().draw_tooltips(x, y)
            }
            _ => {}
        }
    }

    fn unwrap_tab_link(&mut self) -> &mut TabLink {
        match self {
            Element::TabLink(tab_link) => tab_link,
            _ => panic!("Unexpected variant"),
        }
    }
}

pub struct Tabs {
    links: Container,
    tabs: Vec<Element>,
    active_i: usize,
}

impl Tabs {
    pub fn new(active_i: usize, links_and_tabs: Vec<(&'static str, Element)>, font: Font) -> Self {
        let mut links: Vec<TabLink> = links_and_tabs
            .iter()
            .map(|t| TabLink::new(t.0, font.clone()))
            .collect();

        links[active_i].active = true;
        let links_row = Container {
            layout_dir: LayoutDirection::Horizontal,
            children: links.into_iter().map(Element::TabLink).collect(),
            ..Default::default()
        };

        let tabs: Vec<Element> = links_and_tabs.into_iter().map(|t| t.1).collect();
        Self {
            links: links_row,
            tabs,
            active_i,
        }
    }

    pub fn draw(&mut self, x: f32, y: f32) {
        // If a link was clicked, update the state of all links
        let mut maybe_clicked_i = None;
        for (i, link) in self.links.children.iter_mut().enumerate() {
            if link.unwrap_tab_link().was_clicked.get() {
                maybe_clicked_i = Some(i);
                self.active_i = i;
                break;
            }
        }
        if let Some(clicked_i) = maybe_clicked_i {
            for (i, element) in self.links.children.iter_mut().enumerate() {
                let tab_link = element.unwrap_tab_link();
                tab_link.was_clicked.set(false);
                tab_link.active = i == clicked_i;
            }
        }

        self.links.draw(x, y);

        self.tabs[self.active_i].draw(x, y + 40.0);
    }
}

pub struct TabLink {
    text: TextLine,
    active: bool,
    padding: f32,
    size: (f32, f32),
    was_clicked: Cell<bool>,
}

impl TabLink {
    pub fn new(text: impl Into<String>, font: Font) -> Self {
        let text = TextLine::new(text, 20, WHITE, Some(font));
        let padding = 5.0;
        let text_size = text.size();
        Self {
            text,
            active: false,
            padding,
            size: (padding * 2.0 + text_size.0, padding * 2.0 + text_size.1),
            was_clicked: Cell::new(false),
        }
    }

    pub fn draw(&self, x: f32, y: f32) {
        if self.active {
            draw_rectangle(x, y, self.size.0, self.size.1, DARKGREEN);
        }

        if is_mouse_button_pressed(MouseButton::Left) {
            let (mouse_x, mouse_y) = mouse_position();
            if (x..=x + self.size.0).contains(&mouse_x) && (y..=y + self.size.1).contains(&mouse_y)
            {
                self.was_clicked.set(true);
            }
        }

        self.text.draw(x + self.padding, y + self.padding);
    }
}

pub struct TextLine {
    size: (f32, f32),
    string: String,
    offset_y: f32,
    font_size: u16,
    color: Color,
    hover_color: Option<Color>,
    min_height: f32,
    min_width: f32,
    max_width: f32,
    right_align: bool,
    hor_padding: f32,
    vert_padding: f32,
    font: Option<Font>,
    depth: Option<(Color, f32)>,

    pub has_been_hovered: Cell<Option<(f32, f32)>>,
}

impl TextLine {
    pub fn new(
        string: impl Into<String>,
        font_size: u16,
        color: Color,
        font: Option<Font>,
    ) -> Self {
        let mut this = Self {
            size: (0.0, 0.0),
            string: string.into(),
            offset_y: 0.0,
            font_size,
            color,
            hover_color: None,
            min_height: 0.0,
            min_width: 0.0,
            max_width: f32::MAX,
            right_align: false,
            hor_padding: 0.0,
            vert_padding: 0.0,
            font,
            depth: None,
            has_been_hovered: Cell::new(None),
        };
        this.measure();
        this
    }

    pub fn set_min_height(&mut self, min_height: f32) {
        self.min_height = min_height;
        self.size.1 = self.size.1.max(min_height);
    }

    pub fn set_min_width(&mut self, min_width: f32, right_align: bool) {
        self.min_width = min_width;
        self.right_align = right_align;
        self.size.0 = self.size.0.max(min_width);
    }

    pub fn set_max_width(&mut self, max_width: f32) {
        self.max_width = max_width;
        self.size.0 = self.size.0.min(max_width);
    }

    pub fn set_depth(&mut self, color: Color, offset: f32) {
        self.depth = Some((color, offset));
    }

    pub fn with_depth(mut self, color: Color, offset: f32) -> Self {
        self.set_depth(color, offset);
        self
    }

    pub fn set_padding(&mut self, hor: f32, vert: f32) {
        self.hor_padding = hor;
        self.vert_padding = vert;
        self.measure();
    }

    pub fn set_color(&mut self, color: Color) {
        self.color = color;
    }

    pub fn with_padding(mut self, hor: f32, vert: f32) -> Self {
        self.set_padding(hor, vert);
        self
    }

    pub fn with_hover_color(mut self, color: Color) -> Self {
        self.hover_color = Some(color);
        self
    }

    pub fn set_string(&mut self, string: impl Into<String>) {
        let mut string = string.into();
        if string.is_empty() {
            // Empty strings, when measured, result in bad float values
            string.push_str("~~~");
        }
        self.string = string;
        self.measure();
    }

    fn measure(&mut self) {
        let text_dimensions = measure_text(&self.string, self.font.as_ref(), self.font_size, 1.0);

        let parts = self.string.split("|");
        let mut w = 0.0;
        let mut h = 0.0;
        let mut offset_y = 0.0;
        for mut part in parts {
            if part == "<dice>" {
                w += 16.0;
            } else if part == "<shield>" {
                w += 16.0;
            } else {
                let mut font_size = self.font_size;
                if part.starts_with("<value>") {
                    part = &part["<value>".len()..];
                    font_size = (self.font_size as f32 * 1.5).floor() as u16;
                } else if part.starts_with("<keyword>") {
                    part = &part["<keyword>".len()..];
                }

                if part.len() > 0 {
                    let dim = measure_text(part, self.font.as_ref(), self.font_size, 1.0);
                    offset_y = dim.offset_y;

                    w += dim.width;
                    if dim.height > h {
                        h = dim.height;
                    }
                    h = h.max(dim.height);
                }
            }
        }

        self.size = (
            w.max(0.0) + self.hor_padding * 2.0,
            h.max(0.0) + self.vert_padding * 2.0,
        );
        self.size.1 = self.size.1.max(self.min_height);
        assert!(self.size.0.is_finite() && self.size.1.is_finite());
        self.offset_y = offset_y;
    }
}

pub fn draw_text_with_font_icons(line: &str, mut x: f32, y: f32, params: TextParams<'_>) {
    let parts = line.split("|");
    for mut part in parts {
        if part == "<dice>" {
            draw_texture(DICE_SYMBOL.get().unwrap(), x, y - 13.0, WHITE);
            x += 16.0;
        } else if part == "<shield>" {
            draw_texture(SHIELD_SYMBOL.get().unwrap(), x, y - 13.0, WHITE);
            x += 16.0;
        } else if ["Bob", "Alice", "Clara"].contains(&part) {
            let mut params = params.clone();
            params.color = match &part {
                &"Bob" => COL_BOB,
                &"Alice" => COL_ALICE,
                &"Clara" => COL_CLARA,
                _ => unreachable!(),
            };
            let part_dimensions = draw_text_rounded(part, x, y, params);
            x += part_dimensions.width;
        } else {
            let mut params = params.clone();
            if part.starts_with("<value>") {
                part = &part["<value>".len()..];
                params.color = Color::new(0.9, 0.7, 1.0, 1.0);
                params.font_size = (params.font_size as f32 * 1.5).floor() as u16;
            } else if part.starts_with("<keyword>") {
                part = &part["<keyword>".len()..];
                params.color = ORANGE;
            }

            let part_dimensions = draw_text_rounded(part, x, y, params);
            x += part_dimensions.width;
        }
    }
}

pub fn draw_text_rounded(text: &str, x: f32, y: f32, params: TextParams) -> TextDimensions {
    draw_text_ex(text, x.floor(), y.floor(), params)
    //draw_text_ex(text, x, y, params)
}

impl Drawable for TextLine {
    fn draw(&self, x: f32, y: f32) {
        let y0 = y + self.vert_padding;

        let x0 = if self.right_align {
            let text_dimensions =
                measure_text(&self.string, self.font.as_ref(), self.font_size, 1.0);
            x + self.size.0 - text_dimensions.width - self.hor_padding
        } else {
            x + self.hor_padding
        };

        if let Some((color, offset)) = self.depth {
            let params = TextParams {
                font_size: self.font_size,
                color,
                font: self.font.as_ref(),
                ..Default::default()
            };
            draw_text_rounded(
                &self.string,
                x0 + offset,
                y0 + self.offset_y + offset,
                params,
            );
        }

        let (mouse_x, mouse_y) = mouse_position();
        let hovered =
            (x..x + self.size.0).contains(&mouse_x) && (y..y + self.size.1).contains(&mouse_y);

        let mut color = self.color;
        if let Some(hover_color) = self.hover_color {
            if hovered {
                color = hover_color;
            }
        }

        let params = TextParams {
            font_size: self.font_size,
            color,
            font: self.font.as_ref(),
            ..Default::default()
        };
        draw_text_with_font_icons(&self.string, x0, y0 + self.offset_y, params);
        //draw_text_rounded(&self.string, x0, y0 + self.offset_y, params);

        draw_debug(x, y, self.size.0, self.size.1);

        if hovered {
            self.has_been_hovered.set(Some((x, y)));
        }
    }

    fn size(&self) -> (f32, f32) {
        self.size
    }
}

#[derive(Clone, Default)]
pub struct Rectangle {
    pub size: (f32, f32),
    pub style: Style,
    pub texture: Option<Texture2D>,
    pub has_been_hovered: Cell<bool>,
}

impl Drawable for Rectangle {
    fn draw(&self, x: f32, y: f32) {
        self.style.draw(x, y, self.size);
        if let Some(texture) = &self.texture {
            draw_texture_ex(
                texture,
                x,
                y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(self.size.into()),
                    ..Default::default()
                },
            );
        }
        if Rect::new(x, y, self.size.0, self.size.1).contains(mouse_position().into()) {
            self.has_been_hovered.set(true);
        }
    }

    fn size(&self) -> (f32, f32) {
        self.size
    }
}

// TODO
impl Rectangle {
    pub fn draw(&self, x: f32, y: f32) {
        self.style.draw(x, y, self.size);
    }
}

#[derive(Default, Copy, Clone, Debug)]
pub struct Style {
    pub background_color: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: Option<f32>,
    pub padding: f32,
}

impl Style {
    pub fn draw(&self, x: f32, y: f32, size: (f32, f32)) {
        self.draw_background(x, y, size);
        self.draw_foreground(x, y, size);
    }

    pub fn draw_background(&self, x: f32, y: f32, size: (f32, f32)) {
        if let Some(color) = self.background_color {
            draw_rectangle(x, y, size.0, size.1, color);
        }
    }

    pub fn draw_foreground(&self, x: f32, y: f32, size: (f32, f32)) {
        if let Some(color) = self.border_color {
            let thickness = self.border_width.unwrap_or(1.0);
            draw_rectangle_lines(x, y, size.0, size.1, thickness, color);
        }
    }
}

pub struct Circle {
    pub r: f32,
    pub color: Color,
}

impl Circle {
    pub fn draw(&self, x: f32, y: f32) {
        draw_circle(x + self.r, y + self.r, self.r, self.color);
        draw_circle_lines(x + self.r, y + self.r, self.r, 1.0, DARKGRAY);
    }

    pub fn size(&self) -> (f32, f32) {
        (self.r * 2.0, self.r * 2.0)
    }
}

pub enum LayoutDirection {
    Horizontal,
    Vertical,
}

impl Default for LayoutDirection {
    fn default() -> Self {
        Self::Horizontal
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Align {
    Start,
    Center,
    End,
}

impl Default for Align {
    fn default() -> Self {
        Self::Start
    }
}

pub struct ContainerScroll {
    offset: Cell<f32>,
    pub scroll_speed: f32,
    is_dragging: Cell<bool>,
}

impl ContainerScroll {
    pub fn new(scroll_speed: f32) -> Self {
        Self {
            scroll_speed,
            ..Default::default()
        }
    }
}

impl Default for ContainerScroll {
    fn default() -> Self {
        Self {
            offset: Default::default(),
            scroll_speed: 15.0,
            is_dragging: Cell::new(false),
        }
    }
}

#[derive(Default)]
pub struct Container {
    pub layout_dir: LayoutDirection,
    //pub reverse_vertical: bool,
    pub align: Align,
    pub margin: f32,
    pub style: Style,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub children: Vec<Element>,
    pub border_between_children: Option<Color>,
    pub scroll: Option<ContainerScroll>,
}

impl Container {
    pub fn size(&self) -> (f32, f32) {
        let (mut w, mut h) = self.content_size();

        if let Some(min_w) = self.min_width {
            w = w.max(min_w);
        }
        if let Some(min_h) = self.min_height {
            h = h.max(min_h);
        }
        if let Some(max_h) = self.max_height {
            h = h.min(max_h);
        }

        (w, h)
    }

    fn content_size_larger_than_container(&self) -> bool {
        let (mut w, mut h) = self.content_size();

        if let Some(max_h) = self.max_height {
            if h > max_h {
                return true;
            }
        }

        false
    }

    pub fn push_child(&mut self, element: Element) {
        let mut was_scrolled_to_end = false;
        if let Some(scroll) = &self.scroll {
            let content_size = self.content_size();
            let size = self.size();
            if scroll.offset.get() == content_size.1 - size.1 || content_size.1 < size.1 {
                was_scrolled_to_end = true;
            }
        }

        self.children.push(element);

        if let Some(scroll) = &self.scroll {
            if was_scrolled_to_end {
                scroll.offset.set(self.content_size().1 - self.size().1);
            }
        }
    }

    pub fn content_size(&self) -> (f32, f32) {
        let mut w = 0.0;
        let mut h = 0.0;
        for element in &self.children {
            let size = element.size();

            match self.layout_dir {
                LayoutDirection::Horizontal => {
                    w += size.0;
                    if size.1 > h {
                        h = size.1;
                    }
                }
                LayoutDirection::Vertical => {
                    h += size.1;
                    if size.0 > w {
                        w = size.0;
                    }
                }
            }
        }

        w += self.style.padding * 2.0;
        h += self.style.padding * 2.0;

        if !self.children.is_empty() {
            let total_margin = (self.children.len() - 1) as f32 * self.margin;
            match self.layout_dir {
                LayoutDirection::Horizontal => w += total_margin,
                LayoutDirection::Vertical => h += total_margin,
            }
        }

        (w, h)
    }

    pub fn remove_dropped_children(&mut self) {
        self.children.retain(|child| match child {
            Element::WeakRefCell(weak) => weak.upgrade().is_some(),
            _ => true,
        })
    }

    pub fn draw(&self, x: f32, y: f32) -> (f32, f32) {
        self._draw(x, y, false)
    }

    pub fn draw_tooltips(&self, x: f32, y: f32) {
        self._draw(x, y, true);
    }

    fn _draw(&self, x: f32, y: f32, only_tooltips: bool) -> (f32, f32) {
        let size = self.size();
        if !only_tooltips {
            self.style.draw(x, y, size);
        }

        let mut x0 = x + self.style.padding;
        let mut y0 = y + self.style.padding;
        /*
        if self.reverse_vertical {
            y0 = y + size.1 - self.style.padding;
        }
         */

        let scroll_offset = self
            .scroll
            .as_ref()
            .map(|scroll| -scroll.offset.get())
            .unwrap_or(0.0);

        for (i, element) in self.children.iter().enumerate() {
            let (element_w, element_h) = element.size();
            let offset = match (&self.align, &self.layout_dir) {
                (Align::Start, LayoutDirection::Horizontal) => (0.0, 0.0),
                (Align::Start, LayoutDirection::Vertical) => (0.0, scroll_offset),
                (Align::Center, LayoutDirection::Horizontal) => {
                    // Place it in the middle, i.e. empty space above and below
                    (0.0, (size.1 - 2.0 * self.style.padding - element_h) / 2.0)
                }
                (Align::Center, LayoutDirection::Vertical) => {
                    // Place it in the middle, i.e. empty space to the left and right
                    (
                        (size.0 - 2.0 * self.style.padding - element_w) / 2.0,
                        scroll_offset,
                    )
                }
                (Align::End, LayoutDirection::Horizontal) => {
                    // Push it down so that it touches the bottom
                    (0.0, size.1 - 2.0 * self.style.padding - element_h)
                }
                (Align::End, LayoutDirection::Vertical) => {
                    // Push it to the right, so that it touches the right side
                    (size.0 - 2.0 * self.style.padding - element_w, scroll_offset)
                }
            };

            let mut y_element = y0 + offset.1;

            /*
            if self.reverse_vertical {
                y_element -= element_h;
            }
             */

            if (y_element >= y) && (y_element + element.size().1 <= y + size.1 + 5.0) {
                if only_tooltips {
                    element.draw_tooltips(x0 + offset.0, y_element);
                } else {
                    element.draw(x0 + offset.0, y_element);
                }
            }

            match self.layout_dir {
                LayoutDirection::Horizontal => x0 += element_w + self.margin,
                LayoutDirection::Vertical => {
                    /*
                    if self.reverse_vertical {
                        y0 -= element_h + self.margin;
                    } else {
                        y0 += element_h + self.margin
                    }
                     */

                    y0 += element_h + self.margin
                }
            }

            if !(only_tooltips) && i < self.children.len() - 1 {
                if let Some(border_color) = self.border_between_children {
                    let thickness = 1.0;
                    match self.layout_dir {
                        LayoutDirection::Horizontal => draw_line(
                            x0 - self.margin * 0.5,
                            y0,
                            x0 - self.margin * 0.5,
                            y0 + size.1 - self.style.padding * 2.0,
                            thickness,
                            border_color,
                        ),
                        // TODO reverse?
                        LayoutDirection::Vertical => draw_line(
                            x0,
                            y0 - self.margin * 0.5,
                            x0 + size.0 - self.style.padding * 2.0,
                            y0 - self.margin * 0.5,
                            thickness,
                            border_color,
                        ),
                    }
                }
            }
        }

        draw_debug(x, y, size.0, size.1);

        if !only_tooltips {
            if let Some(scroll) = &self.scroll {
                // Only draw scrollbar if the content actually fills the container
                if self.content_size_larger_than_container() {
                    let content_size = self.content_size();

                    let mut bar_y = y + scroll.offset.get() / content_size.1 * size.1;

                    let bar_h = (size.1.powf(2.0) / content_size.1).min(size.1);

                    /*
                    if self.reverse_vertical {
                        // bar_y += content_size.1 - bar_h;
                        bar_y = y + size.1 - bar_h + scroll.offset.get() / content_size.1 * size.1;
                    }
                     */

                    let bar_w = 7.0;
                    draw_rectangle(x + size.0, y, bar_w, size.1, DARKGRAY);
                    draw_rectangle(x + size.0 + 1.0, bar_y, bar_w - 2.0, bar_h, GRAY);

                    let (mouse_x, mouse_y) = mouse_position();
                    let mut new_scroll_offset = None;
                    // Scrolling with mousewheel
                    if (x..x + size.0).contains(&mouse_x) && (y..y + size.1).contains(&mouse_y) {
                        let (_dx, dy) = mouse_wheel();
                        if dy != 0.0 && content_size.1 > size.1 {
                            new_scroll_offset =
                                Some(scroll.offset.get() - dy.signum() * scroll.scroll_speed)
                        }
                    }

                    if !is_mouse_button_down(MouseButton::Left) {
                        scroll.is_dragging.set(false);
                    }

                    // Scrolling by dragging with mouse
                    if (x + size.0..x + size.0 + bar_w).contains(&mouse_x)
                        && (y..y + size.1).contains(&mouse_y)
                    {
                        if is_mouse_button_pressed(MouseButton::Left) {
                            scroll.is_dragging.set(true);
                        }
                    }

                    if scroll.is_dragging.get() {
                        let offset = (mouse_y - bar_h / 2.0 - y) / size.1 * content_size.1;
                        new_scroll_offset = Some(offset);
                    }

                    if let Some(mut new_offset) = new_scroll_offset {
                        /*
                        if self.reverse_vertical {
                            new_offset = new_offset.min(0.0).max(size.1 - content_size.1)
                        } else {
                          */
                        new_offset = new_offset.min(content_size.1 - size.1).max(0.0);
                        //};
                        scroll.offset.set(new_offset);
                    }
                }
            }
        }

        size
    }
}

impl Drawable for Container {
    fn draw(&self, x: f32, y: f32) {
        self.draw(x, y);
    }

    fn size(&self) -> (f32, f32) {
        self.size()
    }

    fn draw_tooltips(&self, x: f32, y: f32) {
        self.draw_tooltips(x, y);
    }
}

#[derive(Copy, Clone, Debug)]
pub struct TableStyle {
    pub outer_border_color: Option<Color>,
    pub inner_border_color: Option<Color>,
    pub background_color: Option<Color>,
    pub all_columns_same_width: bool,
    pub row_font_sizes: &'static [u16],
    pub cell_padding: (f32, f32),
    pub default_text_color: Color,
    pub min_width: Option<f32>,
}

impl Default for TableStyle {
    fn default() -> Self {
        Self {
            outer_border_color: Some(GRAY),
            inner_border_color: Some(GRAY),
            background_color: None,
            all_columns_same_width: false,
            row_font_sizes: &[],
            cell_padding: (8.0, 8.0),
            default_text_color: WHITE,
            min_width: None,
        }
    }
}

pub struct TableCell {
    text: String,
    color_override: Option<Color>,
    depth: Option<Color>,
}

impl TableCell {
    pub fn new(text: String, color: Option<Color>, depth: Option<Color>) -> Self {
        Self {
            text,
            color_override: color,
            depth,
        }
    }
}

impl<T> From<T> for TableCell
where
    T: Into<String>,
{
    fn from(text: T) -> Self {
        TableCell {
            text: text.into(),
            color_override: None,
            depth: None,
        }
    }
}

pub fn table(
    cells: Vec<TableCell>,
    column_alignments: Vec<Align>,
    font: Font,
    style: TableStyle,
) -> Container {
    let num_cols = column_alignments.len();
    let mut columns: Vec<Vec<TextLine>> = vec![];
    for _ in 0..num_cols {
        columns.push(vec![]);
    }

    let mut font_size = 18;

    let mut col_i = 0;
    let mut row_i = 0;
    let mut max_height_in_row: f32 = 0.0;
    for cell in cells {
        if let Some(row_font_size) = style.row_font_sizes.get(row_i) {
            font_size = *row_font_size;
        }

        let mut text = TextLine::new(
            cell.text,
            font_size,
            cell.color_override.unwrap_or(style.default_text_color),
            Some(font.clone()),
        );
        if let Some(depth) = cell.depth {
            text = text.with_depth(depth, 1.0);
        }
        text.set_padding(style.cell_padding.0, style.cell_padding.1);
        max_height_in_row = max_height_in_row.max(text.size().1);
        columns[col_i].push(text);
        col_i = (col_i + 1) % num_cols;
        if col_i == 0 {
            for col in columns.iter_mut() {
                // align cells vertically
                col[row_i].set_min_height(max_height_in_row);
            }
            row_i += 1;
            max_height_in_row = 0.0;
        }
    }

    let mut max_col_w: f32 = 0.0;

    let columns: Vec<Container> = columns
        .into_iter()
        .enumerate()
        .map(|(i, col)| Container {
            layout_dir: LayoutDirection::Vertical,
            border_between_children: style.inner_border_color,
            align: column_alignments[i],
            children: col.into_iter().map(Element::Text).collect(),
            ..Default::default()
        })
        .inspect(|col| max_col_w = max_col_w.max(col.size().0))
        .collect();

    let columns = columns
        .into_iter()
        .map(|mut col| {
            if style.all_columns_same_width {
                col.min_width = Some(max_col_w);
            }
            Element::Container(col)
        })
        .collect();

    Container {
        layout_dir: LayoutDirection::Horizontal,
        style: Style {
            border_color: style.outer_border_color,
            background_color: style.background_color,
            ..Default::default()
        },
        border_between_children: style.inner_border_color,
        children: columns,
        min_width: style.min_width,
        ..Default::default()
    }
}

pub fn draw_debug(x: f32, y: f32, w: f32, h: f32) {
    if false {
        draw_rectangle_lines(x, y, w, h, 1.0, MAGENTA);
    }
}
