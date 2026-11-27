//! The refusal table: every form the front end refuses by name, grouped by family, with the words a user reads and who
//! owns closing it. Generated once from the sites that refused in free text; a new form is one variant, one row (in the same order) of
//! `ROWS` and one entry of `ALL`.

use super::{RefusalOwner, RefusalRow, RefusedForm};

/// The value forms the front end refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueForm {
    /// array expression
    ArrayExpression,
    /// an array of `{}`
    ArrayOf,
    /// an array of carriers as a function parameter
    ArrayOfCarriersAsFunctionParameter,
    /// an array of carriers as a `let` type
    ArrayOfCarriersAsLetType,
    /// array repeat expression
    ArrayRepeatExpression,
    /// array type
    ArrayType,
    /// `as` cast
    AsCast,
    /// {} as a struct expression
    AsStructExpression,
    /// assignment
    Assignment,
    /// behavior block
    BehaviorBlock,
    /// binary expression
    BinaryExpression,
    /// block expression
    BlockExpression,
    /// `break`
    Break,
    /// built-in `{}`
    BuiltIn,
    /// call
    Call,
    /// a call of a struct or variant with named fields: write a struct expression `Name { .. }`
    CallOfStructOrVariantWithNamed,
    /// call of this declaration
    CallOfThisDeclaration,
    /// a call of a value whose type is unknown
    CallOfValueWhoseTypeIsUnknown,
    /// call that is not a function call
    CallThatIsNotFunctionCall,
    /// closure
    Closure,
    /// a closure that captures where a function pointer `fn(..) -> R` is expected (only a closure that captures nothing coerces to one)
    ClosureThatCapturesWhereFunctionPointerFn,
    /// commit `<-`
    Commit,
    /// comparison of values that are neither `f64`, `bool` nor an integer
    ComparisonOfValuesThatAreNeitherFloatBoolNorInteger,
    /// `const` item
    ConstItem,
    /// `const` without a value
    ConstWithoutValue,
    /// `continue`
    Continue,
    /// contribution
    Contribution,
    /// definition `=`
    Definition,
    /// `emit!` statement
    EmitStatement,
    /// empty statement
    EmptyStatement,
    /// `enum` item
    EnumItem,
    /// `enum` variant
    EnumVariant,
    /// `expect` with a message that is not a string literal
    ExpectWithMessageThatIsNotString,
    /// `?` expression
    Expression,
    /// expression statement
    ExpressionStatement,
    /// `extern` function
    ExternFunction,
    /// `extern` module
    ExternModule,
    /// field
    Field,
    /// field access
    FieldAccess,
    /// a field of type `{}` in a value type
    FieldOfTypeInValueType,
    /// `flip` type
    FlipType,
    /// `fn` item
    FnItem,
    /// `for` loop
    ForLoop,
    /// function parameter
    FunctionParameter,
    /// function type
    FunctionType,
    /// function without a body
    FunctionWithoutBody,
    /// generation `for`/`if`
    GenerationForIf,
    /// generic arguments on `{}`, which has no generic parameter
    GenericArgumentsOnWhichHasNoGeneric,
    /// generic function
    GenericFunction,
    /// generic parameter
    GenericParameter,
    /// `if` expression
    IfExpression,
    /// `if let` condition
    IfLetCondition,
    /// `impl` item
    ImplItem,
    /// `include_str!`
    IncludeStr,
    /// index expression
    IndexExpression,
    /// inferred type `_`
    InferredType,
    /// a `{}` inside an array
    InsideArray,
    /// {} inside a module
    InsideModule,
    /// a `{}` inside a tuple
    InsideTuple,
    /// a `{}` inside a tuple type
    InsideTupleType,
    /// integer type `{}`
    IntegerType,
    /// item inside a block
    ItemInsideBlock,
    /// an iterator row with an argument that is not a closure
    IteratorRowWithArgumentThatIsNot,
    /// `let ... else`
    LetElse,
    /// `let` statement
    LetStatement,
    /// `{}` literal
    Literal,
    /// literal
    LiteralExpression,
    /// `loop`
    Loop,
    /// malformed expression
    MalformedExpression,
    /// malformed item
    MalformedItem,
    /// `match` expression
    MatchExpression,
    /// `match` expression over `{}`
    MatchExpressionOver,
    /// method `{}`
    Method,
    /// method call
    MethodCall,
    /// method `{}` defined by the project (the checker knows only the stdlib's own `{}`)
    MethodDefinedByProjectCheckerKnowsOnly,
    /// method `{}` on `{}`
    MethodOn,
    /// `mod` item
    ModItem,
    /// module-body `let`
    ModuleBodyLet,
    /// `module` item
    ModuleItem,
    /// module param
    ModuleParam,
    /// module port
    ModulePort,
    /// the name of {}, which has fields, used as a value: build it with a call or a struct expression
    NameOfWhichHasFieldsUsedAs,
    /// a name whose type is unknown
    NameWhoseTypeIsUnknown,
    /// a named argument of a constructor
    NamedArgumentOfConstructor,
    /// `nature` item
    NatureItem,
    /// nature quantity
    NatureQuantity,
    /// a {} of a whole array of carriers or of a part of one: name an element
    OfWholeArrayOfCarriersOrOf,
    /// `on` reaction
    OnReaction,
    /// operator `+`
    OperatorAdd,
    /// operator `&`
    OperatorBitAnd,
    /// operator `|`
    OperatorBitOr,
    /// operator `^`
    OperatorBitXor,
    /// operator `/`
    OperatorDivide,
    /// operator `==`
    OperatorEqual,
    /// operator `>`
    OperatorGreater,
    /// operator `>=`
    OperatorGreaterOrEqual,
    /// operator `<`
    OperatorLess,
    /// operator `<=`
    OperatorLessOrEqual,
    /// operator `&&`
    OperatorLogicalAnd,
    /// operator `||`
    OperatorLogicalOr,
    /// operator `*`
    OperatorMultiply,
    /// operator `!=`
    OperatorNotEqual,
    /// operator `%`
    OperatorRemainder,
    /// operator `<<`
    OperatorShiftLeft,
    /// operator `>>`
    OperatorShiftRight,
    /// operator `-`
    OperatorSubtract,
    /// `Option` of a const argument
    OptionOfConstArgument,
    /// `Option` without one type argument
    OptionWithoutOneTypeArgument,
    /// param block on a call
    ParamBlockOnCall,
    /// parenthesized expression
    ParenthesizedExpression,
    /// path expression
    PathExpression,
    /// range expression
    RangeExpression,
    /// the range `..` without a bound
    RangeWithoutBound,
    /// `return`
    Return,
    /// `self` parameter
    SelfParameter,
    /// `signal` item
    SignalItem,
    /// `signal` statement
    SignalStatement,
    /// struct expression
    StructExpression,
    /// struct expression of a name that is no struct
    StructExpressionOfNameThatIsNo,
    /// `struct` item
    StructItem,
    /// `struct` item used as a type: `{}` has a carrier field, a bundle
    StructItemUsedAsTypeHasCarrier,
    /// `trait` item
    TraitItem,
    /// tuple expression
    TupleExpression,
    /// tuple type
    TupleType,
    /// type `{}`
    Type,
    /// this type
    TypeAgain,
    /// type
    TypeName,
    /// unary expression
    UnaryExpression,
    /// unit type
    UnitType,
    /// an unresolved name
    UnresolvedName,
    /// `use` item
    UseItem,
    /// {} used as a type
    UsedAsType,
    /// {} used as a value
    UsedAsValue,
    /// a value parameter in the `where` of a `fn`
    ValueParameterInWhereOfFn,
    /// `while` loop
    WhileLoop,
    /// `{}` with an argument that is not a closure
    WithArgumentThatIsNotClosure,
    /// `{}` with a width that is not an integer literal
    WithWidthThatIsNotIntegerLiteral,
}

