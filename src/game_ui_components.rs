use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    color::SKYBLUE,
    input::{is_key_pressed, KeyCode},
    math::Rect,
    shapes::{draw_triangle, draw_triangle_lines},
    texture::{draw_texture, draw_texture_ex, DrawTextureParams, FilterMode},
    time::{get_frame_time, get_time},
};

use indexmap::IndexMap;
use macroquad::{
    color::{Color, BLACK, DARKGRAY, GOLD, GRAY, LIGHTGRAY, RED, WHITE},
    input::{is_mouse_button_pressed, mouse_position, MouseButton},
    shapes::{draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_lines},
    text::Font,
    texture::Texture2D,
};

use crate::{
    action_button::{draw_tooltip, TooltipPositionPreference},
    base_ui::{
        Align, Container, ContainerScroll, Drawable, Element, LayoutDirection, Rectangle, Style,
        TextLine,
    },
    core::{
        Character, CharacterId, Characters, Condition, ConditionInfo, CoreGame, MAX_ACTION_POINTS,
    },
    drawing::{draw_cross, draw_rounded_rectangle_lines},
    sounds::{SoundId, SoundPlayer},
    textures::{PortraitId, StatusId, PORTRAIT_BG_TEXTURE, PORTRAIT_ENEMY_BG_TEXTURE},
};

pub struct CharacterPortraits {
    row: Container,
    active_id: CharacterId,
    hovered_id: Option<CharacterId>,
    portraits: HashMap<CharacterId, Rc<RefCell<TopCharacterPortrait>>>,
}

impl CharacterPortraits {
    pub fn new(
        characters: &Characters,
        active_id: CharacterId,
        font: Font,
        portrait_textures: HashMap<PortraitId, Texture2D>,
    ) -> Self {
        let mut portraits: HashMap<CharacterId, Rc<RefCell<TopCharacterPortrait>>> =
            Default::default();

        let mut elements = vec![];

        for (id, character) in characters.iter_with_ids() {
            let texture = portrait_textures[&character.portrait].clone();

            let portrait = Rc::new(RefCell::new(TopCharacterPortrait::new(
                character,
                font.clone(),
                texture,
            )));
            let cloned = Rc::downgrade(&portrait);
            portraits.insert(*id, portrait);
            elements.push(Element::WeakRefCell(cloned));
        }

        let row = Container {
            layout_dir: LayoutDirection::Horizontal,
            //margin: 2.0,
            children: elements,
            /*
            style: Style {
                padding: 2.0,
                background_color: Some(Color::new(0.0, 0.0, 0.0, 0.5)),
                border_color: Some(BLACK),
                //background_color: Some(Color::new(0.4, 0.3, 0.2, 1.0)),
                ..Default::default()
            },
             */
            ..Default::default()
        };

        let mut this = Self {
            row,
            active_id,
            hovered_id: None,
            portraits,
        };

        this.set_active_character(active_id);
        this
    }

    fn set_active_character(&mut self, id: CharacterId) {
        if let Some(portrait) = self.portraits.get(&self.active_id) {
            // The entry may have been removed if the active character died during its turn
            portrait.borrow_mut().strong_highlight = false;
        }
        self.active_id = id;
        self.portraits[&self.active_id]
            .borrow_mut()
            .strong_highlight = true;
    }

    pub fn update(&mut self, game: &CoreGame) {
        self.set_active_character(game.active_character_id);
        for (id, character) in game.characters.iter_with_ids() {
            /*
            let portrait = self.portraits[id].borrow_mut();
            portrait.action_points_row.borrow_mut().current_ap = character.action_points.current();
            portrait.action_points_row.borrow_mut().is_characters_turn =
                *id == game.active_character_id;
                 */
            //portrait.health_bar.borrow_mut().current = character.health.current();
        }
    }

    pub fn set_hovered_character_id(&mut self, id: Option<CharacterId>) {
        if let Some(previous_id) = self.hovered_id {
            if let Some(portrait) = self.portraits.get(&previous_id) {
                // The entry may have been removed if the character died recently
                portrait.borrow_mut().weak_highlight = false;
            }
        }
        self.hovered_id = id;
        if let Some(id) = self.hovered_id {
            self.portraits[&id].borrow_mut().weak_highlight = true;
        }
    }

    pub fn draw(&self, x: f32, y: f32) {
        self.row.draw(x, y);
    }

    pub fn remove_dead(&mut self) {
        self.portraits
            .retain(|_id, portrait| !portrait.borrow().character.is_dead());
        self.row.remove_dropped_children();
    }
}

