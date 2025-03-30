use macroquad::{
    color::{Color, DARKGREEN, GOLD, GRAY, MAGENTA, WHITE},
    input::{is_mouse_button_pressed, mouse_position, mouse_wheel, MouseButton},
    shapes::{draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_lines},
    text::{draw_text, draw_text_ex, measure_text, Font, TextParams},
};
use std::{
    cell::{Cell, RefCell},
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
    min_height: f32,
    padding: f32,
    font: Option<Font>,

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
            min_height: 0.0,
            padding: 0.0,
            font,
            has_been_hovered: Cell::new(None),
        };
        this.measure();
        this
    }

    pub fn set_min_height(&mut self, min_height: f32) {
        self.min_height = min_height;
        self.size.1 = self.size.1.max(min_height);
    }

    pub fn set_padding(&mut self, padding: f32) {
        self.padding = padding;
        self.measure();
    }

    pub fn set_string(&mut self, string: impl Into<String>) {
        let mut string = string.into();
        if string.is_empty() {
            string.push_str("~~~");
        }
        self.string = string;
        self.measure();
    }

    fn measure(&mut self) {
        let text_dimensions = measure_text(&self.string, self.font.as_ref(), self.font_size, 1.0);
        self.size = (
            text_dimensions.width.max(0.0) + self.padding * 2.0,
            text_dimensions.height.max(0.0) + self.padding * 2.0,
        );
        self.size.1 = self.size.1.max(self.min_height);
        assert!(self.size.0.is_finite() && self.size.1.is_finite());
        self.offset_y = text_dimensions.offset_y;
    }
}

impl Drawable for TextLine {
    fn draw(&self, x: f32, y: f32) {
        let params = TextParams {
            font_size: self.font_size,
            color: self.color,
            font: self.font.as_ref(),
            ..Default::default()
        };
        draw_text_ex(
            &self.string,
            self.padding + x,
            self.padding + y + self.offset_y,
            params,
        );
        draw_debug(x, y, self.size.0, self.size.1);

        let (mouse_x, mouse_y) = mouse_position();
        if (x..x + self.size.0).contains(&mouse_x) && (y..y + self.size.1).contains(&mouse_y) {
            self.has_been_hovered.set(Some((x, y)));
        }
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
    pub border_width: Option<f32>,
    pub padding: f32,
}

impl Style {
    pub fn draw(&self, x: f32, y: f32, size: (f32, f32)) {
        if let Some(color) = self.background_color {
            draw_rectangle(x, y, size.0, size.1, color);
        }
        if let Some(color) = self.border_color {
            let thickness = self.border_width.unwrap_or(1.0);
            draw_rectangle_lines(x, y, size.0, size.1, thickness, color);
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

#[derive(Default)]
pub struct ContainerScroll {
    offset: Cell<f32>,
}

#[derive(Default)]
pub struct Container {
    pub layout_dir: LayoutDirection,
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

    pub fn draw(&self, x: f32, y: f32) {
        let size = self.size();
        self.style.draw(x, y, size);

        let mut x0 = x + self.style.padding;
        let mut y0 = y + self.style.padding;
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

            let y_element = y0 + offset.1;
            if y_element >= y && y_element + element.size().1 <= y + size.1 + 5.0 {
                element.draw(x0 + offset.0, y_element);
            }

            match self.layout_dir {
                LayoutDirection::Horizontal => x0 += element_w + self.margin,
                LayoutDirection::Vertical => y0 += element_h + self.margin,
            }

            if i < self.children.len() - 1 {
                if let Some(border_color) = self.border_between_children {
                    let thickness = 1.0;
                    match self.layout_dir {
                        LayoutDirection::Horizontal => {
                            draw_line(x0, y0, x0, y0 + size.1, thickness, border_color)
                        }
                        LayoutDirection::Vertical => {
                            draw_line(x0, y0, x0 + size.0, y0, thickness, border_color)
                        }
                    }
                }
            }
        }

        draw_debug(x, y, size.0, size.1);

        if let Some(scroll) = &self.scroll {
            let content_size = self.content_size();
            let bar_y = y + scroll.offset.get() / content_size.1 * size.1;
            let bar_h = (size.1.powf(2.0) / content_size.1).min(size.1);
            let bar_w = 7.0;
            draw_rectangle(x + size.0, bar_y, bar_w, bar_h, GRAY);

            let (mouse_x, mouse_y) = mouse_position();
            if (x..x + size.0).contains(&mouse_x) && (y..y + size.1).contains(&mouse_y) {
                let (_dx, dy) = mouse_wheel();
                if dy != 0.0 {
                    const SCROLL_SPEED: f32 = 15.0;
                    let new_offset = (scroll.offset.get() - dy.signum() * SCROLL_SPEED)
                        .max(0.0)
                        .min(content_size.1 - size.1);
                    scroll.offset.set(new_offset);
                }
            }
        }
    }
}

pub fn table(cells: Vec<impl Into<String>>, column_alignments: Vec<Align>, font: Font) -> Element {
    let num_cols = column_alignments.len();
    let mut columns: Vec<Vec<TextLine>> = vec![];
    for _ in 0..num_cols {
        columns.push(vec![]);
    }

    let mut col_i = 0;
    let mut row_i = 0;
    let mut max_height_in_row: f32 = 0.0;
    for cell in cells {
        let mut text = TextLine::new(cell, 18, WHITE, Some(font.clone()));
        text.set_padding(8.0);
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

    let border_color = GRAY;

    let columns = columns
        .into_iter()
        .enumerate()
        .map(|(i, col)| {
            Element::Container(Container {
                layout_dir: LayoutDirection::Vertical,
                border_between_children: Some(border_color),
                align: column_alignments[i],
                children: col.into_iter().map(Element::Text).collect(),
                ..Default::default()
            })
        })
        .collect();

    Element::Container(Container {
        layout_dir: LayoutDirection::Horizontal,
        style: Style {
            border_color: Some(border_color),
            ..Default::default()
        },
        border_between_children: Some(border_color),
        children: columns,
        ..Default::default()
    })
}

pub fn draw_debug(x: f32, y: f32, w: f32, h: f32) {
    if false {
        draw_rectangle_lines(x, y, w, h, 1.0, MAGENTA);
    }
}