impl ValueForm {
    /// Every form of the family, in declaration order.
    pub const ALL: &[ValueForm] = &[
        ValueForm::ArrayExpression,
        ValueForm::ArrayOf,
        ValueForm::ArrayOfCarriersAsFunctionParameter,
        ValueForm::ArrayOfCarriersAsLetType,
        ValueForm::ArrayRepeatExpression,
        ValueForm::ArrayType,
        ValueForm::AsCast,
        ValueForm::AsStructExpression,
        ValueForm::Assignment,
        ValueForm::BehaviorBlock,
        ValueForm::BinaryExpression,
        ValueForm::BlockExpression,
        ValueForm::Break,
        ValueForm::BuiltIn,
        ValueForm::Call,
        ValueForm::CallOfStructOrVariantWithNamed,
        ValueForm::CallOfThisDeclaration,
        ValueForm::CallOfValueWhoseTypeIsUnknown,
        ValueForm::CallThatIsNotFunctionCall,
        ValueForm::Closure,
        ValueForm::ClosureThatCapturesWhereFunctionPointerFn,
        ValueForm::Commit,
        ValueForm::ComparisonOfValuesThatAreNeitherFloatBoolNorInteger,
        ValueForm::ConstItem,
        ValueForm::ConstWithoutValue,
        ValueForm::Continue,
        ValueForm::Contribution,
        ValueForm::Definition,
        ValueForm::EmitStatement,
        ValueForm::EmptyStatement,
        ValueForm::EnumItem,
        ValueForm::EnumVariant,
        ValueForm::ExpectWithMessageThatIsNotString,
        ValueForm::Expression,
        ValueForm::ExpressionStatement,
        ValueForm::ExternFunction,
        ValueForm::ExternModule,
        ValueForm::Field,
        ValueForm::FieldAccess,
        ValueForm::FieldOfTypeInValueType,
        ValueForm::FlipType,
        ValueForm::FnItem,
        ValueForm::ForLoop,
        ValueForm::FunctionParameter,
        ValueForm::FunctionType,
        ValueForm::FunctionWithoutBody,
        ValueForm::GenerationForIf,
        ValueForm::GenericArgumentsOnWhichHasNoGeneric,
        ValueForm::GenericFunction,
        ValueForm::GenericParameter,
        ValueForm::IfExpression,
        ValueForm::IfLetCondition,
        ValueForm::ImplItem,
        ValueForm::IncludeStr,
        ValueForm::IndexExpression,
        ValueForm::InferredType,
        ValueForm::InsideArray,
        ValueForm::InsideModule,
        ValueForm::InsideTuple,
        ValueForm::InsideTupleType,
        ValueForm::IntegerType,
        ValueForm::ItemInsideBlock,
        ValueForm::IteratorRowWithArgumentThatIsNot,
        ValueForm::LetElse,
        ValueForm::LetStatement,
        ValueForm::Literal,
        ValueForm::LiteralExpression,
        ValueForm::Loop,
        ValueForm::MalformedExpression,
        ValueForm::MalformedItem,
        ValueForm::MatchExpression,
        ValueForm::MatchExpressionOver,
        ValueForm::Method,
        ValueForm::MethodCall,
        ValueForm::MethodDefinedByProjectCheckerKnowsOnly,
        ValueForm::MethodOn,
        ValueForm::ModItem,
        ValueForm::ModuleBodyLet,
        ValueForm::ModuleItem,
        ValueForm::ModuleParam,
        ValueForm::ModulePort,
        ValueForm::NameOfWhichHasFieldsUsedAs,
        ValueForm::NameWhoseTypeIsUnknown,
        ValueForm::NamedArgumentOfConstructor,
        ValueForm::NatureItem,
        ValueForm::NatureQuantity,
        ValueForm::OfWholeArrayOfCarriersOrOf,
        ValueForm::OnReaction,
        ValueForm::OperatorAdd,
        ValueForm::OperatorBitAnd,
        ValueForm::OperatorBitOr,
        ValueForm::OperatorBitXor,
        ValueForm::OperatorDivide,
        ValueForm::OperatorEqual,
        ValueForm::OperatorGreater,
        ValueForm::OperatorGreaterOrEqual,
        ValueForm::OperatorLess,
        ValueForm::OperatorLessOrEqual,
        ValueForm::OperatorLogicalAnd,
        ValueForm::OperatorLogicalOr,
        ValueForm::OperatorMultiply,
        ValueForm::OperatorNotEqual,
        ValueForm::OperatorRemainder,
        ValueForm::OperatorShiftLeft,
        ValueForm::OperatorShiftRight,
        ValueForm::OperatorSubtract,
        ValueForm::OptionOfConstArgument,
        ValueForm::OptionWithoutOneTypeArgument,
        ValueForm::ParamBlockOnCall,
        ValueForm::ParenthesizedExpression,
        ValueForm::PathExpression,
        ValueForm::RangeExpression,
        ValueForm::RangeWithoutBound,
        ValueForm::Return,
        ValueForm::SelfParameter,
        ValueForm::SignalItem,
        ValueForm::SignalStatement,
        ValueForm::StructExpression,
        ValueForm::StructExpressionOfNameThatIsNo,
        ValueForm::StructItem,
        ValueForm::StructItemUsedAsTypeHasCarrier,
        ValueForm::TraitItem,
        ValueForm::TupleExpression,
        ValueForm::TupleType,
        ValueForm::Type,
        ValueForm::TypeAgain,
        ValueForm::TypeName,
        ValueForm::UnaryExpression,
        ValueForm::UnitType,
        ValueForm::UnresolvedName,
        ValueForm::UseItem,
        ValueForm::UsedAsType,
        ValueForm::UsedAsValue,
        ValueForm::ValueParameterInWhereOfFn,
        ValueForm::WhileLoop,
        ValueForm::WithArgumentThatIsNotClosure,
        ValueForm::WithWidthThatIsNotIntegerLiteral,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::Value(ValueForm::ArrayExpression), text: "array expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ArrayOf), text: "an array of `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ArrayOfCarriersAsFunctionParameter), text: "an array of carriers as a function parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ArrayOfCarriersAsLetType), text: "an array of carriers as a `let` type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ArrayRepeatExpression), text: "array repeat expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ArrayType), text: "array type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::AsCast), text: "`as` cast", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::AsStructExpression), text: "{} as a struct expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Assignment), text: "assignment", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::BehaviorBlock), text: "behavior block", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::BinaryExpression), text: "binary expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::BlockExpression), text: "block expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Break), text: "`break`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::BuiltIn), text: "built-in `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Call), text: "call", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::CallOfStructOrVariantWithNamed), text: "a call of a struct or variant with named fields: write a struct expression `Name { .. }`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::CallOfThisDeclaration), text: "call of this declaration", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::CallOfValueWhoseTypeIsUnknown), text: "a call of a value whose type is unknown", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::CallThatIsNotFunctionCall), text: "call that is not a function call", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Closure), text: "closure", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ClosureThatCapturesWhereFunctionPointerFn), text: "a closure that captures where a function pointer `fn(..) -> R` is expected (only a closure that captures nothing coerces to one)", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Commit), text: "commit `<-`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ComparisonOfValuesThatAreNeitherFloatBoolNorInteger), text: "comparison of values that are neither `f64`, `bool` nor an integer", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ConstItem), text: "`const` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ConstWithoutValue), text: "`const` without a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Continue), text: "`continue`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Contribution), text: "contribution", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Definition), text: "definition `=`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::EmitStatement), text: "`emit!` statement", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::EmptyStatement), text: "empty statement", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::EnumItem), text: "`enum` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::EnumVariant), text: "`enum` variant", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ExpectWithMessageThatIsNotString), text: "`expect` with a message that is not a string literal", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Expression), text: "`?` expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ExpressionStatement), text: "expression statement", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ExternFunction), text: "`extern` function", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ExternModule), text: "`extern` module", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Field), text: "field", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FieldAccess), text: "field access", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FieldOfTypeInValueType), text: "a field of type `{}` in a value type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FlipType), text: "`flip` type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FnItem), text: "`fn` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ForLoop), text: "`for` loop", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FunctionParameter), text: "function parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FunctionType), text: "function type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FunctionWithoutBody), text: "function without a body", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::GenerationForIf), text: "generation `for`/`if`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::GenericArgumentsOnWhichHasNoGeneric), text: "generic arguments on `{}`, which has no generic parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::GenericFunction), text: "generic function", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::GenericParameter), text: "generic parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IfExpression), text: "`if` expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IfLetCondition), text: "`if let` condition", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ImplItem), text: "`impl` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IncludeStr), text: "`include_str!`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IndexExpression), text: "index expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::InferredType), text: "inferred type `_`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::InsideArray), text: "a `{}` inside an array", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::InsideModule), text: "{} inside a module", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::InsideTuple), text: "a `{}` inside a tuple", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::InsideTupleType), text: "a `{}` inside a tuple type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IntegerType), text: "integer type `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ItemInsideBlock), text: "item inside a block", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IteratorRowWithArgumentThatIsNot), text: "an iterator row with an argument that is not a closure", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::LetElse), text: "`let ... else`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::LetStatement), text: "`let` statement", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Literal), text: "`{}` literal", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::LiteralExpression), text: "literal", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Loop), text: "`loop`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MalformedExpression), text: "malformed expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MalformedItem), text: "malformed item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MatchExpression), text: "`match` expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MatchExpressionOver), text: "`match` expression over `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Method), text: "method `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MethodCall), text: "method call", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MethodDefinedByProjectCheckerKnowsOnly), text: "method `{}` defined by the project (the checker knows only the stdlib's own `{}`)", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MethodOn), text: "method `{}` on `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ModItem), text: "`mod` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ModuleBodyLet), text: "module-body `let`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ModuleItem), text: "`module` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ModuleParam), text: "module param", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ModulePort), text: "module port", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::NameOfWhichHasFieldsUsedAs), text: "the name of {}, which has fields, used as a value: build it with a call or a struct expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::NameWhoseTypeIsUnknown), text: "a name whose type is unknown", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::NamedArgumentOfConstructor), text: "a named argument of a constructor", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::NatureItem), text: "`nature` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::NatureQuantity), text: "nature quantity", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OfWholeArrayOfCarriersOrOf), text: "a {} of a whole array of carriers or of a part of one: name an element", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OnReaction), text: "`on` reaction", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorAdd), text: "operator `+`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorBitAnd), text: "operator `&`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorBitOr), text: "operator `|`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorBitXor), text: "operator `^`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorDivide), text: "operator `/`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorEqual), text: "operator `==`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorGreater), text: "operator `>`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorGreaterOrEqual), text: "operator `>=`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorLess), text: "operator `<`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorLessOrEqual), text: "operator `<=`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorLogicalAnd), text: "operator `&&`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorLogicalOr), text: "operator `||`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorMultiply), text: "operator `*`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorNotEqual), text: "operator `!=`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorRemainder), text: "operator `%`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorShiftLeft), text: "operator `<<`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorShiftRight), text: "operator `>>`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OperatorSubtract), text: "operator `-`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OptionOfConstArgument), text: "`Option` of a const argument", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OptionWithoutOneTypeArgument), text: "`Option` without one type argument", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ParamBlockOnCall), text: "param block on a call", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ParenthesizedExpression), text: "parenthesized expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::PathExpression), text: "path expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::RangeExpression), text: "range expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::RangeWithoutBound), text: "the range `..` without a bound", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Return), text: "`return`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::SelfParameter), text: "`self` parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::SignalItem), text: "`signal` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::SignalStatement), text: "`signal` statement", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::StructExpression), text: "struct expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::StructExpressionOfNameThatIsNo), text: "struct expression of a name that is no struct", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::StructItem), text: "`struct` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::StructItemUsedAsTypeHasCarrier), text: "`struct` item used as a type: `{}` has a carrier field, a bundle", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TraitItem), text: "`trait` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TupleExpression), text: "tuple expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TupleType), text: "tuple type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Type), text: "type `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TypeAgain), text: "this type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TypeName), text: "type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::UnaryExpression), text: "unary expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::UnitType), text: "unit type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::UnresolvedName), text: "an unresolved name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::UseItem), text: "`use` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::UsedAsType), text: "{} used as a type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::UsedAsValue), text: "{} used as a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ValueParameterInWhereOfFn), text: "a value parameter in the `where` of a `fn`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::WhileLoop), text: "`while` loop", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::WithArgumentThatIsNotClosure), text: "`{}` with an argument that is not a closure", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::WithWidthThatIsNotIntegerLiteral), text: "`{}` with a width that is not an integer literal", owner: RefusalOwner::Unswept },
    ];

    /// The row of the form: its words and its owner.
    pub fn row(self) -> &'static RefusalRow {
        &Self::ROWS[self as usize]
    }
}

