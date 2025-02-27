use std::{cell::RefCell, rc::Rc};

use macroquad::{
    color::{
        self, Color, BLACK, BLUE, DARKGRAY, DARKGREEN, GOLD, GRAY, GREEN, LIGHTGRAY, MAGENTA,
        ORANGE, PURPLE, RED, WHITE, YELLOW,
    },
    input::{is_key_pressed, is_mouse_button_pressed, mouse_position, MouseButton},
    shapes::{
        draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_ex,
        draw_rectangle_lines, DrawRectangleParams,
    },
    text::{draw_text, measure_text, Font, TextDimensions},
    window::{clear_background, next_frame, screen_height, screen_width, Conf},
};
use rpg::core::{Action, BaseAction, Character, CoreGame, WaitingFor};

#[macroquad::main(window_conf)]
async fn main() {
    let waiting_for = CoreGame::new();

    let character_ref = waiting_for.game.player_character();

    let mut combat_buttons = vec![];
    let mut skill_buttons = vec![];
    let mut spell_buttons = vec![];

    let event_queue = Rc::new(RefCell::new(vec![]));

    for mut action in character_ref.known_actions.iter().copied() {
        if let BaseAction::Attack { hand, .. } = action {
            if character_ref.weapon(hand).is_none() {
                continue;
            }
            match character_ref.weapon(hand) {
                Some(weapon) => {
                    action = BaseAction::Attack {
                        hand,
                        action_point_cost: weapon.action_point_cost,
                    };
                }
                None => continue,
            }
        }

        let mut btn = new_action_button(&character_ref, action);
        btn.event_sender = Some(EventSender {
            queue: Rc::clone(&event_queue),
        });

        match action {
            BaseAction::Attack { .. } => combat_buttons.push(btn),
            BaseAction::SelfEffect(..) => skill_buttons.push(btn),
            BaseAction::CastSpell(..) => spell_buttons.push(btn),
        }
    }

    let combat_row = buttons_row(combat_buttons);
    let skill_row = buttons_row(skill_buttons);
    let spell_row = buttons_row(spell_buttons);

    let bash_btn = action_button("Bash", "", 2, 0, 0);
    let parry_btn = action_button("Parry", "", 1, 0, 0);
    let rage_btn = action_button("Rage", "", 1, 0, 0);
    let sidestep_btn = action_button("Side step", "", 1, 0, 1);

    let reactions_row = buttons_row(vec![bash_btn, parry_btn, rage_btn, sidestep_btn]);

    let stats_section = Element::Container(Container {
        layout_dir: LayoutDirection::Vertical,
        elements: vec![
            Element::Container(attribute_row(
                ("STR", 5),
                vec![("Health", 10.0), ("Physical resist", 15.0)],
            )),
            Element::Container(attribute_row(
                ("DEX", 4),
                vec![("Defense", 14.0), ("Movement", 2.3)],
            )),
            Element::Container(attribute_row(
                ("INT", 6),
                vec![("Mana", 8.0), ("Mental resist", 16.0)],
            )),
        ],
        ..Default::default()
    });

    let actions_section = Element::Container(Container {
        layout_dir: LayoutDirection::Vertical,
        margin: 10.0,
        elements: vec![combat_row, skill_row, spell_row],
        ..Default::default()
    });

    let reactions_section = Element::Container(Container {
        layout_dir: LayoutDirection::Vertical,
        margin: 10.0,
        elements: vec![reactions_row],
        ..Default::default()
    });

    let mut tabs = Tabs::new(
        0,
        vec![
            ("Actions", actions_section),
            ("Reactions", reactions_section),
            ("Stats", stats_section),
        ],
    );

    let health_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
        character_ref.health.current,
        character_ref.health.max,
        "HP",
        RED,
    )));
    let cloned_health_bar = Rc::clone(&health_bar);

    let mana_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
        character_ref.mana.current,
        character_ref.mana.max,
        "MANA",
        BLUE,
    )));
    let cloned_mana_bar = Rc::clone(&mana_bar);

    let mut resource_bars = Container {
        layout_dir: LayoutDirection::Horizontal,
        margin: 15.0,
        align: Align::End,
        elements: vec![
            Element::Rc(cloned_health_bar),
            Element::Rc(cloned_mana_bar),
            Element::Box(Box::new(LabelledResourceBar::new(
                character_ref.stamina.current,
                character_ref.stamina.max,
                "STA",
                GREEN,
            ))),
        ],
        ..Default::default()
    };

    let mut action_points_label = TextLine::new("Action points", 18);
    let mut action_points_row = ActionPointsRow::new();

    drop(character_ref);
    let mut waiting_for = WaitingFor::Action(waiting_for);

    print_instruction(&waiting_for);

    loop {
        clear_background(BLACK);

        tabs.draw(20.0, 100.0);

        resource_bars.draw(500.0, 150.0);

        action_points_label.draw(20.0, 10.0);
        action_points_row.draw(20.0, 30.0);

        if is_key_pressed(macroquad::input::KeyCode::Up) {
            health_bar.borrow_mut().set_current(8);
            action_points_row.reserved = 2;
        }
        if is_key_pressed(macroquad::input::KeyCode::Down) {
            health_bar.borrow_mut().set_current(4);
            action_points_row.reserved = 0;
        }

        for event in event_queue.borrow_mut().drain(..) {
            match event {
                Event::Hover(hovered, base_action) => {
                    if hovered {
                        let ap = match base_action {
                            BaseAction::Attack {
                                hand,
                                action_point_cost,
                            } => action_point_cost,
                            BaseAction::SelfEffect(self_effect_action) => {
                                self_effect_action.action_point_cost
                            }
                            BaseAction::CastSpell(spell) => spell.action_point_cost,
                        };
                        action_points_row.reserved = ap;
                    } else {
                        action_points_row.reserved = 0;
                    }
                }
                Event::Click(base_action) => {
                    println!("CLICKED: {:?}", base_action);
                    match waiting_for {
                        WaitingFor::Action(action_chooser) => {
                            let character_ref = action_chooser.game.player_character();
                            if character_ref.can_use_action(base_action) {
                                let action = match base_action {
                                    BaseAction::Attack {
                                        hand,
                                        action_point_cost: _,
                                    } => Action::Attack {
                                        hand,
                                        // TODO allow player to choose enhancements
                                        enhancements: Default::default(),
                                    },
                                    BaseAction::SelfEffect(self_effect_action) => {
                                        Action::SelfEffect(self_effect_action)
                                    }
                                    // TODO allow player to choose spell enhancement
                                    BaseAction::CastSpell(spell) => Action::CastSpell {
                                        spell,
                                        enhanced: false,
                                    },
                                };

                                drop(character_ref);
                                waiting_for = action_chooser.commit(action);
                                let character = waiting_for.game().player_character();
                                mana_bar.borrow_mut().set_current(character.mana.current);
                                action_points_row.current = character.action_points;
                            } else {
                                println!("CANNOT USE THAT ACTION");
                                drop(character_ref);
                                waiting_for = WaitingFor::Action(action_chooser)
                            }
                        }
                        WaitingFor::OnAttackedReaction(attacked_reaction_chooser) => {
                            todo!()
                        }
                        WaitingFor::OnAttackedHitReaction(attacked_hit_reaction_chooser) => {
                            todo!()
                        }
                    }

                    print_instruction(&waiting_for);
                }
            }
        }

        next_frame().await
    }
}

