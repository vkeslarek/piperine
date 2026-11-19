//! The Book as the guards and the chapter targets read it: `docs/book/content.html`, read with
//! plain string functions (no HTML parser, no workspace crate), into chapters, code blocks and
//! prose sentences. The block rules follow `piperine-cli/tests/book.rs`: a block is
//! `<pre data-lang="..">` (a mermaid diagram is not one), its other attributes follow the language,
//! and an HTML comment `<!-- cli-file: <path> -->` right before it marks the file it belongs to.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

/// A 64-bit FNV-1a hash of a text: the identity of a block or a sentence, so a text that changed
/// or moved is found by name instead of being run in place of another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fingerprint(u64);

impl Fingerprint {
    /// The fingerprint of `text`.
    pub fn of(text: &str) -> Fingerprint {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in text.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Fingerprint(hash)
    }
}

impl fmt::Display for Fingerprint {
    /// Sixteen lowercase hex digits.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:016x}", self.0)
    }
}

/// The name of one code block: its chapter and its 1-based position among the chapter's blocks,
/// written `welcome#3`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockId {
    pub chapter: String,
    pub index: usize,
}

impl BlockId {
    /// The id spelled `<chapter>#<index>`, or `None` when the text is not one.
    pub fn parse(text: &str) -> Option<BlockId> {
        let (chapter, index) = text.split_once('#')?;
        let index = index.parse::<usize>().ok().filter(|index| *index >= 1)?;
        (!chapter.is_empty()).then(|| BlockId { chapter: chapter.to_string(), index })
    }
}

impl fmt::Display for BlockId {
    /// `<chapter>#<index>`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}#{}", self.chapter, self.index)
    }
}

/// One `<pre data-lang="..">` block: its id, where it starts in the Book, its language, its
/// decoded text and the `cli-file` path marked right before it.
#[derive(Clone, Debug)]
pub struct Block {
    id: BlockId,
    offset: usize,
    language: String,
    attributes: BTreeMap<String, String>,
    text: String,
    file: Option<String>,
}

impl Block {
    /// The block's name.
    pub fn id(&self) -> &BlockId {
        &self.id
    }

    /// The language the Book gives the block (`msl`, `text`, `sh`, ...).
    pub fn language(&self) -> &str {
        &self.language
    }

    /// The value of the attribute `name` of the block's opening tag (`data-cli-output`), decoded.
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(String::as_str)
    }

    /// The block's text, decoded.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The path of the file the block is part of, when a `cli-file` marker comes right before it.
    pub fn file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    /// The fingerprint of the block's text.
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of(&self.text)
    }
}

/// One sentence of the Book's prose, tags removed and whitespace collapsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sentence {
    text: String,
}

impl Sentence {
    /// The sentence whose text is `text`.
    pub fn new(text: &str) -> Sentence {
        Sentence { text: text.to_string() }
    }

    /// The sentence as the reader sees it.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The fingerprint of the sentence's text.
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of(&self.text)
    }
}

/// The Book: its HTML, and every block read out of it.
pub struct Book {
    html: String,
    chapters: Vec<(String, usize)>,
    blocks: Vec<Block>,
}

impl Book {
    /// The marker that opens a chapter.
    const CHAPTER_OPEN: &'static str = "<section id=\"";
    /// The opening of a code block; its other attributes follow the language.
    const BLOCK_OPEN: &'static str = "<pre data-lang=\"";

    /// `docs/book/content.html` of this repository, read and split into chapters and blocks.
    pub fn read() -> Book {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/book/content.html");
        let html = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: the Book is unreadable: {error}", path.display()));
        Book::from_html(html)
    }

    /// The Book whose HTML is `html` (a guard test plants one).
    pub fn from_html(html: String) -> Book {
        let chapters = Self::chapters_of(&html);
        let blocks = Self::blocks_of(&html, &chapters);
        Book { html, chapters, blocks }
    }

    /// Every chapter's id and the offset of its `<section>` tag, in order.
    fn chapters_of(html: &str) -> Vec<(String, usize)> {
        let mut chapters = Vec::new();
        let mut from = 0;
        while let Some(found) = html[from..].find(Self::CHAPTER_OPEN) {
            let at = from + found;
            let after = &html[at + Self::CHAPTER_OPEN.len()..];
            let (id, tail) = after.split_once('"').expect("a chapter's id closes");
            if tail.starts_with(" class=\"chapter\"") {
                chapters.push((id.to_string(), at));
            }
            from = at + 1;
        }
        chapters
    }

    /// Every block of `html`, in order, named by its chapter and position.
    fn blocks_of(html: &str, chapters: &[(String, usize)]) -> Vec<Block> {
        let mut blocks: Vec<Block> = Vec::new();
        let mut from = 0;
        while let Some(found) = html[from..].find(Self::BLOCK_OPEN) {
            let offset = from + found;
            let after = &html[offset + Self::BLOCK_OPEN.len()..];
            let (language, rest) = after.split_once('"').expect("a block's language closes");
            let (tail, body) = rest.split_once('>').expect("a block's opening tag closes");
            let (text, _) = body.split_once("</pre>").expect("a block closes");
            let chapter = chapters.iter().rev().find(|(_, start)| *start < offset).map(|(id, _)| id.clone()).unwrap_or_else(|| panic!("a block at offset {offset} lies in no chapter"));
            let index = blocks.iter().filter(|block| block.id.chapter == chapter).count() + 1;
            let before = html[..offset].trim_end();
            let file = before.strip_suffix("-->").and_then(|head| head.rsplit_once("<!--")).and_then(|(_, comment)| comment.strip_prefix(" cli-file: ")).map(|path| path.trim().to_string());
            blocks.push(Block { id: BlockId { chapter, index }, offset, language: language.to_string(), attributes: Self::attributes(tail), text: Self::decode(text), file });
            from = offset + 1;
        }
        blocks
    }