impl From<ValueForm> for RefusedForm {
    fn from(form: ValueForm) -> RefusedForm {
        RefusedForm::Value(form)
    }
}

/// The pattern forms the front end refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternForm {
    /// array pattern
    ArrayPattern,
    /// {} as a pattern
    AsPattern,
    /// binding
    BindingPattern,
    /// a `let` with a pattern and no value
    LetWithPatternAndNoValue,
    /// literal pattern
    LiteralPattern,
    /// this name as a pattern
    NameAsPattern,
    /// or-pattern
    OrPattern,
    /// path pattern
    PathPattern,
    /// `@` pattern
    Pattern,
    /// a pattern naming something that is no struct or variant
    PatternNamingSomethingThatIsNoStruct,
    /// range pattern
    RangePattern,
    /// rest pattern
    RestPattern,
    /// struct pattern
    StructPattern,
    /// tuple pattern
    TuplePattern,
    /// tuple-struct pattern
    TupleStructPattern,
    /// wildcard pattern
    WildcardPattern,
}

impl PatternForm {
    /// Every form of the family, in declaration order.
    pub const ALL: &[PatternForm] = &[
        PatternForm::ArrayPattern,
        PatternForm::AsPattern,
        PatternForm::BindingPattern,
        PatternForm::LetWithPatternAndNoValue,
        PatternForm::LiteralPattern,
        PatternForm::NameAsPattern,
        PatternForm::OrPattern,
        PatternForm::PathPattern,
        PatternForm::Pattern,
        PatternForm::PatternNamingSomethingThatIsNoStruct,
        PatternForm::RangePattern,
        PatternForm::RestPattern,
        PatternForm::StructPattern,
        PatternForm::TuplePattern,
        PatternForm::TupleStructPattern,
        PatternForm::WildcardPattern,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::Pattern(PatternForm::ArrayPattern), text: "array pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::AsPattern), text: "{} as a pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::BindingPattern), text: "binding", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::LetWithPatternAndNoValue), text: "a `let` with a pattern and no value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::LiteralPattern), text: "literal pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::NameAsPattern), text: "this name as a pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::OrPattern), text: "or-pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::PathPattern), text: "path pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::Pattern), text: "`@` pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::PatternNamingSomethingThatIsNoStruct), text: "a pattern naming something that is no struct or variant", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::RangePattern), text: "range pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::RestPattern), text: "rest pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::StructPattern), text: "struct pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::TuplePattern), text: "tuple pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::TupleStructPattern), text: "tuple-struct pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::WildcardPattern), text: "wildcard pattern", owner: RefusalOwner::Unswept },
    ];

    /// The row of the form: its words and its owner.
    pub fn row(self) -> &'static RefusalRow {
        &Self::ROWS[self as usize]
    }
}