fn print_instruction(waiting_for: &WaitingFor) {
    match &waiting_for {
        WaitingFor::Action(waiting_for_action) => println!("CHOOSE ACTION"),
        WaitingFor::OnAttackedReaction(waiting_for_on_attacked_reaction) => {
            println!("REACT TO ATTACK")
        }
        WaitingFor::OnAttackedHitReaction(waiting_for_on_attacked_hit_reaction) => {
            println!("REACT TO BEING HIT")
        }
    }
}

struct EventSender {
    queue: Rc<RefCell<Vec<Event>>>,
}

impl EventSender {
    fn send(&self, value: Event) {
        self.queue.borrow_mut().push(value);
    }
}

struct ActionPointsRow {
    current: u32,
    reserved: u32,
    max: u32,
    cell_size: (f32, f32),
    padding: f32,
}

impl ActionPointsRow {
    fn new() -> Self {
        Self {
            current: 5,
            reserved: 0,
            max: 6,
            cell_size: (20.0, 20.0),
            padding: 3.0,
        }
    }

    fn draw(&mut self, x: f32, y: f32) {
        //assert!(self.reserved <= self.current);
        assert!(self.current <= self.max);

        let mut x0 = x + self.padding;
        let y0 = y + self.padding;
        let r = self.cell_size.1 * 0.3;
        for i in 0..self.max {
            if i < self.current.saturating_sub(self.reserved) {
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    GOLD,
                );
            } else if i < self.current {
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    WHITE,
                );
            } else if i < self.reserved {
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    RED,
                );
            } else {
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    GRAY,
                );
            }
            draw_circle_lines(
                x0 + self.cell_size.0 / 2.0,
                y0 + self.cell_size.1 / 2.0,
                r,
                1.0,
                DARKGRAY,
            );
            x0 += self.cell_size.0;
        }

        draw_rectangle_lines(
            x,
            y,
            self.max as f32 * self.cell_size.0 + self.padding * 2.0,
            self.cell_size.1 + self.padding * 2.0,
            1.0,
            WHITE,
        );
    }
}