struct TopCharacterPortrait {
    strong_highlight: bool,
    weak_highlight: bool,
    //action_points_row: Rc<RefCell<ActionPointsRow>>,
    //health_bar: Rc<RefCell<ResourceBar>>,
    padding: f32,
    container: Container,
    character: Rc<Character>,
    font: Font,
    texture_size: Rc<RefCell<(f32, f32)>>,
}

impl TopCharacterPortrait {
    fn new(character: &Rc<Character>, font: Font, texture: Texture2D) -> Self {
        /*
        let action_points_row = Rc::new(RefCell::new(ActionPointsRow::new(
            (10.0, 10.0),
            0.2,
            Style {
                background_color: Some(BLACK),
                ..Default::default()
            },
        )));
        let cloned_ap_row = Rc::clone(&action_points_row);
         */

        let name_color = if character.player_controlled() {
            WHITE
        } else {
            Color::new(1.0, 0.7, 0.7, 1.0)
        };

        let mut name_text_line = TextLine::new(character.name, 16, name_color, Some(font.clone()));
        name_text_line.set_min_height(13.0);

        /*
        let health_bar = Rc::new(RefCell::new(ResourceBar::horizontal(
            character.health.max(),
            RED,
            (action_points_row.borrow().size().0, 8.0),
        )));
         */

        //let texture_size = (48.0, 60.0);
        let texture_size = Rc::new(RefCell::new((64.0, 80.0)));
        /*
           let container = Container {
               layout_dir: LayoutDirection::Vertical,
               align: Align::Center,
               children: vec![
                   //Element::Text(name_text_line),
                   Element::Container(Container {
                       style: Style {
                           background_color: Some(DARKGRAY),
                           ..Default::default()
                       },
                       children: vec![Element::ResizableTexture(texture, texture_size.clone())],
                       ..Default::default()
                   }),
                   //Element::RcRefCell(cloned_ap_row),
                   //Element::RcRefCell(health_bar.clone()),
               ],
               margin: 1.0,
               style: Style {
                   padding: 0.0,

                   ..Default::default()
               },
               ..Default::default()
           };
        */

        //texture.set_filter(FilterMode::Linear);

        let container = Container {
            style: Style {
                //background_color: Some(DARKGRAY),
                ..Default::default()
            },
            children: vec![Element::ResizableTexture(texture, texture_size.clone())],
            ..Default::default()
        };

        Self {
            strong_highlight: false,
            weak_highlight: false,
            //action_points_row,
            //health_bar,
            padding: 0.0,
            container,
            character: character.clone(),
            font,
            texture_size,
        }
    }
}

impl Drawable for TopCharacterPortrait {
    fn draw(&self, x: f32, y: f32) {
        let (w, h) = self.size();
        let (portrait_w, portrait_h) = self.container.children[0].size();
        let bg_texture = if self.character.player_controlled() {
            &PORTRAIT_BG_TEXTURE
        } else {
            &PORTRAIT_ENEMY_BG_TEXTURE
        };
        draw_texture_ex(
            bg_texture.get().unwrap(),
            x,
            y,
            WHITE,
            DrawTextureParams {
                dest_size: Some((w, h).into()),
                ..Default::default()
            },
        );
        self.container.draw(x + self.padding, y + self.padding);
        draw_rectangle_lines(x, y, w, h, 2.0, LIGHTGRAY);

        let x0 = x + (w - portrait_w) / 2.0;

        if self.strong_highlight {
            let margin = 1.0;

            draw_rounded_rectangle_lines(
                x0 - margin,
                y - margin,
                portrait_w + 2.0 * margin,
                portrait_h + 2.0 * margin,
                5.0,
                WHITE,
                10.0,
                None,
            );
        }
        if self.weak_highlight {
            draw_rectangle_lines(
                x0 + 1.0,
                y + 1.0,
                portrait_w - 2.0,
                portrait_h - 2.0,
                2.0,
                LIGHTGRAY,
            );
        }

        if !self.character.health.is_at_max() {
            let margin = 1.0;
            let damage_w = w - margin * 2.0;
            let damage_h = (h - margin * 2.0) * (1.0 - self.character.health.ratio());
            let x0 = x + margin;
            let y0 = y + margin;
            draw_rectangle(x0, y0, damage_w, damage_h, Color::new(0.3, 0.0, 0.0, 0.5));
            draw_line(
                x0,
                y0 + damage_h,
                x0 + damage_w,
                y0 + damage_h,
                3.0,
                Color::new(0.6, 0.0, 0.0, 0.6),
            );
        }

        /*
        if self.character.has_taken_a_turn_this_round.get() {
            // TODO: draw some kind of hourglass instead?
            let text = "DONE";
            let font_size = 16;
            let text_dim = measure_text(text, Some(&self.font), font_size, 1.0);
            draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.3));
            draw_text_rounded(
                text,
                x + w / 2.0 - text_dim.width / 2.0,
                y + 30.0,
                TextParams {
                    font: Some(&self.font),
                    font_size,
                    color: WHITE,
                    ..Default::default()
                },
            );
        }
         */
    }