impl From<PatternForm> for RefusedForm {
    fn from(form: PatternForm) -> RefusedForm {
        RefusedForm::Pattern(form)
    }
}

/// The analog forms the front end refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalogForm {
    /// `analysis` of a kind other than `Static`, `Dc`, `Transient`, `Ac` or `Noise`
    AnalysisOfKindOtherThanStaticDc,
    /// commit `<-` outside a reaction body
    CommitOutsideReactionBody,
    /// contribution outside an `analog` block
    ContributionOutsideAnalogBlock,
    /// flow probe read as a value
    FlowProbeReadAsValue,
    /// `logic` of a pattern that is no literal where no `Logic<N>` is expected
    LogicOfPatternThatIsNoLiteral,
    /// `logic` without the standard library's `Logic`
    LogicWithoutStandardLibraryLogic,
    /// method `{}` on `Bit<{}>`: it is the edge of a `Bit` of `bool` or of a `Logic` of one level
    MethodOnBitItIsEdgeOf,
    /// method `{}` on this receiver
    MethodOnThisReceiver,
    /// probe through an access that names several natures
    ProbeThroughAccessThatNamesSeveralNatures,
    /// probe through this name
    ProbeThroughThisName,
    /// probe through an unresolved name
    ProbeThroughUnresolvedName,
    /// {} `{}`
    Refused,
    /// `{}` whose result type is not written (the array type is the `const`'s, §22.6.4 rule 2)
    WhoseResultTypeIsNotWrittenArray,
}