trait Drawable {
    fn draw(&mut self, x: f32, y: f32);
    fn size(&self) -> (f32, f32);
}

struct ResourceBar {
    current: u32,
    max: u32,
    color: Color,
    cell_size: (f32, f32),
}

impl Drawable for ResourceBar {
    fn draw(&mut self, x: f32, y: f32) {
        let cell_size = self.cell_size;
        let mut y0 = y;
        for i in 0..self.max {
            if i >= self.max - self.current {
                draw_rectangle(x, y0, cell_size.0, cell_size.1, self.color);
            }
            if i > 0 {
                let space = 4.0;
                draw_line(x + space, y0, x + cell_size.0 - space, y0, 1.0, DARKGRAY);
            }

            y0 += cell_size.1;
        }

        draw_rectangle_lines(x, y, cell_size.0, self.max as f32 * cell_size.1, 1.0, WHITE);
    }

    fn size(&self) -> (f32, f32) {
        (self.cell_size.0, self.cell_size.1 * self.max as f32)
    }
}

struct LabelledResourceBar {
    list: Container,
    bar: Rc<RefCell<ResourceBar>>,
    value_text: Rc<RefCell<TextLine>>,
    max_value: u32,
}

impl LabelledResourceBar {
    fn new(current: u32, max: u32, label: &'static str, color: Color) -> Self {
        let bar = Rc::new(RefCell::new(ResourceBar {
            current,
            max,
            color,
            cell_size: (20.0, 15.0),
        }));
        let cloned_bar = Rc::clone(&bar);

        let value_text = Rc::new(RefCell::new(TextLine::new(
            format!("{}/{}", current, max),
            18,
        )));
        let cloned_value_text = Rc::clone(&value_text);
        let label_text = TextLine::new(label, 18);

        let list = Container {
            layout_dir: LayoutDirection::Vertical,
            align: Align::Center,
            margin: 5.0,
            elements: vec![
                Element::Rc(cloned_bar),
                Element::Rc(cloned_value_text),
                Element::Text(label_text),
            ],
            ..Default::default()
        };

        Self {
            list,
            bar,
            value_text,
            max_value: max,
        }
    }

    fn set_current(&mut self, value: u32) {
        self.bar.borrow_mut().current = value;
        self.value_text
            .borrow_mut()
            .set_string(format!("{}/{}", value, self.max_value));
    }
}

impl Drawable for LabelledResourceBar {
    fn draw(&mut self, x: f32, y: f32) {
        self.list.draw(x, y)
    }

    fn size(&self) -> (f32, f32) {
        self.list.size()
    }
}

fn buttons_row(buttons: Vec<ActionButton>) -> Element {
    let elements = buttons.into_iter().map(|btn| Element::Btn(btn)).collect();
    Element::Container(Container {
        layout_dir: LayoutDirection::Horizontal,
        margin: 10.0,
        elements,
        ..Default::default()
    })
}

fn action_button(
    text: impl Into<String>,
    subtext: impl Into<String>,
    action_points: u32,
    mana_points: u32,
    stamina_points: u32,
) -> ActionButton {
    let button_size = (100.0, 50.0);
    let highlight_color = YELLOW;
    let btn_style = Style {
        background_color: Some(DARKGRAY),
        border_color: Some(LIGHTGRAY),
    };
    ActionButton::new(
        BaseAction::Attack {
            hand: rpg::core::HandType::MainHand,
            action_point_cost: 0,
        },
        button_size,
        btn_style,
        text,
        subtext,
        highlight_color,
        action_points,
        mana_points,
        stamina_points,
    )
}