    fn size(&self) -> (f32, f32) {
        *self.texture_size.borrow_mut() = if self.character.is_part_of_active_group.get() {
            (64.0, 80.0)
        } else {
            (56.0, 70.0)
        };

        let (w, h) = self.container.size();
        (w + self.padding * 2.0, h + self.padding * 2.0)
    }
}

pub struct CharacterSheetToggle {
    pub shown: Cell<bool>,
    pub text_line: TextLine,
    pub padding: f32,
    pub sound_player: SoundPlayer,
}

impl CharacterSheetToggle {
    pub fn set_shown(&self, shown: bool) {
        if self.shown.get() != shown {
            self.shown.set(shown);
            if shown {
                self.sound_player.play(SoundId::SheetOpen);
            } else {
                self.sound_player.play(SoundId::SheetClose);
            }
        }
    }

    pub fn is_shown(&self) -> bool {
        self.shown.get()
    }
}

impl Drawable for CharacterSheetToggle {
    fn draw(&self, x: f32, y: f32) {
        self.text_line.draw(x + self.padding, y + self.padding);

        let size = self.size();

        let (mouse_x, mouse_y) = mouse_position();
        let hovered = (x..x + size.0).contains(&mouse_x) && (y..y + size.1).contains(&mouse_y);
        if hovered && is_mouse_button_pressed(MouseButton::Left) {
            self.set_shown(!self.shown.get());
        }

        if self.shown.get() {
            draw_rectangle_lines(x, y, size.0, size.1, 2.0, GOLD);
        } else {
            draw_rectangle_lines(x, y, size.0, size.1, 1.0, LIGHTGRAY);
        }

        if hovered {
            draw_rectangle_lines(x + 2.0, y + 2.0, size.0 - 4.0, size.1 - 4.0, 1.0, WHITE);
        }
    }

    fn size(&self) -> (f32, f32) {
        let (w, h) = self.text_line.size();
        (w + self.padding * 2.0, h + self.padding * 2.0)
    }
}

pub struct PlayerPortraits {
    row: Container,
    selected_i: Cell<CharacterId>,
    active_i: Cell<CharacterId>,
    portraits: IndexMap<CharacterId, Rc<RefCell<PlayerCharacterPortrait>>>,
    sound_player: SoundPlayer,
    font: Font,
}

pub struct PlayerPortraitOutcome {
    pub changed_character: bool,
    pub clicked_end_turn: bool,
}

impl PlayerPortraits {
    pub fn new(
        characters: &Characters,
        selected_id: CharacterId,
        active_id: CharacterId,
        font: Font,
        portrait_textures: HashMap<PortraitId, Texture2D>,
        status_textures: HashMap<StatusId, Texture2D>,
        sound_player: SoundPlayer,
    ) -> Self {
        let mut portraits: IndexMap<CharacterId, Rc<RefCell<PlayerCharacterPortrait>>> =
            Default::default();

        for (id, character) in characters.iter_with_ids() {
            if character.player_controlled() {
                let texture = portrait_textures.get(&character.portrait).unwrap().clone();

                portraits.insert(
                    *id,
                    Rc::new(RefCell::new(PlayerCharacterPortrait::new(
                        character,
                        font.clone(),
                        texture,
                        status_textures.clone(),
                    ))),
                );
            }
        }

        let mut portrait_elements = vec![];
        for portrait in portraits.values() {
            portrait_elements.push(Element::RcRefCell(portrait.clone()));
        }

        let row = Container {
            layout_dir: LayoutDirection::Horizontal,
            margin: 10.0,
            children: portrait_elements,
            ..Default::default()
        };

        let this = Self {
            row,
            selected_i: Cell::new(selected_id),
            active_i: Cell::new(active_id),
            portraits,
            sound_player,
            font,
        };

        this.set_selected_id(selected_id);
        this
    }

    pub fn set_selected_id(&self, character_id: CharacterId) {
        self.portraits[&self.selected_i.get()]
            .borrow()
            .is_character_shown
            .set(false);
        self.selected_i.set(character_id);
        self.portraits[&self.selected_i.get()]
            .borrow()
            .is_character_shown
            .set(true);
    }

    pub fn selected_id(&self) -> CharacterId {
        self.selected_i.get()
    }

