//! The Book's buck listing held to the binary (feature `integrated-tests`, IT-11): the block of
//! chapter 15 that shows `piperine run startup` in `examples/buck` is what the binary prints, whole.
//! The run is the 600 us start of the converter (minutes in a debug build), so the target runs on
//! demand (`test = false`, MD-76; `examples/buck/README.md`, "On demand"); `book` holds every other
//! listing of the Book and the buck's files.

#[allow(dead_code)] // the support module is shared by the Book targets of this crate
#[path = "support/book_listing.rs"]
mod book_listing;

use book_listing::BookListing;

#[test]
fn the_buck_block_is_what_the_binary_prints() {
    // Book claims held here: cli~9c4b6fcf
    // CLI-19 AC 4: the standard output of `piperine run startup` in `examples/buck` equal to what the
    // binary prints
    let cache = tempfile::tempdir().expect("tmp");
    let listings = BookListing::of_project("buck");
    let mut failures = Vec::new();
    for listing in &listings {
        let printed = listing.printed(cache.path());
        if printed != listing.text.trim_end_matches('\n') {
            failures.push(format!("`{}`:\n--- the Book\n{}\n--- the binary\n{printed}", listing.command, listing.text));
        }
    }
    assert!(failures.is_empty(), "{} listings differ from the binary:\n{}", failures.len(), failures.join("\n\n"));
    let commands: Vec<&str> = listings.iter().map(|listing| listing.command.as_str()).collect();
    assert_eq!(commands, ["piperine run startup"]);
}
