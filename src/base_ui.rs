use macroquad::{
    color::{Color, DARKGREEN, MAGENTA, WHITE},
    input::{is_mouse_button_pressed, mouse_position, MouseButton},
    shapes::{draw_circle, draw_circle_lines, draw_rectangle, draw_rectangle_lines},
    text::{draw_text, measure_text},
};
use std::{
    cell::{self, Cell, Ref, RefCell, RefMut},
    collections::HashMap,
    rc::Rc,
    rc::Weak,
};

pub trait Drawable {
    fn draw(&self, x: f32, y: f32);
    fn size(&self) -> (f32, f32);
}

pub enum Element {
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
            Element::Container(container) => container.draw(x, y),
            Element::Text(text) => text.draw(x, y),
            Element::Circle(circle) => circle.draw(x, y),
            Element::Rect(rect) => rect.draw(x, y),
            Element::TabLink(link) => link.draw(x, y),
            Element::Box(drawable) => drawable.draw(x, y),
            Element::RcRefCell(drawable) => drawable.borrow_mut().draw(x, y),
            Element::Rc(drawable) => drawable.draw(x, y),
            Element::WeakRefCell(drawable) => drawable.upgrade().unwrap().borrow_mut().draw(x, y),
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
    pub fn new(active_i: usize, links_and_tabs: Vec<(&'static str, Element)>) -> Self {
        let mut links: Vec<TabLink> = links_and_tabs.iter().map(|t| TabLink::new(t.0)).collect();

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
    pub fn new(text: impl Into<String>) -> Self {
        let text = TextLine::new(text, 20, WHITE);
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
}

impl TextLine {
    pub fn new(string: impl Into<String>, font_size: u16, color: Color) -> Self {
        let mut this = Self {
            size: (0.0, 0.0),
            string: "".to_string(),
            offset_y: 0.0,
            font_size,
            color,
        };
        this.set_string(string);
        this
    }

    pub fn set_string(&mut self, string: impl Into<String>) {
        let mut string = string.into();
        if string.is_empty() {
            string.push_str("~~~");
        }
        let text_dimensions = measure_text(&string, None, self.font_size, 1.0);
        self.string = string;
        self.size = (
            text_dimensions.width.max(0.0),
            text_dimensions.height.max(0.0),
        );
        assert!(self.size.0.is_finite() && self.size.1.is_finite());
        self.offset_y = text_dimensions.offset_y;
    }
}

impl Drawable for TextLine {
    fn draw(&self, x: f32, y: f32) {
        draw_text(
            &self.string,
            x,
            y + self.offset_y,
            self.font_size as f32,
            self.color,
        );
        draw_debug(x, y, self.size.0, self.size.1);
    }

    fn size(&self) -> (f32, f32) {
        self.size
    }
}

pub struct Rectangle {
    pub size: (f32, f32),
    pub style: Style,
}

impl Rectangle {
    pub fn draw(&self, x: f32, y: f32) {
        self.style.draw(x, y, self.size);
    }
}

#[derive(Default, Copy, Clone)]
pub struct Style {
    pub background_color: Option<Color>,
    pub border_color: Option<Color>,
}

impl Style {
    pub fn draw(&self, x: f32, y: f32, size: (f32, f32)) {
        if let Some(color) = self.background_color {
            draw_rectangle(x, y, size.0, size.1, color);
        }
        if let Some(color) = self.border_color {
            draw_rectangle_lines(x, y, size.0, size.1, 1.0, color);
        }
    }
}

pub struct Circle {
    r: f32,
    color: Color,
}

impl Circle {
    pub fn draw(&self, x: f32, y: f32) {
        draw_circle(x + self.r, y + self.r, self.r, self.color);
        draw_circle_lines(x + self.r, y + self.r, self.r, 1.0, WHITE);
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

#[derive(Default)]
pub struct Container {
    pub layout_dir: LayoutDirection,
    pub align: Align,
    pub padding: f32,
    pub margin: f32,
    pub style: Style,
    pub min_width: Option<f32>,
    pub children: Vec<Element>,
}

impl Container {
    pub fn size(&self) -> (f32, f32) {
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

        w += self.padding * 2.0;
        h += self.padding * 2.0;

        if !self.children.is_empty() {
            let total_margin = (self.children.len() - 1) as f32 * self.margin;
            match self.layout_dir {
                LayoutDirection::Horizontal => w += total_margin,
                LayoutDirection::Vertical => h += total_margin,
            }
        }

        if let Some(min_w) = self.min_width {
            w = w.max(min_w);
        }

        (w, h)
    }

    pub fn remove_dropped_children(&mut self) {
        self.children.retain(|child| match child {
            Element::WeakRefCell(weak) => weak.upgrade().is_some(),
            _ => true,
        })
    }

    pub fn draw(&self, x: f32, y: f32) {
        let size = self.size();
        self.style.draw(x, y, size);

        let mut x0 = x + self.padding;
        let mut y0 = y + self.padding;
        for element in &self.children {
            let (element_w, element_h) = element.size();

            let offset = match (&self.align, &self.layout_dir) {
                (Align::Start, _) => (0.0, 0.0),
                (Align::Center, LayoutDirection::Horizontal) => {
                    // Place it in the middle, i.e. empty space above and below
                    (0.0, (size.1 - 2.0 * self.padding - element_h) / 2.0)
                }
                (Align::Center, LayoutDirection::Vertical) => {
                    // Place it in the middle, i.e. empty space to the left and right
                    ((size.0 - 2.0 * self.padding - element_w) / 2.0, 0.0)
                }
                (Align::End, LayoutDirection::Horizontal) => {
                    // Push it down so that it touches the bottom
                    (0.0, size.1 - 2.0 * self.padding - element_h)
                }
                (Align::End, LayoutDirection::Vertical) => {
                    // Push it to the right, so that it touches the right side
                    (size.0 - 2.0 * self.padding - element_w, 0.0)
                }
            };

            element.draw(x0 + offset.0, y0 + offset.1);

            match self.layout_dir {
                LayoutDirection::Horizontal => x0 += element_w + self.margin,
                LayoutDirection::Vertical => y0 += element_h + self.margin,
            }
        }

        draw_debug(x, y, size.0, size.1);
    }
}

pub fn draw_debug(x: f32, y: f32, w: f32, h: f32) {
    if false {
        draw_rectangle_lines(x, y, w, h, 1.0, MAGENTA);
    }
}