fn new_action_button(character: &Character, action: BaseAction) -> ActionButton {
    let button_size = (100.0, 50.0);
    let highlight_color = YELLOW;
    let btn_style = Style {
        background_color: Some(DARKGRAY),
        border_color: Some(LIGHTGRAY),
    };

    let mut subtext = String::new();
    let text;
    let mut mana_points = 0;
    let mut stamina_points = 0;
    let action_points;

    match action {
        BaseAction::Attack {
            hand,
            action_point_cost,
        } => {
            text = "Attack";
            subtext = format!("({})", character.weapon(hand).unwrap().name);
            action_points = action_point_cost;
        }
        BaseAction::SelfEffect(self_effect_action) => {
            text = self_effect_action.name;
            action_points = self_effect_action.action_point_cost;
        }
        BaseAction::CastSpell(spell) => {
            text = spell.name;
            subtext = "(Spell)".to_string();
            action_points = spell.action_point_cost;
            mana_points = spell.mana_cost;
        }
    }

    ActionButton::new(
        action,
        button_size,
        btn_style,
        text,
        subtext,
        highlight_color,
        action_points,
        mana_points,
        stamina_points,
    )
}

fn attribute_row(attribute: (&'static str, i32), stats: Vec<(&'static str, f32)>) -> Container {
    let attribute_element = Element::Text(TextLine::new(
        format!("{}: {}", attribute.0, attribute.1),
        22,
    ));

    let stat_rows: Vec<Element> = stats
        .iter()
        .map(|(name, value)| Element::Text(TextLine::new(format!("{} = {}", name, value), 18)))
        .collect();

    let stats_list = Element::Container(Container {
        layout_dir: LayoutDirection::Vertical,
        margin: 4.0,
        elements: stat_rows,
        ..Default::default()
    });
    Container {
        layout_dir: LayoutDirection::Horizontal,
        padding: 5.0,
        margin: 20.0,
        align: Align::Center,
        style: Style {
            border_color: Some(GRAY),
            ..Default::default()
        },
        elements: vec![attribute_element, stats_list],
        ..Default::default()
    }
}

#[derive(Default, Copy, Clone)]
struct Style {
    background_color: Option<Color>,
    border_color: Option<Color>,
}

impl Style {
    fn draw(&self, x: f32, y: f32, size: (f32, f32)) {
        if let Some(color) = self.background_color {
            draw_rectangle(x, y, size.0, size.1, color);
        }
        if let Some(color) = self.border_color {
            draw_rectangle_lines(x, y, size.0, size.1, 1.0, color);
        }
    }
}

enum Element {
    Btn(ActionButton),
    Container(Container),
    Text(TextLine),
    Circle(Circle),
    Rect(Rectangle),
    TabLink(TabLink),
    Box(Box<dyn Drawable>),
    Rc(Rc<RefCell<dyn Drawable>>),
}

impl Element {
    fn size(&self) -> (f32, f32) {
        match self {
            Element::Btn(btn) => btn.size,
            Element::Container(container) => container.size(),
            Element::Text(text) => text.size,
            Element::Circle(circle) => (circle.r * 2.0, circle.r * 2.0),
            Element::Rect(rect) => rect.size,
            Element::TabLink(link) => link.size,
            Element::Box(drawable) => drawable.size(),
            Element::Rc(drawable) => drawable.borrow().size(),
        }
    }

    fn draw(&mut self, x: f32, y: f32) {
        match self {
            Element::Btn(btn) => btn.draw(x, y),
            Element::Container(container) => container.draw(x, y),
            Element::Text(text) => text.draw(x, y),
            Element::Circle(circle) => circle.draw(x, y),
            Element::Rect(rect) => rect.draw(x, y),
            Element::TabLink(link) => link.draw(x, y),
            Element::Box(drawable) => drawable.draw(x, y),
            Element::Rc(drawable) => drawable.borrow_mut().draw(x, y),
        }
    }

    fn unwrap_tab_link(&mut self) -> &mut TabLink {
        match self {
            Element::TabLink(tab_link) => tab_link,
            _ => panic!("Unexpected variant"),
        }
    }
}

struct Tabs {
    links_row: Container,
    tabs: Vec<Element>,
    active_i: usize,
}