    pub fn set_active_character(&self, character_id: CharacterId) {
        if let Some(portrait) = self.portraits.get(&self.active_i.get()) {
            portrait.borrow().is_character_active.set(false);
        }
        self.active_i.set(character_id);
        if let Some(portrait) = self.portraits.get(&character_id) {
            portrait.borrow().is_character_active.set(true);
        }
    }

    pub fn update(&self, game: &CoreGame) {
        self.set_active_character(game.active_character_id);
    }

    pub fn draw(&self, x: f32, y: f32, may_show_end_turn_button: bool) -> PlayerPortraitOutcome {
        for (_, portrait) in &self.portraits {
            let mut portrait = portrait.borrow_mut();
            portrait
                .may_show_end_turn_button
                .set(may_show_end_turn_button);
            let text_color = if may_show_end_turn_button {
                WHITE
            } else {
                GRAY
            };
            portrait.end_turn_text.set_color(text_color);
        }

        self.row.draw(x, y);

        for (_, portrait) in &self.portraits {
            if let Some((i, info)) = portrait.borrow().hovered_status_rect.get() {
                let rect = portrait.borrow().last_drawn_rect.get();
                let name = if let Some(stacks) = info.stacks {
                    format!("{} ({})", info.condition.name(), stacks)
                } else {
                    info.condition.name().to_string()
                };
                draw_tooltip(
                    &self.font,
                    TooltipPositionPreference::At((
                        rect.right() + STATUS_ICON_W + 5.0,
                        rect.y + 1.0,
                    )),
                    &name,
                    None,
                    &[info.populated_description()],
                    &[],
                    true,
                );
            }
        }

        let prev_selected = self.selected_i.get();

        let mut change_attempt = false;
        let mut ended_turn = false;

        for (i, portrait) in &self.portraits {
            if portrait.borrow().has_been_clicked.take() {
                self.sound_player.play(SoundId::ClickButton);
                println!("Portrait has been clicked {i}");
                self.set_selected_id(*i);
                change_attempt = true;
                break;
            }
            if portrait.borrow().has_clicked_end_turn.take() {
                ended_turn = true;
            }
        }

        if is_key_pressed(KeyCode::Tab) {
            self.toggle_active_id();
            change_attempt = true;
        }

        let changed_character = change_attempt && self.selected_i.get() != prev_selected;

        PlayerPortraitOutcome {
            changed_character,
            clicked_end_turn: ended_turn,
        }
    }

    fn toggle_active_id(&self) {
        let idx = self.portraits.get_index_of(&self.selected_i.get()).unwrap();
        let new_idx = (idx + 1) % self.portraits.len();
        let (new_character_id, _) = self.portraits.get_index(new_idx).unwrap();
        self.set_selected_id(*new_character_id);
    }

    pub fn set_statuses(&mut self, character_id: CharacterId, statuses: &[ConditionInfo]) {
        if let Some(portrait) = self.portraits.get_mut(&character_id) {
            portrait.borrow_mut().set_statuses(statuses);
        }
    }
}

struct PlayerCharacterPortrait {
    text: TextLine,
    character: Rc<Character>,
    is_character_shown: Cell<bool>,
    is_character_active: Cell<bool>,
    padding: f32,
    has_been_clicked: Cell<bool>,
    texture: Texture2D,
    status_column: Element,
    status_rects: Vec<Rc<RefCell<Rectangle>>>,
    status_textures: HashMap<StatusId, Texture2D>,
    done_text: TextLine,
    end_turn_text: TextLine,
    pub has_clicked_end_turn: Cell<bool>,
    may_show_end_turn_button: Cell<bool>,
    hovered_status_rect: Cell<Option<(usize, ConditionInfo)>>,
    font: Font,
    last_drawn_rect: Cell<Rect>,
}

impl PlayerCharacterPortrait {
    fn new(
        character: &Rc<Character>,
        font: Font,
        texture: Texture2D,
        status_textures: HashMap<StatusId, Texture2D>,
    ) -> Self {
        let mut text = TextLine::new(character.name, 20, WHITE, Some(font.clone()));
        text.set_depth(BLACK, 2.0);

        let mut status_rects = vec![];
        let mut status_elements = vec![];
        for _ in 0..6 {
            let rect = Rc::new(RefCell::new(Rectangle {
                size: (STATUS_ICON_W, STATUS_ICON_W),
                style: Style {
                    background_color: Some(BLACK),
                    ..Default::default()
                },
                ..Default::default()
            }));
            status_rects.push(rect.clone());
            status_elements.push(Element::RcRefCell(rect));
        }

        let status_column = Element::Container(Container {
            layout_dir: LayoutDirection::Vertical,
            margin: 1.0,
            style: Style {
                background_color: Some(Color::new(0.2, 0.2, 0.2, 1.0)),
                padding: 2.0,
                ..Default::default()
            },
            children: status_elements,
            ..Default::default()
        });

        let done_text = TextLine::new("Done", 18, LIGHTGRAY, Some(font.clone()));
        let end_turn_text = TextLine::new("End turn", 18, WHITE, Some(font.clone()));

        Self {
            character: Rc::clone(character),
            text,
            is_character_shown: Cell::new(false),
            is_character_active: Cell::new(false),
            padding: 10.0,
            has_been_clicked: Cell::new(false),
            texture,
            status_column,
            status_rects,
            status_textures,
            done_text,
            end_turn_text,
            has_clicked_end_turn: Cell::new(false),
            may_show_end_turn_button: Cell::new(false),
            hovered_status_rect: Cell::new(None),
            font,
            last_drawn_rect: Cell::new(Rect::default()),
        }
    }

