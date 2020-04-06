use crate::Terminal;
use crate::Document;
use crate::Row;

use std::env;
use std::time::Duration;
use std::time::Instant;

use termion::{event::Key, color}; 

const VERSION: &str = env!("CARGO_PKG_VERSION");
const STATUS_BG_COLOR: color::Rgb = color::Rgb(0, 50, 100);
const STATUS_FG_COLOR: color::Rgb = color::Rgb(255, 255, 255);
const QUIT_TIMES: u8 = 2;

#[derive(Default)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

pub struct Editor {
    // Document hold the file and its contents
    document: Document,
    // boolean to decide if the editor should quit now
    should_quit: bool,
    terminal: Terminal,
    // (x,y) coordinate of the cursor on the screen
    cursor_position: Position,
    // Offset decides what portion of the file is shown on screen at a time
    offset: Position,
    // message to display in the status bar
    status_message: StatusMessage,
    quit_times: u8,
}

struct StatusMessage {
    text: String,
    time: Instant,
}

impl StatusMessage {
    fn from(message: String) -> Self {
        Self {
            time: Instant::now(),
            text: message,
        }
    }
}

impl Editor {
    pub fn default() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut initial_status = String::from("HELP: Ctrl-F = find | Ctrl-S = save | Ctrl-Q = quit");
        let document = if args.len() > 1 {
            let file_name = &args[1];
            let doc = Document::open(&file_name);
            if doc.is_ok() {
                doc.unwrap()
            } else {
                initial_status = format!("ERR: Could not open file: {}", file_name);
                Document::default()
            }
        } else {
            Document::default()
        };

