//! The Book's buck listing held to the binary (feature `integrated-tests`, IT-11): the block of
//! chapter 15 that shows `piperine run startup` in `examples/buck` is what the binary prints, whole.
//! The run is the 600 us start of the converter (minutes in a debug build), so the target runs on
//! demand (`test = false`, MD-77; `examples/buck/README.md`, "On demand"); `book` holds every other
//! listing of the Book and the buck's files.

mod book_listing {
    //! The Book's command listings as typed values: every `<pre>` block of `docs/book/content.html`
    //! that carries a `data-cli-output` attribute, with its project, its command and its decoded text.
    //! The on-demand `book_buck` target reads the listings of project `buck` through it; `book.rs`
    //! keeps its own reader for the other projects.
    use std::path::{Path, PathBuf};
    use std::process::Command;

    /// One listing of the Book: a block that claims to be what a `piperine` command prints.
    pub struct BookListing {
        /// The `data-cli-project` attribute: the project the command runs in, when the block names one.
        pub project: Option<String>,
        /// The `data-cli-output` attribute: the command, starting with `piperine`.
        pub command: String,
        /// The block's decoded text, as the Book prints it.
        pub text: String,
    }

    impl BookListing {
        /// The opening of a block that holds a listing.
        const OPENING: &'static str = "<pre data-lang=\"";

        /// The repository root.
        fn repository() -> PathBuf {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
        }

        /// The five entities the Book's source uses, decoded.
        fn decoded(text: &str) -> String {
            text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
        }

        /// The value of attribute `name` in the tail of an opening tag, decoded.
        fn attribute(tail: &str, name: &str) -> Option<String> {
            let marker = format!("{name}=\"");
            let after = &tail[tail.find(&marker)? + marker.len()..];
            Some(Self::decoded(after.split_once('"')?.0))
        }

        /// Every listing of the whole Book, in order.
        pub fn all() -> Vec<BookListing> {
            let html = std::fs::read_to_string(Self::repository().join("docs/book/content.html")).expect("docs/book/content.html exists");
            let mut listings = Vec::new();
            for block in html.split(Self::OPENING).skip(1) {
                let (_, rest) = block.split_once('"').expect("the language closes");
                let (tail, body) = rest.split_once('>').expect("a block's opening tag closes");
                let Some(command) = Self::attribute(tail, "data-cli-output") else { continue };
                let text = Self::decoded(body.split_once("</pre>").expect("a block closes").0);
                listings.push(BookListing { project: Self::attribute(tail, "data-cli-project"), command, text });
            }
            listings
        }

        /// The listings whose `data-cli-project` is `name`.
        pub fn of_project(name: &str) -> Vec<BookListing> {
            Self::all().into_iter().filter(|listing| listing.project.as_deref() == Some(name)).collect()
        }

        /// What the binary prints on standard output for this listing's command, run in `examples/<project>`
        /// on the cache directory `cache`, without its trailing line breaks.
        pub fn printed(&self, cache: &Path) -> String {
            let words: Vec<&str> = self.command.split_whitespace().collect();
            assert_eq!(words.first(), Some(&"piperine"), "a listing's command starts with `piperine`: {}", self.command);
            let project = self.project.as_deref().expect("a listing of an example names its project");
            let out = Command::new(env!("CARGO_BIN_EXE_piperine"))
                .args(&words[1..])
                .current_dir(Self::repository().join("examples").join(project))
                .env("PIPERINE_CACHE_DIR", cache)
                .env_remove("PIPERINE_BACKEND")
                .output()
                .expect("run piperine");
            assert_eq!(out.status.code(), Some(0), "`{}`: stderr:\n{}", self.command, String::from_utf8_lossy(&out.stderr));
            String::from_utf8(out.stdout).expect("text").trim_end_matches('\n').to_string()
        }
    }
}

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