    fn set_statuses(&mut self, statuses: &[ConditionInfo]) {
        self.hovered_status_rect.set(None);
        for (i, status_rect) in self.status_rects.iter().enumerate() {
            match statuses.get(i) {
                Some(info) => {
                    status_rect.borrow_mut().style.background_color = Some(BLACK);
                    status_rect.borrow_mut().texture =
                        Some(self.status_textures[&info.condition.status_icon()].clone());
                    if status_rect.borrow().has_been_hovered.take() {
                        self.hovered_status_rect.set(Some((i, *info)));
                    }
                }
                None => {
                    status_rect.borrow_mut().style.background_color = Some(BLACK);
                    status_rect.borrow_mut().texture = None;
                }
            };
        }
    }
}

const STATUS_ICON_W: f32 = 10.0;
impl Drawable for PlayerCharacterPortrait {
    fn draw(&self, x: f32, y: f32) {
        let (w, h) = (64.0, 80.0);
        draw_rectangle(x, y, w, h, DARKGRAY);
        draw_texture(PORTRAIT_BG_TEXTURE.get().unwrap(), x, y, WHITE);
        self.last_drawn_rect.set(Rect::new(x, y, w, h));

        let params = DrawTextureParams {
            dest_size: Some((w, h).into()),
            ..Default::default()
        };
        draw_texture_ex(&self.texture, x, y, WHITE, params);

        if self.character.is_dead() {
            draw_rectangle(x, y, w, h, Color::new(0.6, 0.0, 0.0, 0.5));
            draw_cross(x, y, w, h, RED, 2.0, 5.0);
        } else if !self.character.health.is_at_max() {
            let margin = 1.0;
            let w = 64.0 - margin * 2.0;
            let damage_h = (80.0 - margin * 2.0) * (1.0 - self.character.health.ratio());
            let x0 = x + margin;
            let y0 = y + margin;
            draw_rectangle(x0, y0, w, damage_h, Color::new(0.3, 0.0, 0.0, 0.8));
            draw_line(
                x0,
                y0 + damage_h,
                x0 + w,
                y0 + damage_h,
                3.0,
                Color::new(0.6, 0.0, 0.0, 0.6),
            );
        }

        if self.is_character_shown.get() {
            draw_rounded_rectangle_lines(x, y, w, h, 3.0, WHITE, 8.0, Some((BLACK, 3.0)));
        } else {
            draw_rounded_rectangle_lines(x, y, w, h, 1.0, GRAY, 4.0, Some((BLACK, 3.0)));
        }

        //self.text.draw(self.padding + x, self.padding + y);

        let button_h = 25.0;
        let button_text_vert_pad = 6.0;

        let button_y = y + h + 5.0;
        if self.character.has_taken_a_turn_this_round.get() {
            draw_rectangle_lines(x, button_y, w, button_h, 1.0, GRAY);
            self.done_text.draw(
                x + w / 2.0 - self.done_text.size().0 / 2.0,
                button_y + button_text_vert_pad,
            );
        }

        if self.is_character_active.get() {
            let x_mid = x + w / 2.0;
            let arrow_w = 14.0;
            let arrow_h = 7.0;
            let margin = 7.0;
            let v1 = (x_mid - arrow_w / 2.0, y - margin - arrow_h).into();
            let v2 = (x_mid + arrow_w / 2.0, y - margin - arrow_h).into();
            let v3 = (x_mid, y - margin).into();
            draw_triangle(v1, v2, v3, GOLD);
            draw_triangle_lines(v1, v2, v3, 1.0, LIGHTGRAY);

            draw_rectangle_lines(x, button_y, w, button_h, 1.0, LIGHTGRAY);
            self.end_turn_text.draw(
                x + w / 2.0 - self.end_turn_text.size().0 / 2.0,
                button_y + button_text_vert_pad,
            );
            if self.may_show_end_turn_button.get()
                && Rect::new(x, button_y, w, 20.0).contains(mouse_position().into())
            {
                draw_rectangle_lines(x + 2.0, button_y + 2.0, w - 4.0, button_h - 4.0, 1.0, WHITE);
                if is_mouse_button_pressed(MouseButton::Left) {
                    self.has_clicked_end_turn.set(true);
                }
            }
        }

        let (mouse_x, mouse_y) = mouse_position();
        let hovered = (x..x + w).contains(&mouse_x) && (y..y + h).contains(&mouse_y);

        if hovered && !self.is_character_shown.get() {
            draw_rectangle_lines(x + 1.0, y + 1.0, w - 2.0, h - 2.0, 1.0, LIGHTGRAY);
            if is_mouse_button_pressed(MouseButton::Left) {
                println!(
                    "Set portrait has been clicked mouse={:?}, {},{}",
                    (mouse_x, mouse_y),
                    x,
                    y
                );
                self.has_been_clicked.set(true);
            }
        }

        self.status_column.draw(x + w + 1.0, y + 1.0);
    }