        Self {
            should_quit: false,
            terminal: Terminal::default().expect("Failed to initialise terminal"),
            cursor_position: Position::default(),
            document,
            offset: Position::default(),
            status_message: StatusMessage::from(initial_status),
            quit_times: QUIT_TIMES,
        }
    }

    pub fn draw_row(&self, row: &Row) {
        // let start = 0;
        // let end = self.terminal.size().width as usize;
        let width = self.terminal.size().width as usize;
        let start = self.offset.x;
        let end = self.offset.x + width;

        let row = row.render(start, end);
        println!("{}\r", row);
    }

    pub fn run(&mut self) {
        loop {
            if let Err(error) = self.refresh_screen() {
                die(error);
            }
            if self.should_quit {
                break;
            }
            if let Err(error) = self.process_kepress() {
                die(error);
            }
        }
    }

    fn refresh_screen(&self) -> Result<(), std::io::Error>{
        Terminal::cursor_hide();
        Terminal::cursor_position(&Position::default());
        if self.should_quit {
            Terminal::clear_screen();
            println!("Goodbye!\r");
        } else {
            self.draw_rows();
            self.draw_status_bar();
            self.draw_message_bar();
            // Position the cursor properly when scrolling up
            Terminal::cursor_position(&Position {
                x: self.cursor_position.x.saturating_sub(self.offset.x),
                y: self.cursor_position.y.saturating_sub(self.offset.y),
            });
        }
        Terminal::cursor_show();
        Terminal::flush()
    }

    fn save(&mut self) {
        if self.document.file_name.is_none() {
            let new_name = self.prompt("Save as: ", |_, _, _| {}).unwrap_or(None);
            if new_name.is_none() {
                self.status_message = StatusMessage::from("Save aborted".to_string());
                return;
            }
            self.document.file_name = new_name;
        }
            
        if self.document.save().is_ok() {
            self.status_message = StatusMessage::from("File saved succesfully".to_string());
        } else {
            self.status_message = StatusMessage::from("Error writing file.".to_string());
        }
    }

    fn process_kepress(&mut self) -> Result<(), std::io::Error> {
        let pressed_key = Terminal::read_key()?;
        match pressed_key {
            Key::Ctrl('q') => {
                if self.quit_times > 0 && self.document.is_dirty() {
                    self.status_message = StatusMessage::from(format!(
                        "WARNING! File has unsaved changes. Press Ctrl-Q {} more times to quit.",
                        self.quit_times,
                    ));
                    self.quit_times -= 1;
                    return Ok(());
                }
                self.should_quit = true;
            },
            Key::Ctrl('f') => {
                if let Some(query) = self
                .prompt("Search: ", |editor, _, query| {
                    if let Some(position) = editor.document.find(&query) {
                        editor.cursor_position = position;
                        editor.scroll();
                    }
                })
                .unwrap_or(None)
                {
                    if let Some(position) = self.document.find(&query[..]) {
                        self.cursor_position = position;
                    } else {
                        self.status_message = StatusMessage::from(format!("Not found :{}", query));
                    }       
                }
            },
            Key::Ctrl('s') => self.save(),
            Key::Ctrl('h') => {
                self.status_message = StatusMessage::from("HELP: Ctrl-F = find | Ctrl-S = save | Ctrl-Q = quit".to_string());
            }
            Key::Char(c) => {
                // don't move cursor to the right if enter is pressed
                self.move_cursor(Key::Right);
                self.document.insert(&self.cursor_position, c);
            },
            Key::Delete => self.document.delete(&self.cursor_position),
            Key::Backspace => {
                // Backspace = going left and perform delete
                if self.cursor_position.x > 0 || self.cursor_position.y > 0 {
                    self.move_cursor(Key::Left);
                    self.document.delete(&self.cursor_position);
                }
            }
            Key::Up
            | Key::Down
            | Key::Left
            | Key::Right
            | Key::PageUp
            | Key::PageDown
            | Key::Home
            | Key::End => self.move_cursor(pressed_key),
            _ => (),
        }
        self.scroll();
        if self.quit_times < QUIT_TIMES {
            self.quit_times = QUIT_TIMES;
            self.status_message = StatusMessage::from(String::new());
        }
        Ok(())
    }

    fn scroll(&mut self) {
        let Position { x, y } = self.cursor_position;
        let width = self.terminal.size().width as usize;
        let height = self.terminal.size().height as usize;
        let mut offset = &mut self.offset;

        // vertical scrolling
        if y < offset.y {
            offset.y = y;
        } else if y >= offset.y.saturating_add(height) {
            // scroll vertically one line at a time
            offset.y = offset.y.saturating_add(1);
        }

        // horizontal scrolling
        if x < offset.x {
            offset.x = x;   
        } else if x >= offset.x.saturating_add(width) {
            // scroll horizontally one letter at a time
            offset.x = offset.x.saturating_add(1);
        }
    }

    fn draw_rows(&self) {
        let height = self.terminal.size().height;
        for terminal_row in 0..height {
            Terminal::clear_current_line();

            // If document is empty print hello message else print
            // document content. Print caret at beginning of every empty line
            if let Some(row) = self.document.row(terminal_row as usize + self.offset.y) {
                self.draw_row(row);
            } else if self.document.is_empty() && terminal_row == height / 3 {
                self.draw_welcome_message();
            } else {
                println!("~\r");
            }
        }
    }

    fn draw_status_bar(&self) {
        let mut status;
        let width = self.terminal.size().width as usize;

        // if the document is dirty, show indicator
        let modified_indicator = if self.document.is_dirty() {
            " (modified)"
        } else {
            ""
        };

        let mut file_name = "[No Name]".to_string();
        if let Some(name) = &self.document.file_name {
            file_name = name.clone();
            file_name.truncate(20);
        }
        status = format!(
            "{} - {} lines{}",
            file_name,
            self.document.len(),
            modified_indicator,
        );
        
        let line_indicator = format! (
            "Ln {}, Col {}",
            self.cursor_position.y.saturating_add(1),
            self.cursor_position.x.saturating_add(1),
        );

        let len = status.len() + line_indicator.len();
        if width > len {
            status.push_str(&" ".repeat(width - len));
        }
        status = format!("{}{}", status, line_indicator);
        status.truncate(width);
        Terminal::set_bg_color(STATUS_BG_COLOR);
        Terminal::set_fg_color(STATUS_FG_COLOR);
        println!("{}\r", status);

        // reset the bg and fg colors so that only status is printed in these colors
        Terminal::reset_fg_color();
        Terminal::reset_bg_color();
    }

    fn draw_message_bar(&self) {
        Terminal::clear_current_line();
        let message = &self.status_message;
        if Instant::now() - message.time < Duration::new(5, 0) {
            let mut text = message.text.clone();
            text.truncate(self.terminal.size().width as usize);
            print!("{}", text);
        }
    }
    
    fn draw_welcome_message(&self) {
        let mut welcome_message = format!("Editr -- version {}", VERSION);
        let width = self.terminal.size().width as usize;
        let len = welcome_message.len();
        let padding = width.saturating_sub(len) / 2;
        let spaces = " ".repeat(padding.saturating_sub(1));
        welcome_message = format!("~{}{}", spaces, welcome_message);
        welcome_message.truncate(width);
        println!("{}\r", welcome_message);
    }

    fn move_cursor(&mut self, key: Key)  {
        let terminal_height = self.terminal.size().height as usize;
        let Position { mut x, mut y } = self.cursor_position;
        // let size = self.terminal.size();
        let height = self.document.len();
        let mut width = if let Some(row) = self.document.row(y) {
            row.len()
        } else {
            0
        };

        match key {
            Key::Up => y = y.saturating_sub(1),
            Key::Down => {
                if y < height {
                    y = y.saturating_add(1);
                }
            },
            Key::Left => {
                if x > 0 {
                    x = x.saturating_sub(1);
                } else if y > 0 {
                    y = y.saturating_sub(1);
                    // set x to width of above row
                    x = self.document.row(y).unwrap().len();
                }
            },
            Key::Right => {
                if x < width {
                    x += 1;
                } else if y < height {
                    y += 1;
                    x = 0;
                }
            },
            Key::PageUp => {
                y = if y > terminal_height {
                    y - terminal_height
                } else {
                    0
                };
            },
            Key::PageDown => {
                y = if y.saturating_add(terminal_height) < height {
                    y + terminal_height as usize
                } else {
                    height
                };
            },
            Key::Home => x = 0,
            Key::End => x = width,
            _ => (),
        }
        width = if let Some(row) = self.document.row(y) {
            row.len()
        } else {
            0
        };
        if x > width {
            x = width;
        }
        
        self.cursor_position = Position { x, y };
    }

    fn prompt<C>(&mut self, prompt: &str, callback: C) -> Result<Option<String>, std::io::Error>
        where
            C: Fn(&mut Self, Key, &String)
    {
        let mut result = String::new();
        loop {
            self.status_message = StatusMessage::from(format!("{}{}", prompt, result));
            self.refresh_screen()?;

            let key = Terminal::read_key()?;
            match key {
                Key::Backspace => {
                    if !result.is_empty() {
                        result.truncate(result.len() - 1);
                    }
                },
                Key::Char('\n') => break,
                Key::Char(c) => {
                    if !c.is_control() {
                        result.push(c);
                    }
                },
                Key::Esc => {
                    result.truncate(0);
                    break;
                },
                _ => (),
            }
            callback(self, key, &result);
        }
        self.status_message = StatusMessage::from(String::new());
        if result.is_empty() {
            return Ok(None);
        }
        Ok(Some(result))
    }
}

fn die(e: std::io::Error) {
    Terminal::clear_screen();
    panic!(e);
}