    /// The attributes `name="value"` of an opening tag's tail, decoded, by name.
    fn attributes(tail: &str) -> BTreeMap<String, String> {
        let mut attributes = BTreeMap::new();
        let mut rest = tail;
        while let Some((name, after)) = rest.split_once("=\"") {
            let (value, next) = after.split_once('"').expect("an attribute value closes");
            attributes.insert(name.trim().to_string(), Self::decode(value));
            rest = next;
        }
        attributes
    }

    /// `text` with the five entities the Book's source uses decoded.
    fn decode(text: &str) -> String {
        text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
    }

    /// The ids of the chapters, in the Book's order.
    pub fn chapters(&self) -> Vec<String> {
        self.chapters.iter().map(|(id, _)| id.clone()).collect()
    }

    /// Every block of the Book, in order.
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// The block named `id`. Panics naming the id when the Book has no such block.
    pub fn block(&self, id: &BlockId) -> &Block {
        self.blocks.iter().find(|block| block.id == *id).unwrap_or_else(|| panic!("the Book has no block {id}"))
    }

    /// The file the Book marks `path`, as it stands at the block `upto`: the blocks marked
    /// `cli-file: <path>` up to and including `upto`, each followed by a newline, in order.
    pub fn file(&self, path: &str, upto: &BlockId) -> String {
        let end = self.block(upto).offset;
        let mut file = String::new();
        for block in self.blocks.iter().filter(|block| block.offset <= end && block.file() == Some(path)) {
            file.push_str(&block.text);
            file.push('\n');
        }
        file
    }

    /// The sentences of the prose of `chapter`: the text of its `<p>`, `<li>` and `<td>` elements
    /// outside any `<pre>`, split where a full stop, a question mark or an exclamation mark is
    /// followed by a capital letter or a digit. Panics naming the chapter when the Book has none.
    pub fn sentences(&self, chapter: &str) -> Vec<Sentence> {
        let prose = self.prose_of(chapter);
        let mut sentences = Vec::new();
        for element in Self::elements(&prose) {
            sentences.extend(Self::split(&Self::plain(&element)).iter().map(|text| Sentence::new(text)));
        }
        sentences
    }

    /// The chapter's HTML with its code blocks removed.
    fn prose_of(&self, chapter: &str) -> String {
        let position = self.chapters.iter().position(|(id, _)| id == chapter).unwrap_or_else(|| panic!("the Book has no chapter `{chapter}`"));
        let start = self.chapters[position].1;
        let end = self.chapters.get(position + 1).map_or(self.html.len(), |(_, next)| *next);
        let mut rest = &self.html[start..end];
        let mut prose = String::new();
        while let Some(open) = rest.find("<pre") {
            prose.push_str(&rest[..open]);
            rest = rest[open..].split_once("</pre>").map_or("", |(_, after)| after);
        }
        prose.push_str(rest);
        prose
    }

    /// The inner HTML of every `<p>`, `<li>` and `<td>` element of `html`, in order.
    fn elements(html: &str) -> Vec<String> {
        let mut found = Vec::new();
        let mut rest = html;
        while let Some((name, open)) = ["p", "li", "td"].iter().filter_map(|name| Self::opening(rest, name).map(|at| (*name, at))).min_by_key(|(_, at)| *at) {
            let Some((_, inner)) = rest[open..].split_once('>') else { break };
            let Some((content, after)) = inner.split_once(&format!("</{name}>")) else { break };
            found.push(content.to_string());
            rest = after;
        }
        found
    }

    /// The offset of the first `<name>` or `<name ...>` tag of `html`.
    fn opening(html: &str, name: &str) -> Option<usize> {
        let mut from = 0;
        while let Some(found) = html[from..].find(&format!("<{name}")) {
            let at = from + found;
            if matches!(html[at + 1 + name.len()..].chars().next(), Some('>' | ' ')) {
                return Some(at);
            }
            from = at + 1;
        }
        None
    }

    /// `html` with its tags removed, entities decoded and whitespace collapsed.
    fn plain(html: &str) -> String {
        let mut text = String::new();
        let mut in_tag = false;
        for character in html.chars() {
            match character {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => text.push(character),
                _ => {}
            }
        }
        Self::decode(&text).split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// `text` cut after each sentence end: `.`, `?` or `!` and a blank, when what follows starts
    /// with a capital letter or a digit and the word before is not an abbreviation (`e.g.`).
    fn split(text: &str) -> Vec<String> {
        let characters: Vec<char> = text.chars().collect();
        let mut sentences = Vec::new();
        let mut start = 0;
        for (at, character) in characters.iter().enumerate() {
            let ends = matches!(character, '.' | '?' | '!') && characters.get(at + 1) == Some(&' ') && characters.get(at + 2).is_some_and(|next| next.is_uppercase() || next.is_ascii_digit());
            if ends && !Self::abbreviation(&characters[start..=at]) {
                sentences.push(characters[start..=at].iter().collect::<String>());
                start = at + 2;
            }
        }
        if start < characters.len() {
            sentences.push(characters[start..].iter().collect());
        }
        sentences
    }

    /// Whether the text ends in an abbreviation that does not end a sentence.
    fn abbreviation(text: &[char]) -> bool {
        let text: String = text.iter().collect();
        ["e.g.", "i.e.", "vs.", "cf."].iter().any(|abbreviation| text.ends_with(abbreviation))
    }
}