    fn size(&self) -> (f32, f32) {
        //let text_size = self.text.size();
        //(text_size.0 + self.padding * 2.0, 15.0 + self.padding * 2.0)

        (64.0 + 3.0 + STATUS_ICON_W, 80.0)
    }
}

pub struct Log {
    container: Container,
    text_lines: Vec<Rc<TextLine>>,
    line_details: Vec<Option<Container>>,
    font: Font,
    padding: f32,
}

impl Log {
    pub fn new(font: Font) -> Self {
        let h = 200.0;
        Self {
            container: Container {
                layout_dir: LayoutDirection::Vertical,
                //reverse_vertical: false,
                children: vec![],
                margin: 4.0,
                align: Align::End,
                scroll: Some(ContainerScroll::default()),
                //min_width: Some(450.0),
                min_width: Some(430.0),
                min_height: Some(h),
                max_height: Some(h),
                style: Style {
                    border_color: Some(GRAY),
                    background_color: Some(Color::new(0.0, 0.0, 0.0, 0.85)),
                    padding: 5.0,
                    ..Default::default()
                },
                ..Default::default()
            },
            text_lines: vec![],
            line_details: vec![],
            font,
            padding: 10.0,
        }
    }

    pub fn width(&self) -> f32 {
        self.container.size().0 + self.padding * 2.0
    }

    pub fn add(&mut self, text: impl Into<String>) {
        self.add_with_details(text, &[]);
    }

    pub fn add_with_details(&mut self, text: impl Into<String>, details: &[String]) {
        const MAX_LINES: usize = 50;
        let text = text.into();
        if self.container.children.len() == MAX_LINES {
            self.container.children.remove(0);
            self.text_lines.remove(0);
            self.line_details.remove(0);
        }
        // TODO Support setting max width for TextLine, and having it line-wrap to fit inside the given width
        let mut text_line = TextLine::new(text, 18, WHITE, Some(self.font.clone()));
        text_line.set_padding(3.0, 3.0);
        text_line.set_max_width(self.container.min_width.unwrap());
        let text_line = Rc::new(text_line);

        self.text_lines.push(text_line.clone());
        self.container.push_child(Element::Rc(text_line));

        if !details.is_empty() {
            let details_container = Container {
                layout_dir: LayoutDirection::Vertical,
                margin: 5.0,
                style: Style {
                    background_color: Some(BLACK),
                    padding: 5.0,
                    border_color: Some(GOLD),
                    ..Default::default()
                },
                children: details
                    .iter()
                    .map(|s| Element::Text(TextLine::new(s, 18, WHITE, Some(self.font.clone()))))
                    .collect(),
                ..Default::default()
            };
            self.line_details.push(Some(details_container));
        } else {
            self.line_details.push(None);
        }
    }

    pub fn draw(&self, x: f32, y: f32) {
        self.container.draw(x + self.padding, y + self.padding);
    }

    pub fn draw_tooltips(&self, x: f32, y: f32) {
        let size = self.size();
        for (i, text_line) in self.text_lines.iter().enumerate() {
            if let Some(line_pos) = text_line.has_been_hovered.take() {
                if let Some(details) = &self.line_details[i] {
                    let popup_size = details.size();
                    let details_x = x + size.0 - details.size().0 - 10.0;
                    let mut details_y = line_pos.1 + text_line.size().1 + 5.0;

                    if details_y + popup_size.1 > y + size.1 {
                        details_y = line_pos.1 - popup_size.1 - 5.0;
                    }

                    details.draw(details_x, details_y);
                }
            }
        }
    }