impl AnalogForm {
    /// Every form of the family, in declaration order.
    pub const ALL: &[AnalogForm] = &[
        AnalogForm::AnalysisOfKindOtherThanStaticDc,
        AnalogForm::CommitOutsideReactionBody,
        AnalogForm::ContributionOutsideAnalogBlock,
        AnalogForm::FlowProbeReadAsValue,
        AnalogForm::LogicOfPatternThatIsNoLiteral,
        AnalogForm::LogicWithoutStandardLibraryLogic,
        AnalogForm::MethodOnBitItIsEdgeOf,
        AnalogForm::MethodOnThisReceiver,
        AnalogForm::ProbeThroughAccessThatNamesSeveralNatures,
        AnalogForm::ProbeThroughThisName,
        AnalogForm::ProbeThroughUnresolvedName,
        AnalogForm::Refused,
        AnalogForm::WhoseResultTypeIsNotWrittenArray,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::Analog(AnalogForm::AnalysisOfKindOtherThanStaticDc), text: "`analysis` of a kind other than `Static`, `Dc`, `Transient`, `Ac` or `Noise`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::CommitOutsideReactionBody), text: "commit `<-` outside a reaction body", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ContributionOutsideAnalogBlock), text: "contribution outside an `analog` block", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::FlowProbeReadAsValue), text: "flow probe read as a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::LogicOfPatternThatIsNoLiteral), text: "`logic` of a pattern that is no literal where no `Logic<N>` is expected", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::LogicWithoutStandardLibraryLogic), text: "`logic` without the standard library's `Logic`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::MethodOnBitItIsEdgeOf), text: "method `{}` on `Bit<{}>`: it is the edge of a `Bit` of `bool` or of a `Logic` of one level", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::MethodOnThisReceiver), text: "method `{}` on this receiver", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ProbeThroughAccessThatNamesSeveralNatures), text: "probe through an access that names several natures", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ProbeThroughThisName), text: "probe through this name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ProbeThroughUnresolvedName), text: "probe through an unresolved name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::Refused), text: "{} `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::WhoseResultTypeIsNotWrittenArray), text: "`{}` whose result type is not written (the array type is the `const`'s, §22.6.4 rule 2)", owner: RefusalOwner::Unswept },
    ];

    /// The row of the form: its words and its owner.
    pub fn row(self) -> &'static RefusalRow {
        &Self::ROWS[self as usize]
    }
}

impl From<AnalogForm> for RefusedForm {
    fn from(form: AnalogForm) -> RefusedForm {
        RefusedForm::Analog(form)
    }
}

/// The digital forms the front end refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigitalForm {
    /// a carrier of the single-variant enum `{}` (its tag has zero levels)
    CarrierOfSingleVariantEnumItsTag,
    /// method `others` on this receiver
    MethodOthersOnThisReceiver,
    /// method `prev` on this receiver
    MethodPrevOnThisReceiver,
    /// `{}` of a const argument
    OfConstArgument,
    /// value input port of type `{}`
    ValueInputPortOfType,
    /// `{}` with {} generic arguments
    WithGenericArguments,
    /// `{}` with these generic arguments
    WithTheseGenericArguments,
    /// `{}` without its type argument
    WithoutItsTypeArgument,
}

impl DigitalForm {
    /// Every form of the family, in declaration order.
    pub const ALL: &[DigitalForm] = &[
        DigitalForm::CarrierOfSingleVariantEnumItsTag,
        DigitalForm::MethodOthersOnThisReceiver,
        DigitalForm::MethodPrevOnThisReceiver,
        DigitalForm::OfConstArgument,
        DigitalForm::ValueInputPortOfType,
        DigitalForm::WithGenericArguments,
        DigitalForm::WithTheseGenericArguments,
        DigitalForm::WithoutItsTypeArgument,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::Digital(DigitalForm::CarrierOfSingleVariantEnumItsTag), text: "a carrier of the single-variant enum `{}` (its tag has zero levels)", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::MethodOthersOnThisReceiver), text: "method `others` on this receiver", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::MethodPrevOnThisReceiver), text: "method `prev` on this receiver", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::OfConstArgument), text: "`{}` of a const argument", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::ValueInputPortOfType), text: "value input port of type `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::WithGenericArguments), text: "`{}` with {} generic arguments", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::WithTheseGenericArguments), text: "`{}` with these generic arguments", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::WithoutItsTypeArgument), text: "`{}` without its type argument", owner: RefusalOwner::Unswept },
    ];

    /// The row of the form: its words and its owner.
    pub fn row(self) -> &'static RefusalRow {
        &Self::ROWS[self as usize]
    }
}