impl Tabs {
    fn new(active_i: usize, links_and_tabs: Vec<(&'static str, Element)>) -> Self {
        let mut links: Vec<TabLink> = links_and_tabs.iter().map(|t| TabLink::new(t.0)).collect();

        links[active_i].active = true;
        let links_row = Container {
            layout_dir: LayoutDirection::Horizontal,
            elements: links.into_iter().map(Element::TabLink).collect(),
            ..Default::default()
        };

        let tabs: Vec<Element> = links_and_tabs.into_iter().map(|t| t.1).collect();
        Self {
            links_row,
            tabs,
            active_i,
        }
    }

    fn draw(&mut self, x: f32, y: f32) {
        // If a link was clicked, update the state of all links
        let mut maybe_clicked_i = None;
        for (i, element) in self.links_row.elements.iter_mut().enumerate() {
            if element.unwrap_tab_link().was_clicked {
                maybe_clicked_i = Some(i);
                self.active_i = i;
                break;
            }
        }
        if let Some(clicked_i) = maybe_clicked_i {
            for (i, element) in self.links_row.elements.iter_mut().enumerate() {
                let tab_link = element.unwrap_tab_link();
                tab_link.was_clicked = false;
                tab_link.active = i == clicked_i;
            }
        }

        self.links_row.draw(x, y);

        self.tabs[self.active_i].draw(x, y + 50.0);
    }
}

struct TabLink {
    text: TextLine,
    active: bool,
    padding: f32,
    size: (f32, f32),
    was_clicked: bool,
}

impl TabLink {
    fn new(text: impl Into<String>) -> Self {
        let text = TextLine::new(text, 20);
        let padding = 5.0;
        let text_size = text.size;
        Self {
            text,
            active: false,
            padding,
            size: (padding * 2.0 + text_size.0, padding * 2.0 + text_size.1),
            was_clicked: false,
        }
    }

