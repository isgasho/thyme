use crate::render::{TexCoord, DrawList, FontHandle};
use crate::{Point, Rect, Align, Color};

pub struct FontSource {
    pub(crate) font: rusttype::Font<'static>,
}

pub struct FontChar {
    pub size: Point,
    pub(crate) tex_coords: [TexCoord; 2],
    pub x_advance: f32,
    pub y_offset: f32,
}

impl Default for FontChar {
    fn default() -> Self {
        FontChar {
            size: Point::default(),
            tex_coords: [TexCoord::new(0.0, 0.0), TexCoord::new(0.0, 0.0)],
            x_advance: 0.0,
            y_offset: 0.0,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct FontSummary {
    pub handle: FontHandle,
    pub line_height: f32,
}

pub struct Font {
    handle: FontHandle,
    characters: Vec<FontChar>,
    line_height: f32,
    ascent: f32,
}

impl Font {
    pub(crate) fn new(handle: FontHandle, characters: Vec<FontChar>, line_height: f32, ascent: f32) -> Font {
        Font {
            handle,
            characters,
            line_height,
            ascent,
        }
    }

    fn char(&self, c: char) -> Option<&FontChar> {
        self.characters.get(c as usize) // TODO smarter lookup
    }

    pub fn line_height(&self) -> f32 { self.line_height }

    pub fn ascent(&self) -> f32 { self.ascent }

    pub fn handle(&self) -> FontHandle { self.handle }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw<D: DrawList>(
        &self,
        draw_list: &mut D,
        area_size: Point,
        pos: [f32; 2],
        text: &str,
        align: Align,
        color: Color,
        clip: Rect,
    ) {
        let renderer = FontRenderer::new(
            &self,
            draw_list,
            area_size,
            pos.into(),
            align,
            color,
            clip
        );
        renderer.render(text);
    }
}

struct FontRenderer<'a, D> {
    font: &'a Font,
    draw_list: &'a mut D,
    initial_index: usize,

    clip: Rect,
    align: Align,
    color: Color,

    area_size: Point,
    initial_pos: Point,

    pos: Point,
    size: Point,
    cur_line_index: usize,

    cur_word: Vec<&'a FontChar>,
    cur_word_width: f32,
}

impl<'a, D: DrawList> FontRenderer<'a, D> {
    fn new(
        font: &'a Font,
        draw_list: &'a mut D,
        area_size: Point,
        pos: Point,
        align: Align,
        color: Color,
        clip: Rect,
    ) -> FontRenderer<'a, D> {
        let initial_index = draw_list.len();

        FontRenderer {
            font,
            draw_list,
            initial_index,
            align,
            color,
            clip,
            area_size,
            initial_pos: pos,
            pos,
            size: Point::default(),
            cur_line_index: initial_index,
            cur_word: Vec::new(),
            cur_word_width: 0.0,
        }
    }

    fn render(mut self, text: &str) {
        for c in text.chars() {
            let font_char = match self.font.char(c) {
                None => continue, // TODO draw a special character here?
                Some(char) => char,
            };

            if c == '\n' {
                self.draw_cur_word();
                self.next_line();
            } else if c.is_whitespace() {
                self.draw_cur_word();

                // don't draw whitespace at the start of a line
                if self.cur_line_index != self.draw_list.len() {
                    self.pos.x += font_char.x_advance;
                    self.size.x += font_char.x_advance;
                }

                continue;
            }

            self.cur_word_width += font_char.x_advance;
            self.cur_word.push(font_char);

            if self.size.x + self.cur_word_width > self.area_size.x {
                // if the word was so long that we drew nothing at all
                if self.cur_line_index == self.draw_list.len() {
                    self.draw_cur_word();
                    self.next_line();
                } else {
                    self.next_line();
                    self.draw_cur_word();
                }
            }
        }

        self.draw_cur_word();

        if self.cur_line_index < self.draw_list.len() {    
            // adjust characters on the last line
            self.adjust_line_x();
            self.size.y += self.font.line_height;
        }

        self.adjust_all_y();
    }

    fn draw_cur_word(&mut self) {
        for font_char in self.cur_word.drain(..) {
            self.draw_list.push_rect(
                [self.pos.x, self.pos.y + font_char.y_offset + self.font.ascent],
                [font_char.size.x, font_char.size.y],
                font_char.tex_coords,
                self.color,
                self.clip,
            );
            self.pos.x += font_char.x_advance;
            self.size.x += font_char.x_advance;
        }
        self.cur_word_width = 0.0;
    }

    fn next_line(&mut self) {
        self.pos.y += self.font.line_height;
        self.size.y += self.font.line_height;
        self.pos.x = self.initial_pos.x;

        self.adjust_line_x();
        self.cur_line_index = self.draw_list.len();
        self.size.x = 0.0;
    }

    fn adjust_all_y(&mut self) {
        use Align::*;
        let y_offset = match self.align {
            TopLeft =>  0.0,
            TopRight => 0.0,
            BotLeft =>  self.area_size.y - self.size.y,
            BotRight => self.area_size.y - self.size.y,
            Left =>     (self.area_size.y - self.size.y) / 2.0,
            Right =>    (self.area_size.y - self.size.y) / 2.0,
            Bot =>      self.area_size.y - self.size.y,
            Top =>      0.0,
            Center =>   (self.area_size.y - self.size.y) / 2.0,
        };

        self.draw_list.back_adjust_positions(
            self.initial_index,
            Point { x: 0.0, y: y_offset }
        );
    }

    fn adjust_line_x(&mut self) {
        use Align::*;
        let x_offset = match self.align {
            TopLeft =>  0.0,
            TopRight => self.area_size.x - self.size.x,
            BotLeft =>  0.0,
            BotRight => self.area_size.x - self.size.x,
            Left =>     0.0,
            Right =>    self.area_size.x - self.size.x,
            Bot =>      (self.area_size.x - self.size.x) / 2.0,
            Top =>      (self.area_size.x - self.size.x) / 2.0,
            Center =>   (self.area_size.x - self.size.x) / 2.0,
        };
    
        self.draw_list.back_adjust_positions(
            self.cur_line_index,
            Point { x: x_offset, y: 0.0 }
        );
    }
}