impl From<DigitalForm> for RefusedForm {
    fn from(form: DigitalForm) -> RefusedForm {
        RefusedForm::Digital(form)
    }
}

/// The structure forms the front end refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureForm {
    /// the feedback of `{}` through an instance whose value is not a placement of a module with a result
    FeedbackOfThroughInstanceWhoseValueIs,
    /// a `let` of a child whose type is not known
    LetOfChildWhoseTypeIsNot,
    /// module result
    ModuleResult,
    /// param of type `{}`
    ParamOfType,
    /// port default value
    PortDefaultValue,
    /// a string literal bound by a `let`
    StringLiteralBoundByLet,
}

impl StructureForm {
    /// Every form of the family, in declaration order.
    pub const ALL: &[StructureForm] = &[
        StructureForm::FeedbackOfThroughInstanceWhoseValueIs,
        StructureForm::LetOfChildWhoseTypeIsNot,
        StructureForm::ModuleResult,
        StructureForm::ParamOfType,
        StructureForm::PortDefaultValue,
        StructureForm::StringLiteralBoundByLet,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::Structure(StructureForm::FeedbackOfThroughInstanceWhoseValueIs), text: "the feedback of `{}` through an instance whose value is not a placement of a module with a result", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::LetOfChildWhoseTypeIsNot), text: "a `let` of a child whose type is not known", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::ModuleResult), text: "module result", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::ParamOfType), text: "param of type `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::PortDefaultValue), text: "port default value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::StringLiteralBoundByLet), text: "a string literal bound by a `let`", owner: RefusalOwner::Unswept },
    ];

    /// The row of the form: its words and its owner.
    pub fn row(self) -> &'static RefusalRow {
        &Self::ROWS[self as usize]
    }
}