    fn draw(&mut self, x: f32, y: f32) {
        if self.active {
            draw_rectangle(x, y, self.size.0, self.size.1, DARKGREEN);
        }

        if is_mouse_button_pressed(MouseButton::Left) {
            let (mouse_x, mouse_y) = mouse_position();
            if (x..=x + self.size.0).contains(&mouse_x) && (y..=y + self.size.1).contains(&mouse_y)
            {
                self.was_clicked = true;
            }
        }

        self.text.draw(x + self.padding, y + self.padding);
    }
}

enum LayoutDirection {
    Horizontal,
    Vertical,
}

impl Default for LayoutDirection {
    fn default() -> Self {
        Self::Horizontal
    }
}

enum Align {
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
struct Container {
    layout_dir: LayoutDirection,
    align: Align,
    padding: f32,
    margin: f32,
    style: Style,
    elements: Vec<Element>,
}

impl Container {
    fn size(&self) -> (f32, f32) {
        let mut w = 0.0;
        let mut h = 0.0;
        for element in &self.elements {
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

        if !self.elements.is_empty() {
            let total_margin = (self.elements.len() - 1) as f32 * self.margin;
            match self.layout_dir {
                LayoutDirection::Horizontal => w += total_margin,
                LayoutDirection::Vertical => h += total_margin,
            }
        }

        (w, h)
    }

    fn draw(&mut self, x: f32, y: f32) {
        let size = self.size();
        self.style.draw(x, y, size);

        let mut x0 = x + self.padding;
        let mut y0 = y + self.padding;
        for element in &mut self.elements {
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

struct TextLine {
    size: (f32, f32),
    string: String,
    offset_y: f32,
    font_size: u16,
}

impl TextLine {
    fn new(text: impl Into<String>, font_size: u16) -> Self {
        let string = text.into();

        let mut this = Self {
            size: (0.0, 0.0),
            string: "".to_string(),
            offset_y: 0.0,
            font_size,
        };
        this.set_string(string);
        this
    }

    fn set_string(&mut self, string: String) {
        let text_dimensions = measure_text(&string, None, self.font_size, 1.0);
        self.string = string;
        self.size = (text_dimensions.width, text_dimensions.height);
        self.offset_y = text_dimensions.offset_y;
    }
}

impl Drawable for TextLine {
    fn draw(&mut self, x: f32, y: f32) {
        draw_text(
            &self.string,
            x,
            y + self.offset_y,
            self.font_size as f32,
            WHITE,
        );
        draw_debug(x, y, self.size.0, self.size.1);
    }

    fn size(&self) -> (f32, f32) {
        self.size
    }
}

struct ActionButton {
    action: BaseAction,
    size: (f32, f32),
    style: Style,
    content: Box<Element>,
    highlight_border_color: Color,
    points_row: Container,
    point_radius: f32,
    hovered: bool,
    event_sender: Option<EventSender>,
}

impl ActionButton {
    fn new(
        action: BaseAction,
        size: (f32, f32),
        style: Style,
        text: impl Into<String>,
        subtext: impl Into<String>,
        highlight_border_color: Color,
        action_points: u32,
        mana_points: u32,
        stamina_points: u32,
    ) -> Self {
        let r = 4.0;
        let mut point_icons = vec![];
        for _ in 0..action_points {
            point_icons.push(Element::Rect(Rectangle {
                size: (r * 2.0, r * 2.0),
                style: Style {
                    background_color: Some(color::GOLD),
                    border_color: Some(BLACK),
                },
            }))
        }
        for _ in 0..mana_points {
            point_icons.push(Element::Rect(Rectangle {
                size: (r * 2.0, r * 2.0),
                style: Style {
                    background_color: Some(BLUE),
                    border_color: Some(BLACK),
                },
            }))
        }
        for _ in 0..stamina_points {
            point_icons.push(Element::Rect(Rectangle {
                size: (r * 2.0, r * 2.0),
                style: Style {
                    background_color: Some(GREEN),
                    border_color: Some(BLACK),
                },
            }))
        }
        let points_row = Container {
            elements: point_icons,
            margin: 2.0,
            layout_dir: LayoutDirection::Horizontal,
            ..Default::default()
        };

        let text = Element::Text(TextLine::new(text, 20));
        let subtext = subtext.into();
        let content = if subtext.len() > 0 {
            Box::new(Element::Container(Container {
                layout_dir: LayoutDirection::Vertical,
                margin: 8.0,
                align: Align::Center,
                elements: vec![text, Element::Text(TextLine::new(subtext, 16))],
                ..Default::default()
            }))
        } else {
            Box::new(text)
        };

        Self {
            action,
            size,
            style,
            content,
            highlight_border_color,
            points_row,
            point_radius: r,
            hovered: false,
            event_sender: None,
        }
    }

    fn draw(&mut self, x: f32, y: f32) {
        let (w, h) = self.size;
        self.style.draw(x, y, self.size);

        let (mouse_x, mouse_y) = mouse_position();

        let hovered = (x..=x + w).contains(&mouse_x) && (y..=y + h).contains(&mouse_y);
        if hovered != self.hovered {
            self.hovered = hovered;
            if let Some(event_sender) = &self.event_sender {
                event_sender.send(Event::Hover(hovered, self.action));
            }
        }

        if hovered && is_mouse_button_pressed(MouseButton::Left) {
            if let Some(event_sender) = &self.event_sender {
                event_sender.send(Event::Click(self.action));
            }
        }

        if hovered {
            draw_rectangle_lines(x, y, w, h, 1.0, self.highlight_border_color);
        }

        let margin_x = (w - self.content.size().0) / 2.0;
        let margin_y = (h - self.point_radius * 2.0 - self.content.size().1) / 2.0;
        self.content.draw(x + margin_x, y + margin_y);

        let margin = 4.0;
        let row_size = self.points_row.size();
        self.points_row
            .draw(x + w - row_size.0 - margin, y + h - margin - row_size.1);

        draw_debug(x, y, w, h);
    }
}

enum Event {
    Hover(bool, BaseAction),
    Click(BaseAction),
}

struct Circle {
    r: f32,
    color: Color,
}

impl Circle {
    fn draw(&self, x: f32, y: f32) {
        draw_circle(x + self.r, y + self.r, self.r, self.color);
        draw_circle_lines(x + self.r, y + self.r, self.r, 1.0, WHITE);
    }
}

struct Rectangle {
    size: (f32, f32),
    style: Style,
}

impl Rectangle {
    fn draw(&self, x: f32, y: f32) {
        self.style.draw(x, y, self.size);
    }
}

fn draw_debug(x: f32, y: f32, w: f32, h: f32) {
    if false {
        draw_rectangle_lines(x, y, w, h, 1.0, MAGENTA);
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "UI test".to_owned(),
        window_width: 1600,
        window_height: 1200,
        high_dpi: true,
        ..Default::default()
    }
}