    fn size(&self) -> (f32, f32) {
        let container_size = self.container.size();
        (
            container_size.0 + self.padding,
            container_size.1 + self.padding,
        )
    }
}

#[derive(Default)]
pub struct ActionPointsRow {
    pub is_characters_turn: bool,
    pub current_ap: u32,
    pub reserved_and_hovered_ap: (i32, i32),
    max_ap: u32,
    cell_size: (f32, f32),
    pub padding: f32,
    style: Style,
    radius_factor: f32,
}

impl ActionPointsRow {
    pub fn new(cell_size: (f32, f32), radius_factor: f32, style: Style) -> Self {
        Self {
            is_characters_turn: false,
            current_ap: 0,
            reserved_and_hovered_ap: (0, 0),
            max_ap: MAX_ACTION_POINTS,
            cell_size,
            radius_factor,
            padding: 3.0,
            style,
        }
    }
}

impl Drawable for ActionPointsRow {
    fn draw(&self, x: f32, y: f32) {
        assert!(self.current_ap <= self.max_ap);

        let size = self.size();
        //draw_rectangle(x, y, size.0, size.1, BLACK);
        self.style.draw_background(x, y, size);

        let mut x0 = (x + self.padding).floor();
        let y0 = (y + self.padding).floor();
        let r = (self.cell_size.1 * self.radius_factor).round();

        let (reserved_ap, hovered_ap) = self.reserved_and_hovered_ap;

        for i in 0..self.max_ap as i32 {
            let is_point_hovered = if hovered_ap >= 0 {
                ((self.current_ap as i32).saturating_sub(hovered_ap)..(self.current_ap as i32))
                    .contains(&i)
            } else {
                ((self.current_ap as i32)..((self.current_ap as i32) - hovered_ap)).contains(&i)
            };

            let mut overcomitted = false;
            let mut reserved = false;
            let mut available = false;
            let mut missing = false;

            if reserved_ap >= 0 {
                if i < self.current_ap as i32 - reserved_ap {
                    available = true;
                } else if i < self.current_ap as i32 {
                    reserved = true;
                } else if (i) < (reserved_ap).max(hovered_ap) {
                    overcomitted = true;
                } else {
                    missing = true;
                }
            } else {
                // A negative reserved_ap means that the player is about to make an action that will grant AP (such as
                // ending their turn)
                if i < self.current_ap as i32 {
                    available = true;
                } else if i < self.current_ap as i32 - reserved_ap {
                    reserved = true;
                } else {
                    missing = true;
                }
            }

            if available {
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    GOLD,
                );
            } else if reserved {
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    GOLD,
                );

                let game_time = get_time();
                let t = (game_time * 0.7).fract() as f32;
                let alpha = 0.2 + if t < 0.5 { t } else { 1.0 - t };

                draw_circle_lines(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    2.0,
                    Color::new(1.0, 1.0, 1.0, alpha),
                );
            } else if missing {
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    GRAY,
                );
            }

            if overcomitted {
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    GRAY,
                );
                draw_circle_lines(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    2.0,
                    RED,
                );
            } else if is_point_hovered {
                draw_circle_lines(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    2.0,
                    SKYBLUE,
                );
            } else {
                /*
                draw_circle_lines(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    1.0,
                    GRAY,
                );
                 */
            }

            x0 += self.cell_size.0;
        }

        self.style.draw_foreground(x, y, self.size());

        if reserved_ap > self.max_ap as i32 {
            let (w, h) = self.size();
            draw_rectangle_lines(x, y, w, h, 2.0, RED);
        }
    }

    fn size(&self) -> (f32, f32) {
        (
            self.max_ap as f32 * self.cell_size.0 + self.padding * 2.0,
            self.cell_size.1 + self.padding * 2.0,
        )
    }
}

pub struct ResourceBar {
    pub current: u32,
    pub reserved: u32,
    pub max: u32,
    pub color: Color,
    pub cell_size: (f32, f32),
    pub layout: LayoutDirection,
}

impl ResourceBar {
    pub fn horizontal(max: u32, color: Color, size: (f32, f32)) -> Self {
        Self {
            current: max,
            reserved: 0,
            max,
            color,
            cell_size: (size.0 / max as f32, size.1),
            layout: LayoutDirection::Horizontal,
        }
    }
}