impl From<StructureForm> for RefusedForm {
    fn from(form: StructureForm) -> RefusedForm {
        RefusedForm::Structure(form)
    }
}

/// The generics forms the front end refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenericsForm {
    /// a call of a trait's function with no `self`
    CallOfTraitFunctionWithNoReceiver,
    /// a discriminant of `{}` that is not an integer literal, a `const` or integer arithmetic of them
    DiscriminantOfThatIsNotIntegerLiteral,
    /// `Option::default()` without the payload type written as `Option::<T>::default()`
    OptionDefaultWithoutPayloadTypeWrittenAs,
    /// `#[repr(..)]` other than `#[repr(uN)]` with `N` from 1 to 64
    ReprOtherThanSizedInteger,
    /// `Self` outside an `impl` or a trait
    SelfOutsideImplOrTrait,
    /// the template parameter `{}` of `{}`, bounded by `Fn`
    TemplateParameterOfBoundedByFn,
}

impl GenericsForm {
    /// Every form of the family, in declaration order.
    pub const ALL: &[GenericsForm] = &[
        GenericsForm::CallOfTraitFunctionWithNoReceiver,
        GenericsForm::DiscriminantOfThatIsNotIntegerLiteral,
        GenericsForm::OptionDefaultWithoutPayloadTypeWrittenAs,
        GenericsForm::ReprOtherThanSizedInteger,
        GenericsForm::SelfOutsideImplOrTrait,
        GenericsForm::TemplateParameterOfBoundedByFn,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::Generics(GenericsForm::CallOfTraitFunctionWithNoReceiver), text: "a call of a trait's function with no `self`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Generics(GenericsForm::DiscriminantOfThatIsNotIntegerLiteral), text: "a discriminant of `{}` that is not an integer literal, a `const` or integer arithmetic of them", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Generics(GenericsForm::OptionDefaultWithoutPayloadTypeWrittenAs), text: "`Option::default()` without the payload type written as `Option::<T>::default()`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Generics(GenericsForm::ReprOtherThanSizedInteger), text: "`#[repr(..)]` other than `#[repr(uN)]` with `N` from 1 to 64", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Generics(GenericsForm::SelfOutsideImplOrTrait), text: "`Self` outside an `impl` or a trait", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Generics(GenericsForm::TemplateParameterOfBoundedByFn), text: "the template parameter `{}` of `{}`, bounded by `Fn`", owner: RefusalOwner::Unswept },
    ];

    /// The row of the form: its words and its owner.
    pub fn row(self) -> &'static RefusalRow {
        &Self::ROWS[self as usize]
    }
}

impl From<GenericsForm> for RefusedForm {
    fn from(form: GenericsForm) -> RefusedForm {
        RefusedForm::Generics(form)
    }
}

/// The constant forms the front end refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstantForm {
}

impl ConstantForm {
    /// Every form of the family, in declaration order.
    pub const ALL: &[ConstantForm] = &[
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
    ];

    /// The row of the form: its words and its owner.
    pub fn row(self) -> &'static RefusalRow {
        &Self::ROWS[self as usize]
    }
}

impl From<ConstantForm> for RefusedForm {
    fn from(form: ConstantForm) -> RefusedForm {
        RefusedForm::Constant(form)
    }
}
