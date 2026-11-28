//! The MSL front end of text: the lexer against the lexical appendix, the grammar parser and its error codes, the printer and the formatter, and the corpus of every spec fence, standard-library file and example through lex, parse, print and format. Each former target is a module of this one.


mod corpus;
mod errors;
mod format;
mod lexer;
mod parser;
mod print;