impl Drawable for ResourceBar {
    fn draw(&self, x: f32, y: f32) {
        assert!(self.current <= self.max);

        let cell_size = self.cell_size;
        let mut x0 = x;
        let mut y0 = y;

        match self.layout {
            LayoutDirection::Horizontal => {
                for i in 0..self.max {
                    if i < self.current {
                        draw_rectangle(x0, y0, cell_size.0, cell_size.1, self.color);
                        if i >= self.current.saturating_sub(self.reserved) {
                            draw_rectangle(
                                x0,
                                y0,
                                cell_size.0,
                                cell_size.1,
                                Color::new(1.0, 1.0, 1.0, 0.5),
                            );
                        }
                    }

                    if i > 0 {
                        let thick = if self.max < 8 { true } else { i % 5 == 0 };

                        let (thickness, color, space) = if thick {
                            (2.0, BLACK, self.cell_size.1 * 0.14)
                        } else {
                            (1.0, DARKGRAY, self.cell_size.1 * 0.3)
                        };

                        draw_line(
                            x0,
                            y0 + space,
                            x0,
                            y0 + cell_size.1 - space,
                            thickness,
                            color,
                        );
                    }
                    x0 += cell_size.0;
                }

                draw_rectangle_lines(x, y, self.max as f32 * cell_size.0, cell_size.1, 1.0, WHITE);
            }
            LayoutDirection::Vertical => {
                for i in 0..self.max {
                    if i >= self.max - self.current {
                        draw_rectangle(x0, y0, cell_size.0, cell_size.1, self.color);
                        if i < self.max - self.current + self.reserved {
                            draw_rectangle(
                                x0,
                                y0,
                                cell_size.0,
                                cell_size.1,
                                Color::new(1.0, 1.0, 1.0, 0.5),
                            );
                        }
                    }

                    if i > 0 {
                        let thick = if self.max < 8 {
                            true
                        } else {
                            (self.max - i) % 5 == 0
                        };

                        let (thickness, color, space) = if thick {
                            (2.0, BLACK, self.cell_size.0 * 0.14)
                        } else {
                            (1.0, DARKGRAY, self.cell_size.0 * 0.3)
                        };

                        draw_line(
                            x0 + space,
                            y0,
                            x0 + cell_size.0 - space,
                            y0,
                            thickness,
                            color,
                        );
                    }
                    y0 += cell_size.1;
                }

                draw_rectangle_lines(x, y, cell_size.0, self.max as f32 * cell_size.1, 1.0, WHITE);
            }
        }
    }

    fn size(&self) -> (f32, f32) {
        match self.layout {
            LayoutDirection::Horizontal => (self.cell_size.0 * self.max as f32, self.cell_size.1),
            LayoutDirection::Vertical => (self.cell_size.0, self.cell_size.1 * self.max as f32),
        }
    }
}

pub struct LabelledResourceBar {
    list: Container,
    bar: Rc<RefCell<ResourceBar>>,
    value_text: Rc<RefCell<TextLine>>,
    max_value: u32,
}

impl LabelledResourceBar {
    pub fn new(current: u32, max: u32, label: &'static str, color: Color, font: Font) -> Self {
        assert!(current <= max);

        let cell_h = 12.0;
        let max_w = 70.0;
        /*
        let cell_w = if max <= 7 {
            max_w / 7.0
        } else {
            max_w / max as f32
        };
         */
        let cell_w = max_w / max as f32;
        let bar = Rc::new(RefCell::new(ResourceBar {
            current,
            reserved: 0,
            max,
            color,
            cell_size: (cell_w, cell_h),
            layout: LayoutDirection::Horizontal,
        }));
        let cloned_bar = Rc::clone(&bar);

        let value_text = Rc::new(RefCell::new(TextLine::new(
            format!("{}/{}", current, max),
            20,
            WHITE,
            Some(font.clone()),
        )));
        let cloned_value_text = Rc::clone(&value_text);
        let label_text = TextLine::new(label, 16, WHITE, Some(font.clone()));

        let list = Container {
            layout_dir: LayoutDirection::Horizontal,
            align: Align::Start,
            margin: 5.0,
            children: vec![
                Element::RcRefCell(cloned_bar),
                Element::RcRefCell(cloned_value_text),
                //Element::Text(label_text),
            ],
            min_width: Some(40.0),
            ..Default::default()
        };

        Self {
            list,
            bar,
            value_text,
            max_value: max,
        }
    }

    pub fn set_current(&mut self, value: u32) {
        assert!(value <= self.bar.borrow().max);
        self.bar.borrow_mut().current = value;
        self.value_text
            .borrow_mut()
            .set_string(format!("{}/{}", value, self.max_value));
    }

    pub fn set_reserved(&mut self, value: u32) {
        self.bar.borrow_mut().reserved = value;
    }
}

impl Drawable for LabelledResourceBar {
    fn draw(&self, x: f32, y: f32) {
        if self.max_value > 0 {
            self.list.draw(x, y);
        }
    }

    fn size(&self) -> (f32, f32) {
        if self.max_value > 0 {
            self.list.size()
        } else {
            (0.0, 0.0)
        }
    }
}
