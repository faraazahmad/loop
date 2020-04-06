// #![warn(clippy::all, clippy::pedantic, clippy::restriction)]
#![warn(clippy::all, clippy::pedantic)]

mod editor;
mod terminal;
mod row;
mod document;

use editor::Editor;
pub use editor::Position;
pub use terminal::Terminal;
pub use document::Document;
pub use row::Row;

fn main() {
    let args = std::env::args();
    if args.len() > 2 {
        println!("Too many arguments");
        return;
    }

    // let path = args.nth(1).unwrap();
    // println!("Editing {}", path);
    Editor::default().run();
}
