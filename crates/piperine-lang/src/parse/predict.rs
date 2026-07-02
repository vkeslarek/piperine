use super::lexer::Tok;

#[derive(Debug, Clone, PartialEq)]
pub enum ExpectedSyntax {
    Punctuation(Tok),
    Keyword(String),
    Ident(IdentRole),
    Expression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentRole {
    TypeName,
    PortName,
    ModName,
    DisciplineName,
    BundleName,
    EnumName,
    CapabilityName,
    VariableName,
    FieldName,
    ConstantName,
}
