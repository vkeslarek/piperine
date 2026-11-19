//! `Chapter`: the Book and its inventory, read once for a chapter test, and the models the Book's
//! blocks make. A chapter test names a block by id; the inventory checks the block's fingerprint, so
//! a block that moved or changed fails by name instead of running another text.

use super::book::Book;
use super::inventory::Inventory;
use super::model::Model;

/// The Book and its inventory as they stand in this repository.
pub struct Chapter {
    book: Book,
    inventory: Inventory,
}

impl Chapter {
    /// The Book and the inventory of this repository.
    pub fn read() -> Chapter {
        Chapter { book: Book::read(), inventory: Inventory::read() }
    }

    /// A lone-file model of the blocks `ids`, written as `file_name`.
    pub fn lone(&self, file_name: &str, ids: &[&str]) -> Model {
        Model::lone(&self.book, &self.inventory, file_name, ids)
    }

    /// The file the Book assembles at `path` as it stands at the block `upto`.
    pub fn marked(&self, path: &str, upto: &str) -> Model {
        Model::marked(&self.book, &self.inventory, path, upto)
    }

    /// The text of the block `id`, after its fingerprint is checked.
    pub fn text(&self, id: &str) -> String {
        self.inventory.checked_block(&self.book, id).text().to_string()
    }
}
