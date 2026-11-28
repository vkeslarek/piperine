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
    /// an array that is not an array value
    ArrayThatIsNotArrayValue,
    /// array type
    ArrayType,
    /// `as` cast
    AsCast,
    /// {} as a struct expression
    AsStructExpression,
    /// assignment
    Assignment,
    /// an assignment to a field of a type that has none
    AssignmentToFieldOfTypeThatHas,
    /// an assignment to a field or an element
    AssignmentToFieldOrElement,
    /// an assignment to a local that is not mutable here
    AssignmentToLocalThatIsNotMutable,
    /// an assignment to a name that is not a local
    AssignmentToNameThatIsNotLocal,
    /// an assignment to a place whose type has no value in the IR
    AssignmentToPlaceWhoseTypeHasNo,
    /// an assignment to a range index
    AssignmentToRangeIndex,
    /// binary expression
    BinaryExpression,
    /// block expression
    BlockExpression,
    /// `break`
    Break,
    /// a `break`, `continue` or `return` outside the loop or function it leaves in {}
    BreakContinueOrReturnOutsideLoopOr,
    /// a `break` outside a loop
    BreakOutsideLoop,
    /// built-in `{}`
    BuiltIn,
    /// a built-in name used as a value
    BuiltInNameUsedAsValue,
    /// call
    Call,
    /// a call of a function with no body (an `extern` function) that must be inlined
    CallOfFunctionWithNoBodyExtern,
    /// a call of the generic `extern` function `{}`
    CallOfGenericExternFunction,
    /// a call of a struct or variant with named fields: write a struct expression `Name { .. }`
    CallOfStructOrVariantWithNamed,
    /// call of this declaration
    CallOfThisDeclaration,
    /// a call of a value that is not a function
    CallOfValueThatIsNotFunction,
    /// a call of a value whose type is unknown
    CallOfValueWhoseTypeIsUnknown,
    /// call that is not a function call
    CallThatIsNotFunctionCall,
    /// a call the type checker did not resolve
    CallTypeCheckerDidNotResolve,
    /// a call whose arguments the type checker did not bind
    CallWhoseArgumentsTypeCheckerDidNot,
    /// a callee whose function value has no function type once lowered
    CalleeWhoseFunctionValueHasNoFunction,
    /// a carrier of a child read by its parent that the child's body could not place where the parent expects it
    CarrierOfChildReadByItsParent,
    /// closure
    Closure,
    /// a closure argument that is not a closure literal
    ClosureArgumentThatIsNotClosureLiteral,
    /// a closure of more than one parameter
    ClosureOfMoreThanOneParameter,
    /// a closure parameter that is not a name
    ClosureParameterThatIsNotName,
    /// a closure that captures where a function pointer `fn(..) -> R` is expected (only a closure that captures nothing coerces to one)
    ClosureThatCapturesWhereFunctionPointerFn,
    /// a closure with another number of parameters than the row gives it
    ClosureWithAnotherNumberOfParametersThan,
    /// commit `<-`
    Commit,
    /// comparison of values that are neither `f64`, `bool` nor an integer
    ComparisonOfValuesThatAreNeitherFloatBoolNorInteger,
    /// `const` without a value
    ConstWithoutValue,
    /// a `const` without a value in {}
    ConstWithoutValueIn,
    /// `continue`
    Continue,
    /// contribution
    Contribution,
    /// a deferred `let` of a type with no zero in the IR
    DeferredLetOfTypeWithNoZero,
    /// a deferred `let` whose pattern is not a name
    DeferredLetWhosePatternIsNotName,
    /// a drive of a child's carrier that is not of a resolved nature
    DriveOfChildCarrierThatIsNot,
    /// `emit!` statement
    EmitStatement,
    /// empty statement
    EmptyStatement,
    /// an enum of one variant, whose tag has no level,
    EnumOfOneVariantWhoseTagHas,
    /// `expect` with a message that is not a string literal
    ExpectWithMessageThatIsNotString,
    /// expression statement
    ExpressionStatement,
    /// an expression the type checker gave no value type
    ExpressionTypeCheckerGaveNoValueType,
    /// `extern` function
    ExternFunction,
    /// `extern` module
    ExternModule,
    /// field access
    FieldAccess,
    /// a field access that is not an item of a placed module
    FieldAccessThatIsNotItemOf,
    /// a field of a child of a child
    FieldOfChildOfChild,
    /// a field of an element of an array of bundles in this position
    FieldOfElementOfArrayOfBundles,
    /// a field of something that is not a name bound to a placement
    FieldOfSomethingThatIsNotName,
    /// a field of type `{}` in a value type
    FieldOfTypeInValueType,
    /// `flip` type
    FlipType,
    /// `for` loop
    ForLoop,
    /// a `for` over anything but an array, a range or an iterator chain
    ForOverAnythingButArrayRangeOr,
    /// a `for` pattern that is not a name
    ForPatternThatIsNotName,
    /// this form of an `Option` value
    FormOfOptionValue,
    /// a function returning an `Option` whose payload has no value in the IR
    FunctionReturningOptionWhosePayloadHasNo,
    /// a function returning an `Option` with no tail value
    FunctionReturningOptionWithNoTailValue,
    /// function type
    FunctionType,
    /// a function with an `Option` parameter or result used as a value
    FunctionWithOptionParameterOrResultUsed,
    /// function without a body
    FunctionWithoutBody,
    /// generic arguments on `{}`, which has no generic parameter
    GenericArgumentsOnWhichHasNoGeneric,
    /// generic function
    GenericFunction,
    /// a generic parameter read as a value
    GenericParameterReadAsValue,
    /// `if` expression
    IfExpression,
    /// `if let` condition
    IfLetCondition,
    /// `include_str!`
    IncludeStr,
    /// index expression
    IndexExpression,
    /// an index of a value that is not an array
    IndexOfValueThatIsNotArray,
    /// inferred type `_`
    InferredType,
    /// `inflow()` of a port of a module that is placed in an expression
    InflowOfPortOfModuleThatIs,
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
    /// `is_empty` of a range with a missing bound
    IsEmptyOfRangeWithMissingBound,
    /// item inside a block
    ItemInsideBlock,
    /// an item of a child placed in generation, read without naming its iteration: index every loop
    ItemOfChildPlacedInGenerationRead,
    /// an iterator consumer with other arguments than the row takes
    IteratorConsumerWithOtherArgumentsThanRow,
    /// an iterator method outside Appendix H.7
    IteratorMethodOutsideAppendix,
    /// an iterator row with an argument that is not a closure
    IteratorRowWithArgumentThatIsNot,
    /// an iterator that is not a chain written where it is used or bound by a `let`
    IteratorThatIsNotChainWrittenWhere,
    /// a length or a bound that is not an integer constant
    LengthOrBoundThatIsNotInteger,
    /// `let ... else`
    LetElse,
    /// a `let` pattern that is not a name
    LetPatternThatIsNotName,
    /// `let` statement
    LetStatement,
    /// `{}` literal
    Literal,
    /// literal
    LiteralExpression,
    /// `loop`
    Loop,
    /// a function without a body
    LoweredFunctionWithoutBody,
    /// a method call
    LoweredMethodCall,
    /// malformed expression
    MalformedExpression,
    /// `match` expression
    MatchExpression,
    /// `match` expression over `{}`
    MatchExpressionOver,
    /// a `match` whose last arm has a guard
    MatchWhoseLastArmHasGuard,
    /// method `{}`
    Method,
    /// method call
    MethodCall,
    /// method `{}` defined by the project (the checker knows only the stdlib's own `{}`)
    MethodDefinedByProjectCheckerKnowsOnly,
    /// method `{}` on `{}`
    MethodOn,
    /// a method on a nature that returns an `Option`
    MethodOnNatureThatReturnsOption,
    /// a method outside the catalogue
    MethodOutsideCatalogue,
    /// this name as a value
    NameAsValue,
    /// a name of a child that is not one of its nodes, as a node
    NameOfChildThatIsNotOne,
    /// the name of {}, which has fields, used as a value: build it with a call or a struct expression
    NameOfWhichHasFieldsUsedAs,
    /// a name whose type is not a value type
    NameWhoseTypeIsNotValueType,
    /// a name whose type is unknown
    NameWhoseTypeIsUnknown,
    /// a name with no value here
    NameWithNoValueHere,
    /// a named argument of a constructor
    NamedArgumentOfConstructor,
    /// a node declared in generation of a module a parent reads through
    NodeDeclaredInGenerationOfModuleParent,
    /// a node of a child read as a value
    NodeOfChildReadAsValue,
    /// a node of a child read by its parent that the child's body could not place where the parent expects it
    NodeOfChildReadByItsParent,
    /// a node or a port read as a value
    NodeOrPortReadAsValue,
    /// a {} of a whole array of carriers or of a part of one: name an element
    OfWholeArrayOfCarriersOrOf,
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
    /// an `Option` parameter whose payload has no value in the IR
    OptionParameterWhosePayloadHasNoValue,
    /// an `Option` parameter whose payload has no zero in the IR
    OptionParameterWhosePayloadHasNoZero,
    /// an `Option` record field whose payload has no value in the IR
    OptionRecordFieldWhosePayloadHasNo,
    /// an `Option` whose payload has no value in the IR
    OptionWhosePayloadHasNoValueIn,
    /// an `Option` whose payload has no zero in the IR
    OptionWhosePayloadHasNoZeroIn,
    /// `Option` without one type argument
    OptionWithoutOneTypeArgument,
    /// `?` outside a function that returns an `Option`
    OutsideFunctionThatReturnsOption,
    /// param block on a call
    ParamBlockOnCall,
    /// a param of another module
    ParamOfAnotherModule,
    /// a param of a child that no placement value fixes
    ParamOfChildThatNoPlacementValue,
    /// parenthesized expression
    ParenthesizedExpression,
    /// a part of an array of generated instances as the receiver of a field: name an instance
    PartOfArrayOfGeneratedInstancesAs,
    /// path expression
    PathExpression,
    /// a pattern other than a name, `_` or a tuple of them here
    PatternOtherThanNameOrTupleOf,
    /// a pattern with fields that names no variant
    PatternWithFieldsThatNamesNoVariant,
    /// a placement used as a value
    PlacementUsedAsValue,
    /// {} `{}`
    PrimitiveWithNoSignatureRow,
    /// range expression
    RangeExpression,
    /// a range of anything but integers
    RangeOfAnythingButIntegers,
    /// a range that is not written where it is used or bound by a `let`
    RangeThatIsNotWrittenWhereIt,
    /// the range `..` without a bound
    RangeWithoutBound,
    /// a resolved carrier whose drive type has no value in the IR
    ResolvedCarrierWhoseDriveTypeHasNo,
    /// `return`
    Return,
    /// a `return` outside a function that returns early
    ReturnOutsideFunctionThatReturnsEarly,
    /// a `return` with no value in an inlined function that returns an `Option`
    ReturnWithNoValueInInlinedFunction,
    /// a `rev` of an iterator that was filtered
    RevOfIteratorThatWasFiltered,
    /// a `{}` row read as a value
    RowReadAsValue,
    /// a `{}` row read as a value rather than as an `Option`
    RowReadAsValueRatherThanAs,
    /// `self` parameter
    SelfParameter,
    /// `signal` statement
    SignalStatement,
    /// a `simparam` name that is not a text literal
    SimparamNameThatIsNotTextLiteral,
    /// this statement
    Statement,
    /// a `str` that is not a text after inlining
    StrThatIsNotTextAfterInlining,
    /// struct expression
    StructExpression,
    /// struct expression of a name that is no struct
    StructExpressionOfNameThatIsNo,
    /// a struct expression that names neither a struct nor a variant
    StructExpressionThatNamesNeitherStructNor,
    /// `struct` item used as a type: `{}` has a carrier field, a bundle
    StructItemUsedAsTypeHasCarrier,
    /// {} that reads a param
    ThatReadsParam,
    /// this expression
    ThisExpression,
    /// this type
    ThisType,
    /// the top module (its elaboration failed without a diagnostic)
    TopModuleItsElaborationFailedWithoutDiagnostic,
    /// `?` expression
    TryExpression,
    /// tuple expression
    TupleExpression,
    /// a tuple `let` component that is not a name
    TupleLetComponentThatIsNotName,
    /// a tuple `let` of another number of components
    TupleLetOfAnotherNumberOfComponents,
    /// a tuple `let` whose value is not a tuple expression
    TupleLetWhoseValueIsNotTuple,
    /// a tuple pattern over a value that is not a tuple
    TuplePatternOverValueThatIsNot,
    /// tuple type
    TupleType,
    /// type `{}`
    Type,
    /// type
    TypeName,
    /// a type that has no value in the IR
    TypeThatHasNoValueInIr,
    /// unary expression
    UnaryExpression,
    /// {} under a condition that binds a local
    UnderConditionThatBindsLocal,
    /// {} under an `if let`
    UnderIfLet,
    /// {} under a `match` arm with a guard
    UnderMatchArmWithGuard,
    /// unit type
    UnitType,
    /// an unresolved name
    UnresolvedName,
    /// {} used as a type
    UsedAsType,
    /// {} used as a value
    UsedAsValue,
    /// a value input left out, read as a value
    ValueInputLeftOutReadAsValue,
    /// a value parameter in the `where` of a `fn`
    ValueParameterInWhereOfFn,
    /// a value that is not an array
    ValueThatIsNotArray,
    /// a variant outside its enum
    VariantOutsideItsEnum,
    /// `?` where the payload has no zero in the IR
    WherePayloadHasNoZeroInIr,
    /// a `where` that is not a `bool`
    WhereThatIsNotBool,
    /// `while` loop
    WhileLoop,
    /// `{}` with an argument that is not a closure
    WithArgumentThatIsNotClosure,
    /// `{}` with a width that is not an integer literal
    WithWidthThatIsNotIntegerLiteral,
    /// a `zip` of an iterator that was filtered
    ZipOfIteratorThatWasFiltered,
}

impl ValueForm {
    /// Every form of the family, in declaration order.
    pub const ALL: &[ValueForm] = &[
        ValueForm::ArrayExpression,
        ValueForm::ArrayOf,
        ValueForm::ArrayOfCarriersAsFunctionParameter,
        ValueForm::ArrayOfCarriersAsLetType,
        ValueForm::ArrayRepeatExpression,
        ValueForm::ArrayThatIsNotArrayValue,
        ValueForm::ArrayType,
        ValueForm::AsCast,
        ValueForm::AsStructExpression,
        ValueForm::Assignment,
        ValueForm::AssignmentToFieldOfTypeThatHas,
        ValueForm::AssignmentToFieldOrElement,
        ValueForm::AssignmentToLocalThatIsNotMutable,
        ValueForm::AssignmentToNameThatIsNotLocal,
        ValueForm::AssignmentToPlaceWhoseTypeHasNo,
        ValueForm::AssignmentToRangeIndex,
        ValueForm::BinaryExpression,
        ValueForm::BlockExpression,
        ValueForm::Break,
        ValueForm::BreakContinueOrReturnOutsideLoopOr,
        ValueForm::BreakOutsideLoop,
        ValueForm::BuiltIn,
        ValueForm::BuiltInNameUsedAsValue,
        ValueForm::Call,
        ValueForm::CallOfFunctionWithNoBodyExtern,
        ValueForm::CallOfGenericExternFunction,
        ValueForm::CallOfStructOrVariantWithNamed,
        ValueForm::CallOfThisDeclaration,
        ValueForm::CallOfValueThatIsNotFunction,
        ValueForm::CallOfValueWhoseTypeIsUnknown,
        ValueForm::CallThatIsNotFunctionCall,
        ValueForm::CallTypeCheckerDidNotResolve,
        ValueForm::CallWhoseArgumentsTypeCheckerDidNot,
        ValueForm::CalleeWhoseFunctionValueHasNoFunction,
        ValueForm::CarrierOfChildReadByItsParent,
        ValueForm::Closure,
        ValueForm::ClosureArgumentThatIsNotClosureLiteral,
        ValueForm::ClosureOfMoreThanOneParameter,
        ValueForm::ClosureParameterThatIsNotName,
        ValueForm::ClosureThatCapturesWhereFunctionPointerFn,
        ValueForm::ClosureWithAnotherNumberOfParametersThan,
        ValueForm::Commit,
        ValueForm::ComparisonOfValuesThatAreNeitherFloatBoolNorInteger,
        ValueForm::ConstWithoutValue,
        ValueForm::ConstWithoutValueIn,
        ValueForm::Continue,
        ValueForm::Contribution,
        ValueForm::DeferredLetOfTypeWithNoZero,
        ValueForm::DeferredLetWhosePatternIsNotName,
        ValueForm::DriveOfChildCarrierThatIsNot,
        ValueForm::EmitStatement,
        ValueForm::EmptyStatement,
        ValueForm::EnumOfOneVariantWhoseTagHas,
        ValueForm::ExpectWithMessageThatIsNotString,
        ValueForm::ExpressionStatement,
        ValueForm::ExpressionTypeCheckerGaveNoValueType,
        ValueForm::ExternFunction,
        ValueForm::ExternModule,
        ValueForm::FieldAccess,
        ValueForm::FieldAccessThatIsNotItemOf,
        ValueForm::FieldOfChildOfChild,
        ValueForm::FieldOfElementOfArrayOfBundles,
        ValueForm::FieldOfSomethingThatIsNotName,
        ValueForm::FieldOfTypeInValueType,
        ValueForm::FlipType,
        ValueForm::ForLoop,
        ValueForm::ForOverAnythingButArrayRangeOr,
        ValueForm::ForPatternThatIsNotName,
        ValueForm::FormOfOptionValue,
        ValueForm::FunctionReturningOptionWhosePayloadHasNo,
        ValueForm::FunctionReturningOptionWithNoTailValue,
        ValueForm::FunctionType,
        ValueForm::FunctionWithOptionParameterOrResultUsed,
        ValueForm::FunctionWithoutBody,
        ValueForm::GenericArgumentsOnWhichHasNoGeneric,
        ValueForm::GenericFunction,
        ValueForm::GenericParameterReadAsValue,
        ValueForm::IfExpression,
        ValueForm::IfLetCondition,
        ValueForm::IncludeStr,
        ValueForm::IndexExpression,
        ValueForm::IndexOfValueThatIsNotArray,
        ValueForm::InferredType,
        ValueForm::InflowOfPortOfModuleThatIs,
        ValueForm::InsideArray,
        ValueForm::InsideModule,
        ValueForm::InsideTuple,
        ValueForm::InsideTupleType,
        ValueForm::IntegerType,
        ValueForm::IsEmptyOfRangeWithMissingBound,
        ValueForm::ItemInsideBlock,
        ValueForm::ItemOfChildPlacedInGenerationRead,
        ValueForm::IteratorConsumerWithOtherArgumentsThanRow,
        ValueForm::IteratorMethodOutsideAppendix,
        ValueForm::IteratorRowWithArgumentThatIsNot,
        ValueForm::IteratorThatIsNotChainWrittenWhere,
        ValueForm::LengthOrBoundThatIsNotInteger,
        ValueForm::LetElse,
        ValueForm::LetPatternThatIsNotName,
        ValueForm::LetStatement,
        ValueForm::Literal,
        ValueForm::LiteralExpression,
        ValueForm::Loop,
        ValueForm::LoweredFunctionWithoutBody,
        ValueForm::LoweredMethodCall,
        ValueForm::MalformedExpression,
        ValueForm::MatchExpression,
        ValueForm::MatchExpressionOver,
        ValueForm::MatchWhoseLastArmHasGuard,
        ValueForm::Method,
        ValueForm::MethodCall,
        ValueForm::MethodDefinedByProjectCheckerKnowsOnly,
        ValueForm::MethodOn,
        ValueForm::MethodOnNatureThatReturnsOption,
        ValueForm::MethodOutsideCatalogue,
        ValueForm::NameAsValue,
        ValueForm::NameOfChildThatIsNotOne,
        ValueForm::NameOfWhichHasFieldsUsedAs,
        ValueForm::NameWhoseTypeIsNotValueType,
        ValueForm::NameWhoseTypeIsUnknown,
        ValueForm::NameWithNoValueHere,
        ValueForm::NamedArgumentOfConstructor,
        ValueForm::NodeDeclaredInGenerationOfModuleParent,
        ValueForm::NodeOfChildReadAsValue,
        ValueForm::NodeOfChildReadByItsParent,
        ValueForm::NodeOrPortReadAsValue,
        ValueForm::OfWholeArrayOfCarriersOrOf,
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
        ValueForm::OptionParameterWhosePayloadHasNoValue,
        ValueForm::OptionParameterWhosePayloadHasNoZero,
        ValueForm::OptionRecordFieldWhosePayloadHasNo,
        ValueForm::OptionWhosePayloadHasNoValueIn,
        ValueForm::OptionWhosePayloadHasNoZeroIn,
        ValueForm::OptionWithoutOneTypeArgument,
        ValueForm::OutsideFunctionThatReturnsOption,
        ValueForm::ParamBlockOnCall,
        ValueForm::ParamOfAnotherModule,
        ValueForm::ParamOfChildThatNoPlacementValue,
        ValueForm::ParenthesizedExpression,
        ValueForm::PartOfArrayOfGeneratedInstancesAs,
        ValueForm::PathExpression,
        ValueForm::PatternOtherThanNameOrTupleOf,
        ValueForm::PatternWithFieldsThatNamesNoVariant,
        ValueForm::PlacementUsedAsValue,
        ValueForm::PrimitiveWithNoSignatureRow,
        ValueForm::RangeExpression,
        ValueForm::RangeOfAnythingButIntegers,
        ValueForm::RangeThatIsNotWrittenWhereIt,
        ValueForm::RangeWithoutBound,
        ValueForm::ResolvedCarrierWhoseDriveTypeHasNo,
        ValueForm::Return,
        ValueForm::ReturnOutsideFunctionThatReturnsEarly,
        ValueForm::ReturnWithNoValueInInlinedFunction,
        ValueForm::RevOfIteratorThatWasFiltered,
        ValueForm::RowReadAsValue,
        ValueForm::RowReadAsValueRatherThanAs,
        ValueForm::SelfParameter,
        ValueForm::SignalStatement,
        ValueForm::SimparamNameThatIsNotTextLiteral,
        ValueForm::Statement,
        ValueForm::StrThatIsNotTextAfterInlining,
        ValueForm::StructExpression,
        ValueForm::StructExpressionOfNameThatIsNo,
        ValueForm::StructExpressionThatNamesNeitherStructNor,
        ValueForm::StructItemUsedAsTypeHasCarrier,
        ValueForm::ThatReadsParam,
        ValueForm::ThisExpression,
        ValueForm::ThisType,
        ValueForm::TopModuleItsElaborationFailedWithoutDiagnostic,
        ValueForm::TryExpression,
        ValueForm::TupleExpression,
        ValueForm::TupleLetComponentThatIsNotName,
        ValueForm::TupleLetOfAnotherNumberOfComponents,
        ValueForm::TupleLetWhoseValueIsNotTuple,
        ValueForm::TuplePatternOverValueThatIsNot,
        ValueForm::TupleType,
        ValueForm::Type,
        ValueForm::TypeName,
        ValueForm::TypeThatHasNoValueInIr,
        ValueForm::UnaryExpression,
        ValueForm::UnderConditionThatBindsLocal,
        ValueForm::UnderIfLet,
        ValueForm::UnderMatchArmWithGuard,
        ValueForm::UnitType,
        ValueForm::UnresolvedName,
        ValueForm::UsedAsType,
        ValueForm::UsedAsValue,
        ValueForm::ValueInputLeftOutReadAsValue,
        ValueForm::ValueParameterInWhereOfFn,
        ValueForm::ValueThatIsNotArray,
        ValueForm::VariantOutsideItsEnum,
        ValueForm::WherePayloadHasNoZeroInIr,
        ValueForm::WhereThatIsNotBool,
        ValueForm::WhileLoop,
        ValueForm::WithArgumentThatIsNotClosure,
        ValueForm::WithWidthThatIsNotIntegerLiteral,
        ValueForm::ZipOfIteratorThatWasFiltered,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::Value(ValueForm::ArrayExpression), text: "array expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ArrayOf), text: "an array of `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ArrayOfCarriersAsFunctionParameter), text: "an array of carriers as a function parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ArrayOfCarriersAsLetType), text: "an array of carriers as a `let` type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ArrayRepeatExpression), text: "array repeat expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ArrayThatIsNotArrayValue), text: "an array that is not an array value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ArrayType), text: "array type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::AsCast), text: "`as` cast", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::AsStructExpression), text: "{} as a struct expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Assignment), text: "assignment", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::AssignmentToFieldOfTypeThatHas), text: "an assignment to a field of a type that has none", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::AssignmentToFieldOrElement), text: "an assignment to a field or an element", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::AssignmentToLocalThatIsNotMutable), text: "an assignment to a local that is not mutable here", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::AssignmentToNameThatIsNotLocal), text: "an assignment to a name that is not a local", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::AssignmentToPlaceWhoseTypeHasNo), text: "an assignment to a place whose type has no value in the IR", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::AssignmentToRangeIndex), text: "an assignment to a range index", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::BinaryExpression), text: "binary expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::BlockExpression), text: "block expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Break), text: "`break`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::BreakContinueOrReturnOutsideLoopOr), text: "a `break`, `continue` or `return` outside the loop or function it leaves in {}", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::BreakOutsideLoop), text: "a `break` outside a loop", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::BuiltIn), text: "built-in `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::BuiltInNameUsedAsValue), text: "a built-in name used as a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Call), text: "call", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::CallOfFunctionWithNoBodyExtern), text: "a call of a function with no body (an `extern` function) that must be inlined", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::CallOfGenericExternFunction), text: "a call of the generic `extern` function `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::CallOfStructOrVariantWithNamed), text: "a call of a struct or variant with named fields: write a struct expression `Name { .. }`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::CallOfThisDeclaration), text: "call of this declaration", owner: RefusalOwner::Gap("L83") },
        RefusalRow { form: RefusedForm::Value(ValueForm::CallOfValueThatIsNotFunction), text: "a call of a value that is not a function", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::CallOfValueWhoseTypeIsUnknown), text: "a call of a value whose type is unknown", owner: RefusalOwner::Gap("L84") },
        RefusalRow { form: RefusedForm::Value(ValueForm::CallThatIsNotFunctionCall), text: "call that is not a function call", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::CallTypeCheckerDidNotResolve), text: "a call the type checker did not resolve", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::CallWhoseArgumentsTypeCheckerDidNot), text: "a call whose arguments the type checker did not bind", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::CalleeWhoseFunctionValueHasNoFunction), text: "a callee whose function value has no function type once lowered", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::CarrierOfChildReadByItsParent), text: "a carrier of a child read by its parent that the child's body could not place where the parent expects it", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Closure), text: "closure", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ClosureArgumentThatIsNotClosureLiteral), text: "a closure argument that is not a closure literal", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ClosureOfMoreThanOneParameter), text: "a closure of more than one parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ClosureParameterThatIsNotName), text: "a closure parameter that is not a name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ClosureThatCapturesWhereFunctionPointerFn), text: "a closure that captures where a function pointer `fn(..) -> R` is expected (only a closure that captures nothing coerces to one)", owner: RefusalOwner::Gap("L85") },
        RefusalRow { form: RefusedForm::Value(ValueForm::ClosureWithAnotherNumberOfParametersThan), text: "a closure with another number of parameters than the row gives it", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Commit), text: "commit `<-`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ComparisonOfValuesThatAreNeitherFloatBoolNorInteger), text: "comparison of values that are neither `f64`, `bool` nor an integer", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ConstWithoutValue), text: "`const` without a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ConstWithoutValueIn), text: "a `const` without a value in {}", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Continue), text: "`continue`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Contribution), text: "contribution", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::DeferredLetOfTypeWithNoZero), text: "a deferred `let` of a type with no zero in the IR", owner: RefusalOwner::Gap("L84") },
        RefusalRow { form: RefusedForm::Value(ValueForm::DeferredLetWhosePatternIsNotName), text: "a deferred `let` whose pattern is not a name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::DriveOfChildCarrierThatIsNot), text: "a drive of a child's carrier that is not of a resolved nature", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::EmitStatement), text: "`emit!` statement", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::EmptyStatement), text: "empty statement", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::EnumOfOneVariantWhoseTagHas), text: "an enum of one variant, whose tag has no level,", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ExpectWithMessageThatIsNotString), text: "`expect` with a message that is not a string literal", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ExpressionStatement), text: "expression statement", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ExpressionTypeCheckerGaveNoValueType), text: "an expression the type checker gave no value type", owner: RefusalOwner::Gap("L86") },
        RefusalRow { form: RefusedForm::Value(ValueForm::ExternFunction), text: "`extern` function", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ExternModule), text: "`extern` module", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FieldAccess), text: "field access", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FieldAccessThatIsNotItemOf), text: "a field access that is not an item of a placed module", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FieldOfChildOfChild), text: "a field of a child of a child", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FieldOfElementOfArrayOfBundles), text: "a field of an element of an array of bundles in this position", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FieldOfSomethingThatIsNotName), text: "a field of something that is not a name bound to a placement", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FieldOfTypeInValueType), text: "a field of type `{}` in a value type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FlipType), text: "`flip` type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ForLoop), text: "`for` loop", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ForOverAnythingButArrayRangeOr), text: "a `for` over anything but an array, a range or an iterator chain", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ForPatternThatIsNotName), text: "a `for` pattern that is not a name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FormOfOptionValue), text: "this form of an `Option` value", owner: RefusalOwner::Gap("L92") },
        RefusalRow { form: RefusedForm::Value(ValueForm::FunctionReturningOptionWhosePayloadHasNo), text: "a function returning an `Option` whose payload has no value in the IR", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FunctionReturningOptionWithNoTailValue), text: "a function returning an `Option` with no tail value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FunctionType), text: "function type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::FunctionWithOptionParameterOrResultUsed), text: "a function with an `Option` parameter or result used as a value", owner: RefusalOwner::Gap("L86") },
        RefusalRow { form: RefusedForm::Value(ValueForm::FunctionWithoutBody), text: "function without a body", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::GenericArgumentsOnWhichHasNoGeneric), text: "generic arguments on `{}`, which has no generic parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::GenericFunction), text: "generic function", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::GenericParameterReadAsValue), text: "a generic parameter read as a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IfExpression), text: "`if` expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IfLetCondition), text: "`if let` condition", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IncludeStr), text: "`include_str!`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IndexExpression), text: "index expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IndexOfValueThatIsNotArray), text: "an index of a value that is not an array", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::InferredType), text: "inferred type `_`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::InflowOfPortOfModuleThatIs), text: "`inflow()` of a port of a module that is placed in an expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::InsideArray), text: "a `{}` inside an array", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::InsideModule), text: "{} inside a module", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::InsideTuple), text: "a `{}` inside a tuple", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::InsideTupleType), text: "a `{}` inside a tuple type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IntegerType), text: "integer type `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IsEmptyOfRangeWithMissingBound), text: "`is_empty` of a range with a missing bound", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ItemInsideBlock), text: "item inside a block", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ItemOfChildPlacedInGenerationRead), text: "an item of a child placed in generation, read without naming its iteration: index every loop", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IteratorConsumerWithOtherArgumentsThanRow), text: "an iterator consumer with other arguments than the row takes", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IteratorMethodOutsideAppendix), text: "an iterator method outside Appendix H.7", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IteratorRowWithArgumentThatIsNot), text: "an iterator row with an argument that is not a closure", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::IteratorThatIsNotChainWrittenWhere), text: "an iterator that is not a chain written where it is used or bound by a `let`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::LengthOrBoundThatIsNotInteger), text: "a length or a bound that is not an integer constant", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::LetElse), text: "`let ... else`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::LetPatternThatIsNotName), text: "a `let` pattern that is not a name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::LetStatement), text: "`let` statement", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Literal), text: "`{}` literal", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::LiteralExpression), text: "literal", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Loop), text: "`loop`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::LoweredFunctionWithoutBody), text: "a function without a body", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::LoweredMethodCall), text: "a method call", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MalformedExpression), text: "malformed expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MatchExpression), text: "`match` expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MatchExpressionOver), text: "`match` expression over `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MatchWhoseLastArmHasGuard), text: "a `match` whose last arm has a guard", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Method), text: "method `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MethodCall), text: "method call", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MethodDefinedByProjectCheckerKnowsOnly), text: "method `{}` defined by the project (the checker knows only the stdlib's own `{}`)", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MethodOn), text: "method `{}` on `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MethodOnNatureThatReturnsOption), text: "a method on a nature that returns an `Option`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::MethodOutsideCatalogue), text: "a method outside the catalogue", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::NameAsValue), text: "this name as a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::NameOfChildThatIsNotOne), text: "a name of a child that is not one of its nodes, as a node", owner: RefusalOwner::Gap("L96") },
        RefusalRow { form: RefusedForm::Value(ValueForm::NameOfWhichHasFieldsUsedAs), text: "the name of {}, which has fields, used as a value: build it with a call or a struct expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::NameWhoseTypeIsNotValueType), text: "a name whose type is not a value type", owner: RefusalOwner::Gap("L86") },
        RefusalRow { form: RefusedForm::Value(ValueForm::NameWhoseTypeIsUnknown), text: "a name whose type is unknown", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::NameWithNoValueHere), text: "a name with no value here", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::NamedArgumentOfConstructor), text: "a named argument of a constructor", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::NodeDeclaredInGenerationOfModuleParent), text: "a node declared in generation of a module a parent reads through", owner: RefusalOwner::Gap("L94") },
        RefusalRow { form: RefusedForm::Value(ValueForm::NodeOfChildReadAsValue), text: "a node of a child read as a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::NodeOfChildReadByItsParent), text: "a node of a child read by its parent that the child's body could not place where the parent expects it", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::NodeOrPortReadAsValue), text: "a node or a port read as a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OfWholeArrayOfCarriersOrOf), text: "a {} of a whole array of carriers or of a part of one: name an element", owner: RefusalOwner::Unswept },
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
        RefusalRow { form: RefusedForm::Value(ValueForm::OptionParameterWhosePayloadHasNoValue), text: "an `Option` parameter whose payload has no value in the IR", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OptionParameterWhosePayloadHasNoZero), text: "an `Option` parameter whose payload has no zero in the IR", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OptionRecordFieldWhosePayloadHasNo), text: "an `Option` record field whose payload has no value in the IR", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OptionWhosePayloadHasNoValueIn), text: "an `Option` whose payload has no value in the IR", owner: RefusalOwner::Gap("L92") },
        RefusalRow { form: RefusedForm::Value(ValueForm::OptionWhosePayloadHasNoZeroIn), text: "an `Option` whose payload has no zero in the IR", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OptionWithoutOneTypeArgument), text: "`Option` without one type argument", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::OutsideFunctionThatReturnsOption), text: "`?` outside a function that returns an `Option`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ParamBlockOnCall), text: "param block on a call", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ParamOfAnotherModule), text: "a param of another module", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ParamOfChildThatNoPlacementValue), text: "a param of a child that no placement value fixes", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ParenthesizedExpression), text: "parenthesized expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::PartOfArrayOfGeneratedInstancesAs), text: "a part of an array of generated instances as the receiver of a field: name an instance", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::PathExpression), text: "path expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::PatternOtherThanNameOrTupleOf), text: "a pattern other than a name, `_` or a tuple of them here", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::PatternWithFieldsThatNamesNoVariant), text: "a pattern with fields that names no variant", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::PlacementUsedAsValue), text: "a placement used as a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::PrimitiveWithNoSignatureRow), text: "{} `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::RangeExpression), text: "range expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::RangeOfAnythingButIntegers), text: "a range of anything but integers", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::RangeThatIsNotWrittenWhereIt), text: "a range that is not written where it is used or bound by a `let`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::RangeWithoutBound), text: "the range `..` without a bound", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ResolvedCarrierWhoseDriveTypeHasNo), text: "a resolved carrier whose drive type has no value in the IR", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Return), text: "`return`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ReturnOutsideFunctionThatReturnsEarly), text: "a `return` outside a function that returns early", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ReturnWithNoValueInInlinedFunction), text: "a `return` with no value in an inlined function that returns an `Option`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::RevOfIteratorThatWasFiltered), text: "a `rev` of an iterator that was filtered", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::RowReadAsValue), text: "a `{}` row read as a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::RowReadAsValueRatherThanAs), text: "a `{}` row read as a value rather than as an `Option`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::SelfParameter), text: "`self` parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::SignalStatement), text: "`signal` statement", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::SimparamNameThatIsNotTextLiteral), text: "a `simparam` name that is not a text literal", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Statement), text: "this statement", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::StrThatIsNotTextAfterInlining), text: "a `str` that is not a text after inlining", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::StructExpression), text: "struct expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::StructExpressionOfNameThatIsNo), text: "struct expression of a name that is no struct", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::StructExpressionThatNamesNeitherStructNor), text: "a struct expression that names neither a struct nor a variant", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::StructItemUsedAsTypeHasCarrier), text: "`struct` item used as a type: `{}` has a carrier field, a bundle", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ThatReadsParam), text: "{} that reads a param", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ThisExpression), text: "this expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ThisType), text: "this type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TopModuleItsElaborationFailedWithoutDiagnostic), text: "the top module (its elaboration failed without a diagnostic)", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TryExpression), text: "`?` expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TupleExpression), text: "tuple expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TupleLetComponentThatIsNotName), text: "a tuple `let` component that is not a name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TupleLetOfAnotherNumberOfComponents), text: "a tuple `let` of another number of components", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TupleLetWhoseValueIsNotTuple), text: "a tuple `let` whose value is not a tuple expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TuplePatternOverValueThatIsNot), text: "a tuple pattern over a value that is not a tuple", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TupleType), text: "tuple type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::Type), text: "type `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TypeName), text: "type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::TypeThatHasNoValueInIr), text: "a type that has no value in the IR", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::UnaryExpression), text: "unary expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::UnderConditionThatBindsLocal), text: "{} under a condition that binds a local", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::UnderIfLet), text: "{} under an `if let`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::UnderMatchArmWithGuard), text: "{} under a `match` arm with a guard", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::UnitType), text: "unit type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::UnresolvedName), text: "an unresolved name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::UsedAsType), text: "{} used as a type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::UsedAsValue), text: "{} used as a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ValueInputLeftOutReadAsValue), text: "a value input left out, read as a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ValueParameterInWhereOfFn), text: "a value parameter in the `where` of a `fn`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ValueThatIsNotArray), text: "a value that is not an array", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::VariantOutsideItsEnum), text: "a variant outside its enum", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::WherePayloadHasNoZeroInIr), text: "`?` where the payload has no zero in the IR", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::WhereThatIsNotBool), text: "a `where` that is not a `bool`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::WhileLoop), text: "`while` loop", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::WithArgumentThatIsNotClosure), text: "`{}` with an argument that is not a closure", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::WithWidthThatIsNotIntegerLiteral), text: "`{}` with a width that is not an integer literal", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Value(ValueForm::ZipOfIteratorThatWasFiltered), text: "a `zip` of an iterator that was filtered", owner: RefusalOwner::Unswept },
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
    /// an assignment to a local of type `Option`
    AssignmentToLocalOfTypeOption,
    /// a binding inside `Some` in a definition
    BindingInsideSomeInDefinition,
    /// binding
    BindingPattern,
    /// an `if let` of `Option` values whose block is more than its value
    IfLetOfOptionValuesWhoseBlock,
    /// a `let` with a pattern and no value
    LetWithPatternAndNoValue,
    /// literal pattern
    LiteralPattern,
    /// this name as a pattern
    NameAsPattern,
    /// a name bound by a pattern in a definition
    NameBoundByPatternInDefinition,
    /// or-pattern
    OrPattern,
    /// path pattern
    PathPattern,
    /// `@` pattern
    Pattern,
    /// this pattern inside `Some` in a definition
    PatternInsideSomeInDefinition,
    /// a pattern naming something that is no struct or variant
    PatternNamingSomethingThatIsNoStruct,
    /// this pattern of an `Option`
    PatternOfOption,
    /// a pattern with fields over a value that is not a struct, a tuple or an enum
    PatternWithFieldsOverValueThatIs,
    /// range pattern
    RangePattern,
    /// rest pattern
    RestPattern,
    /// struct pattern
    StructPattern,
    /// this pattern
    ThisPattern,
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
        PatternForm::AssignmentToLocalOfTypeOption,
        PatternForm::BindingInsideSomeInDefinition,
        PatternForm::BindingPattern,
        PatternForm::IfLetOfOptionValuesWhoseBlock,
        PatternForm::LetWithPatternAndNoValue,
        PatternForm::LiteralPattern,
        PatternForm::NameAsPattern,
        PatternForm::NameBoundByPatternInDefinition,
        PatternForm::OrPattern,
        PatternForm::PathPattern,
        PatternForm::Pattern,
        PatternForm::PatternInsideSomeInDefinition,
        PatternForm::PatternNamingSomethingThatIsNoStruct,
        PatternForm::PatternOfOption,
        PatternForm::PatternWithFieldsOverValueThatIs,
        PatternForm::RangePattern,
        PatternForm::RestPattern,
        PatternForm::StructPattern,
        PatternForm::ThisPattern,
        PatternForm::TuplePattern,
        PatternForm::TupleStructPattern,
        PatternForm::WildcardPattern,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::Pattern(PatternForm::ArrayPattern), text: "array pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::AsPattern), text: "{} as a pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::AssignmentToLocalOfTypeOption), text: "an assignment to a local of type `Option`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::BindingInsideSomeInDefinition), text: "a binding inside `Some` in a definition", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::BindingPattern), text: "binding", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::IfLetOfOptionValuesWhoseBlock), text: "an `if let` of `Option` values whose block is more than its value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::LetWithPatternAndNoValue), text: "a `let` with a pattern and no value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::LiteralPattern), text: "literal pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::NameAsPattern), text: "this name as a pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::NameBoundByPatternInDefinition), text: "a name bound by a pattern in a definition", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::OrPattern), text: "or-pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::PathPattern), text: "path pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::Pattern), text: "`@` pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::PatternInsideSomeInDefinition), text: "this pattern inside `Some` in a definition", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::PatternNamingSomethingThatIsNoStruct), text: "a pattern naming something that is no struct or variant", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::PatternOfOption), text: "this pattern of an `Option`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::PatternWithFieldsOverValueThatIs), text: "a pattern with fields over a value that is not a struct, a tuple or an enum", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::RangePattern), text: "range pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::RestPattern), text: "rest pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::StructPattern), text: "struct pattern", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Pattern(PatternForm::ThisPattern), text: "this pattern", owner: RefusalOwner::Unswept },
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
    /// an assignment to something that is no name of {}
    AssignmentToSomethingThatIsNoName,
    /// a `bound_step` inside a closure or an `Option` method
    BoundStepInsideClosureOrOptionMethod,
    /// a `bound_step` under a condition whose step binds a local
    BoundStepUnderConditionWhoseStepBinds,
    /// a `break`, a `continue` or a nested `while` in a loop of {}
    BreakContinueOrNestedWhileInLoop,
    /// a `break`, a `continue` or a `return` with no value in a `fn` called inside a `ddx`
    BreakContinueOrReturnWithNoValue,
    /// this call as an event
    CallAsEvent,
    /// a call inside a `ddx` that expands to no expression
    CallInsideDdxThatExpandsToNo,
    /// a closure or an `Option` binding inside a `ddt` state
    ClosureOrOptionBindingInsideDdtState,
    /// a closure or an `Option` binding inside a `delay` input
    ClosureOrOptionBindingInsideDelayInput,
    /// a closure or an `Option` binding inside a `noise` density
    ClosureOrOptionBindingInsideNoiseDensity,
    /// commit `<-` outside a reaction body
    CommitOutsideReactionBody,
    /// a compound assignment through an operator of a type, in {}
    CompoundAssignmentThroughOperatorOfTypeIn,
    /// contribution outside an `analog` block
    ContributionOutsideAnalogBlock,
    /// a contribution target that is not a probe
    ContributionTargetThatIsNotProbe,
    /// a contribution to a probe of more than two nodes
    ContributionToProbeOfMoreThanTwo,
    /// core primitive `{}`
    CorePrimitive,
    /// a `ddx` whose derivative reads the value of the flow of a branch with no potential contribution
    DdxWhoseDerivativeReadsValueOfFlow,
    /// an `else` that is no block or `if` in {}
    ElseThatIsNoBlockOrIf,
    /// `encode` on a value whose level count is not known where it is lowered
    EncodeOnValueWhoseLevelCountIs,
    /// an event that is not an event read
    EventThatIsNotEventRead,
    /// an expression statement that is no assignment, `if` or `for` in a `fn` called inside a `ddx`
    ExpressionStatementThatIsNoAssignmentIf,
    /// a field of an element of an array of bundles that is no node
    FieldOfElementOfArrayOfBundles,
    /// a flow probe of a branch without a potential contribution
    FlowProbeOfBranchWithoutPotentialContribution,
    /// flow probe read as a value
    FlowProbeReadAsValue,
    /// a `fn` called inside a `ddx` that forks into more than 4096 paths
    FnCalledInsideDdxThatForksInto,
    /// a `fn` with no value inside a `ddx`
    FnWithNoValueInsideDdx,
    /// a `for` of more than 4096 iterations in {}
    ForOfMoreThanIterationsIn,
    /// a `for` that is not over a range with both bounds, in {}
    ForThatIsNotOverRangeWith,
    /// a `for` whose bounds do not fold to constants, in {}
    ForWhoseBoundsDoNotFoldTo,
    /// a `for` whose variable is no name, in {}
    ForWhoseVariableIsNoNameIn,
    /// this form of an event
    FormOfEvent,
    /// {}
    FormWithNoDerivativeRule,
    /// a function of §22.1 called with the wrong number of arguments
    FunctionOfCalledWithWrongNumberOf,
    /// a generated node outside the generation that declares it, without an index
    GeneratedNodeOutsideGenerationThatDeclaresIt,
    /// a guarded `ac_stim` whose input binds a local
    GuardedAcStimWhoseInputBindsLocal,
    /// a guarded `limit` whose input binds a local
    GuardedLimitWhoseInputBindsLocal,
    /// an `if let` in a `fn` called inside a `ddx`
    IfLetInFnCalledInsideDdx,
    /// an index of a value that is no array of nodes
    IndexOfValueThatIsNoArray,
    /// a `{}` inside an `Option` method
    InsideOptionMethod,
    /// a `let` in the argument of an event
    LetInArgumentOfEvent,
    /// a `let` in the discrete part of a guard
    LetInDiscretePartOfGuard,
    /// a `let` in the guard of a head split per disjunct
    LetInGuardOfHeadSplitPer,
    /// a `let` of a pattern that is no name in {}
    LetOfPatternThatIsNoName,
    /// a level cast to a type that has no value type in the IR
    LevelCastToTypeThatHasNo,
    /// a `logic` call whose type is not a `Logic<N>` of known width
    LogicCallWhoseTypeIsNotLogic,
    /// `logic` of a pattern that is no literal where no `Logic<N>` is expected
    LogicOfPatternThatIsNoLiteral,
    /// a `matches_bits` subject that is not a `Logic<N>` of known width
    MatchesBitsSubjectThatIsNotLogic,
    /// this method as an event
    MethodAsEvent,
    /// method `{}` on `Bit<{}>`: it is the edge of a `Bit` of `bool` or of a `Logic` of one level
    MethodOnBitItIsEdgeOf,
    /// method `{}` on this receiver
    MethodOnThisReceiver,
    /// this name as an event
    NameAsEvent,
    /// a name that is not a node of this module
    NameThatIsNotNodeOfThis,
    /// nature `{}` without a potential
    NatureWithoutPotential,
    /// a node argument that is not a name
    NodeArgumentThatIsNotName,
    /// a node argument that is not a node
    NodeArgumentThatIsNotNode,
    /// a `noise` name that is not a text literal
    NoiseNameThatIsNotTextLiteral,
    /// `parse_table` or `parse_hex` outside a `const`: a run never reads a file (§20)
    ParseTableOrParseHexOutsideConst,
    /// a pattern that is not a text known where it is read
    PatternThatIsNotTextKnownWhere,
    /// a probe of more than two nodes
    ProbeOfMoreThanTwoNodes,
    /// probe through an access that names several natures
    ProbeThroughAccessThatNamesSeveralNatures,
    /// probe through this name
    ProbeThroughThisName,
    /// probe through an unresolved name
    ProbeThroughUnresolvedName,
    /// a receiver that is not a carrier of this module
    ReceiverThatIsNotCarrierOfThis,
    /// a statement that is no `let` of a name, assignment, `if` or `for` in a `fn` called inside a `ddx`
    StatementThatIsNoLetOfName,
    /// a `table_model` called with other than three arguments
    TableModelCalledWithOtherThanThree,
    /// a `table_model` table that is not an array of `(f64, f64)` pairs known at elaboration
    TableModelTableThatIsNotArray,
    /// {} that is not a constant or a param
    ThatIsNotConstantOrParam,
    /// a `while` or `loop` in a `fn` called inside a `ddx`: it has no finite symbolic reading
    WhileOrLoopInFnCalledInside,
    /// `{}` whose result type is not written (the array type is the `const`'s, §22.6.4 rule 2)
    WhoseResultTypeIsNotWrittenArray,
}

impl AnalogForm {
    /// Every form of the family, in declaration order.
    pub const ALL: &[AnalogForm] = &[
        AnalogForm::AnalysisOfKindOtherThanStaticDc,
        AnalogForm::AssignmentToSomethingThatIsNoName,
        AnalogForm::BoundStepInsideClosureOrOptionMethod,
        AnalogForm::BoundStepUnderConditionWhoseStepBinds,
        AnalogForm::BreakContinueOrNestedWhileInLoop,
        AnalogForm::BreakContinueOrReturnWithNoValue,
        AnalogForm::CallAsEvent,
        AnalogForm::CallInsideDdxThatExpandsToNo,
        AnalogForm::ClosureOrOptionBindingInsideDdtState,
        AnalogForm::ClosureOrOptionBindingInsideDelayInput,
        AnalogForm::ClosureOrOptionBindingInsideNoiseDensity,
        AnalogForm::CommitOutsideReactionBody,
        AnalogForm::CompoundAssignmentThroughOperatorOfTypeIn,
        AnalogForm::ContributionOutsideAnalogBlock,
        AnalogForm::ContributionTargetThatIsNotProbe,
        AnalogForm::ContributionToProbeOfMoreThanTwo,
        AnalogForm::CorePrimitive,
        AnalogForm::DdxWhoseDerivativeReadsValueOfFlow,
        AnalogForm::ElseThatIsNoBlockOrIf,
        AnalogForm::EncodeOnValueWhoseLevelCountIs,
        AnalogForm::EventThatIsNotEventRead,
        AnalogForm::ExpressionStatementThatIsNoAssignmentIf,
        AnalogForm::FieldOfElementOfArrayOfBundles,
        AnalogForm::FlowProbeOfBranchWithoutPotentialContribution,
        AnalogForm::FlowProbeReadAsValue,
        AnalogForm::FnCalledInsideDdxThatForksInto,
        AnalogForm::FnWithNoValueInsideDdx,
        AnalogForm::ForOfMoreThanIterationsIn,
        AnalogForm::ForThatIsNotOverRangeWith,
        AnalogForm::ForWhoseBoundsDoNotFoldTo,
        AnalogForm::ForWhoseVariableIsNoNameIn,
        AnalogForm::FormOfEvent,
        AnalogForm::FormWithNoDerivativeRule,
        AnalogForm::FunctionOfCalledWithWrongNumberOf,
        AnalogForm::GeneratedNodeOutsideGenerationThatDeclaresIt,
        AnalogForm::GuardedAcStimWhoseInputBindsLocal,
        AnalogForm::GuardedLimitWhoseInputBindsLocal,
        AnalogForm::IfLetInFnCalledInsideDdx,
        AnalogForm::IndexOfValueThatIsNoArray,
        AnalogForm::InsideOptionMethod,
        AnalogForm::LetInArgumentOfEvent,
        AnalogForm::LetInDiscretePartOfGuard,
        AnalogForm::LetInGuardOfHeadSplitPer,
        AnalogForm::LetOfPatternThatIsNoName,
        AnalogForm::LevelCastToTypeThatHasNo,
        AnalogForm::LogicCallWhoseTypeIsNotLogic,
        AnalogForm::LogicOfPatternThatIsNoLiteral,
        AnalogForm::MatchesBitsSubjectThatIsNotLogic,
        AnalogForm::MethodAsEvent,
        AnalogForm::MethodOnBitItIsEdgeOf,
        AnalogForm::MethodOnThisReceiver,
        AnalogForm::NameAsEvent,
        AnalogForm::NameThatIsNotNodeOfThis,
        AnalogForm::NatureWithoutPotential,
        AnalogForm::NodeArgumentThatIsNotName,
        AnalogForm::NodeArgumentThatIsNotNode,
        AnalogForm::NoiseNameThatIsNotTextLiteral,
        AnalogForm::ParseTableOrParseHexOutsideConst,
        AnalogForm::PatternThatIsNotTextKnownWhere,
        AnalogForm::ProbeOfMoreThanTwoNodes,
        AnalogForm::ProbeThroughAccessThatNamesSeveralNatures,
        AnalogForm::ProbeThroughThisName,
        AnalogForm::ProbeThroughUnresolvedName,
        AnalogForm::ReceiverThatIsNotCarrierOfThis,
        AnalogForm::StatementThatIsNoLetOfName,
        AnalogForm::TableModelCalledWithOtherThanThree,
        AnalogForm::TableModelTableThatIsNotArray,
        AnalogForm::ThatIsNotConstantOrParam,
        AnalogForm::WhileOrLoopInFnCalledInside,
        AnalogForm::WhoseResultTypeIsNotWrittenArray,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::Analog(AnalogForm::AnalysisOfKindOtherThanStaticDc), text: "`analysis` of a kind other than `Static`, `Dc`, `Transient`, `Ac` or `Noise`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::AssignmentToSomethingThatIsNoName), text: "an assignment to something that is no name of {}", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::BoundStepInsideClosureOrOptionMethod), text: "a `bound_step` inside a closure or an `Option` method", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::BoundStepUnderConditionWhoseStepBinds), text: "a `bound_step` under a condition whose step binds a local", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::BreakContinueOrNestedWhileInLoop), text: "a `break`, a `continue` or a nested `while` in a loop of {}", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::BreakContinueOrReturnWithNoValue), text: "a `break`, a `continue` or a `return` with no value in a `fn` called inside a `ddx`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::CallAsEvent), text: "this call as an event", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::CallInsideDdxThatExpandsToNo), text: "a call inside a `ddx` that expands to no expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ClosureOrOptionBindingInsideDdtState), text: "a closure or an `Option` binding inside a `ddt` state", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ClosureOrOptionBindingInsideDelayInput), text: "a closure or an `Option` binding inside a `delay` input", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ClosureOrOptionBindingInsideNoiseDensity), text: "a closure or an `Option` binding inside a `noise` density", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::CommitOutsideReactionBody), text: "commit `<-` outside a reaction body", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::CompoundAssignmentThroughOperatorOfTypeIn), text: "a compound assignment through an operator of a type, in {}", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ContributionOutsideAnalogBlock), text: "contribution outside an `analog` block", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ContributionTargetThatIsNotProbe), text: "a contribution target that is not a probe", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ContributionToProbeOfMoreThanTwo), text: "a contribution to a probe of more than two nodes", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::CorePrimitive), text: "core primitive `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::DdxWhoseDerivativeReadsValueOfFlow), text: "a `ddx` whose derivative reads the value of the flow of a branch with no potential contribution", owner: RefusalOwner::Gap("L73") },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ElseThatIsNoBlockOrIf), text: "an `else` that is no block or `if` in {}", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::EncodeOnValueWhoseLevelCountIs), text: "`encode` on a value whose level count is not known where it is lowered", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::EventThatIsNotEventRead), text: "an event that is not an event read", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ExpressionStatementThatIsNoAssignmentIf), text: "an expression statement that is no assignment, `if` or `for` in a `fn` called inside a `ddx`", owner: RefusalOwner::Gap("L74") },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::FieldOfElementOfArrayOfBundles), text: "a field of an element of an array of bundles that is no node", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::FlowProbeOfBranchWithoutPotentialContribution), text: "a flow probe of a branch without a potential contribution", owner: RefusalOwner::Gap("L73") },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::FlowProbeReadAsValue), text: "flow probe read as a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::FnCalledInsideDdxThatForksInto), text: "a `fn` called inside a `ddx` that forks into more than 4096 paths", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::FnWithNoValueInsideDdx), text: "a `fn` with no value inside a `ddx`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ForOfMoreThanIterationsIn), text: "a `for` of more than 4096 iterations in {}", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ForThatIsNotOverRangeWith), text: "a `for` that is not over a range with both bounds, in {}", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ForWhoseBoundsDoNotFoldTo), text: "a `for` whose bounds do not fold to constants, in {}", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ForWhoseVariableIsNoNameIn), text: "a `for` whose variable is no name, in {}", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::FormOfEvent), text: "this form of an event", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::FormWithNoDerivativeRule), text: "{}", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::FunctionOfCalledWithWrongNumberOf), text: "a function of §22.1 called with the wrong number of arguments", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::GeneratedNodeOutsideGenerationThatDeclaresIt), text: "a generated node outside the generation that declares it, without an index", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::GuardedAcStimWhoseInputBindsLocal), text: "a guarded `ac_stim` whose input binds a local", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::GuardedLimitWhoseInputBindsLocal), text: "a guarded `limit` whose input binds a local", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::IfLetInFnCalledInsideDdx), text: "an `if let` in a `fn` called inside a `ddx`", owner: RefusalOwner::Gap("L74") },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::IndexOfValueThatIsNoArray), text: "an index of a value that is no array of nodes", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::InsideOptionMethod), text: "a `{}` inside an `Option` method", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::LetInArgumentOfEvent), text: "a `let` in the argument of an event", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::LetInDiscretePartOfGuard), text: "a `let` in the discrete part of a guard", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::LetInGuardOfHeadSplitPer), text: "a `let` in the guard of a head split per disjunct", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::LetOfPatternThatIsNoName), text: "a `let` of a pattern that is no name in {}", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::LevelCastToTypeThatHasNo), text: "a level cast to a type that has no value type in the IR", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::LogicCallWhoseTypeIsNotLogic), text: "a `logic` call whose type is not a `Logic<N>` of known width", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::LogicOfPatternThatIsNoLiteral), text: "`logic` of a pattern that is no literal where no `Logic<N>` is expected", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::MatchesBitsSubjectThatIsNotLogic), text: "a `matches_bits` subject that is not a `Logic<N>` of known width", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::MethodAsEvent), text: "this method as an event", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::MethodOnBitItIsEdgeOf), text: "method `{}` on `Bit<{}>`: it is the edge of a `Bit` of `bool` or of a `Logic` of one level", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::MethodOnThisReceiver), text: "method `{}` on this receiver", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::NameAsEvent), text: "this name as an event", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::NameThatIsNotNodeOfThis), text: "a name that is not a node of this module", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::NatureWithoutPotential), text: "nature `{}` without a potential", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::NodeArgumentThatIsNotName), text: "a node argument that is not a name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::NodeArgumentThatIsNotNode), text: "a node argument that is not a node", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::NoiseNameThatIsNotTextLiteral), text: "a `noise` name that is not a text literal", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ParseTableOrParseHexOutsideConst), text: "`parse_table` or `parse_hex` outside a `const`: a run never reads a file (§20)", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::PatternThatIsNotTextKnownWhere), text: "a pattern that is not a text known where it is read", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ProbeOfMoreThanTwoNodes), text: "a probe of more than two nodes", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ProbeThroughAccessThatNamesSeveralNatures), text: "probe through an access that names several natures", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ProbeThroughThisName), text: "probe through this name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ProbeThroughUnresolvedName), text: "probe through an unresolved name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ReceiverThatIsNotCarrierOfThis), text: "a receiver that is not a carrier of this module", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::StatementThatIsNoLetOfName), text: "a statement that is no `let` of a name, assignment, `if` or `for` in a `fn` called inside a `ddx`", owner: RefusalOwner::Gap("L74") },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::TableModelCalledWithOtherThanThree), text: "a `table_model` called with other than three arguments", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::TableModelTableThatIsNotArray), text: "a `table_model` table that is not an array of `(f64, f64)` pairs known at elaboration", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::ThatIsNotConstantOrParam), text: "{} that is not a constant or a param", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Analog(AnalogForm::WhileOrLoopInFnCalledInside), text: "a `while` or `loop` in a `fn` called inside a `ddx`: it has no finite symbolic reading", owner: RefusalOwner::Gap("L74") },
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
    /// an argument at an array port of nodes that is no array of nodes
    ArgumentAtArrayPortOfNodesThat,
    /// an array of nodes, or a part of a nested one, as a node argument: name an element
    ArrayOfNodesOrPartOfNested,
    /// an array of resolved carriers
    ArrayOfResolvedCarriers,
    /// an array of {} whose length is not a constant yet
    ArrayOfWhoseLengthIsNotConstant,
    /// a carrier of the single-variant enum `{}` (its tag has zero levels)
    CarrierOfSingleVariantEnumItsTag,
    /// a carrier whose value type has no value in the IR
    CarrierWhoseValueTypeHasNoValue,
    /// a commit or a definition of a whole array of carriers, or of a part of one: name an element
    CommitOrDefinitionOfWholeArrayOf,
    /// a delayed definition of an element of an array of carriers
    DelayedDefinitionOfElementOfArrayOf,
    /// a drive value
    DriveValue,
    /// an element of an array whose length is not a constant yet
    ElementOfArrayWhoseLengthIsNot,
    /// an element of a value that is no array
    ElementOfValueThatIsNoArray,
    /// a field of a bundle that is no carrier of this module
    FieldOfBundleThatIsNoCarrier,
    /// a field of a bundle that is no node of this module
    FieldOfBundleThatIsNoNode,
    /// a field of an element of an array of bundles that is no carrier, read as a value
    FieldOfElementOfArrayOfBundles,
    /// a field of a type that has none
    FieldOfTypeThatHasNone,
    /// a field of a value that is no tuple or struct
    FieldOfValueThatIsNoTuple,
    /// an index of an array of carriers past its levels
    IndexOfArrayOfCarriersPastIts,
    /// an index of an array of nodes past its levels
    IndexOfArrayOfNodesPastIts,
    /// an initial value of an array of nodes other than `[x; K]` or `[a, b, c]`
    InitialValueOfArrayOfNodesOther,
    /// an initial value of an array of nodes that is not an `f64`
    InitialValueOfArrayOfNodesThat,
    /// an initial value of a nested array of carriers other than `[x; K]`
    InitialValueOfNestedArrayOfCarriers,
    /// a `let` inside a definition
    LetInsideDefinition,
    /// method `others` on this receiver
    MethodOthersOnThisReceiver,
    /// method `prev` on this receiver
    MethodPrevOnThisReceiver,
    /// `{}` of a const argument
    OfConstArgument,
    /// a part of a nested array of carriers as a value: name an element
    PartOfNestedArrayOfCarriersAs,
    /// a target that is not a carrier of this module
    TargetThatIsNotCarrierOfThis,
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
        DigitalForm::ArgumentAtArrayPortOfNodesThat,
        DigitalForm::ArrayOfNodesOrPartOfNested,
        DigitalForm::ArrayOfResolvedCarriers,
        DigitalForm::ArrayOfWhoseLengthIsNotConstant,
        DigitalForm::CarrierOfSingleVariantEnumItsTag,
        DigitalForm::CarrierWhoseValueTypeHasNoValue,
        DigitalForm::CommitOrDefinitionOfWholeArrayOf,
        DigitalForm::DelayedDefinitionOfElementOfArrayOf,
        DigitalForm::DriveValue,
        DigitalForm::ElementOfArrayWhoseLengthIsNot,
        DigitalForm::ElementOfValueThatIsNoArray,
        DigitalForm::FieldOfBundleThatIsNoCarrier,
        DigitalForm::FieldOfBundleThatIsNoNode,
        DigitalForm::FieldOfElementOfArrayOfBundles,
        DigitalForm::FieldOfTypeThatHasNone,
        DigitalForm::FieldOfValueThatIsNoTuple,
        DigitalForm::IndexOfArrayOfCarriersPastIts,
        DigitalForm::IndexOfArrayOfNodesPastIts,
        DigitalForm::InitialValueOfArrayOfNodesOther,
        DigitalForm::InitialValueOfArrayOfNodesThat,
        DigitalForm::InitialValueOfNestedArrayOfCarriers,
        DigitalForm::LetInsideDefinition,
        DigitalForm::MethodOthersOnThisReceiver,
        DigitalForm::MethodPrevOnThisReceiver,
        DigitalForm::OfConstArgument,
        DigitalForm::PartOfNestedArrayOfCarriersAs,
        DigitalForm::TargetThatIsNotCarrierOfThis,
        DigitalForm::ValueInputPortOfType,
        DigitalForm::WithGenericArguments,
        DigitalForm::WithTheseGenericArguments,
        DigitalForm::WithoutItsTypeArgument,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::Digital(DigitalForm::ArgumentAtArrayPortOfNodesThat), text: "an argument at an array port of nodes that is no array of nodes", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::ArrayOfNodesOrPartOfNested), text: "an array of nodes, or a part of a nested one, as a node argument: name an element", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::ArrayOfResolvedCarriers), text: "an array of resolved carriers", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::ArrayOfWhoseLengthIsNotConstant), text: "an array of {} whose length is not a constant yet", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::CarrierOfSingleVariantEnumItsTag), text: "a carrier of the single-variant enum `{}` (its tag has zero levels)", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::CarrierWhoseValueTypeHasNoValue), text: "a carrier whose value type has no value in the IR", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::CommitOrDefinitionOfWholeArrayOf), text: "a commit or a definition of a whole array of carriers, or of a part of one: name an element", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::DelayedDefinitionOfElementOfArrayOf), text: "a delayed definition of an element of an array of carriers", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::DriveValue), text: "a drive value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::ElementOfArrayWhoseLengthIsNot), text: "an element of an array whose length is not a constant yet", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::ElementOfValueThatIsNoArray), text: "an element of a value that is no array", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::FieldOfBundleThatIsNoCarrier), text: "a field of a bundle that is no carrier of this module", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::FieldOfBundleThatIsNoNode), text: "a field of a bundle that is no node of this module", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::FieldOfElementOfArrayOfBundles), text: "a field of an element of an array of bundles that is no carrier, read as a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::FieldOfTypeThatHasNone), text: "a field of a type that has none", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::FieldOfValueThatIsNoTuple), text: "a field of a value that is no tuple or struct", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::IndexOfArrayOfCarriersPastIts), text: "an index of an array of carriers past its levels", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::IndexOfArrayOfNodesPastIts), text: "an index of an array of nodes past its levels", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::InitialValueOfArrayOfNodesOther), text: "an initial value of an array of nodes other than `[x; K]` or `[a, b, c]`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::InitialValueOfArrayOfNodesThat), text: "an initial value of an array of nodes that is not an `f64`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::InitialValueOfNestedArrayOfCarriers), text: "an initial value of a nested array of carriers other than `[x; K]`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::LetInsideDefinition), text: "a `let` inside a definition", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::MethodOthersOnThisReceiver), text: "method `others` on this receiver", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::MethodPrevOnThisReceiver), text: "method `prev` on this receiver", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::OfConstArgument), text: "`{}` of a const argument", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::PartOfNestedArrayOfCarriersAs), text: "a part of a nested array of carriers as a value: name an element", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::TargetThatIsNotCarrierOfThis), text: "a target that is not a carrier of this module", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Digital(DigitalForm::ValueInputPortOfType), text: "value input port of type `{}`", owner: RefusalOwner::Gap("L87") },
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
    /// an analog loop whose variable is not a name
    AnalogLoopWhoseVariableIsNotName,
    /// an array of templates inside an array of templates
    ArrayOfTemplatesInsideArrayOfTemplates,
    /// an array of templates that is not an array
    ArrayOfTemplatesThatIsNotArray,
    /// an array of templates that is not written out, held by a `let`, a `const` or a `param`
    ArrayOfTemplatesThatIsNotWritten,
    /// a `bool` initial value
    BoolInitialValue,
    /// a call of a function that emits inside a `ddx`
    CallOfFunctionThatEmitsInsideDdx,
    /// a call of a function with no body inside a `ddx`
    CallOfFunctionWithNoBodyInside,
    /// a call that leaves out an argument, inside a `ddx`
    CallThatLeavesOutArgumentInsideDdx,
    /// a call whose arguments the type checker did not bind, inside a `ddx`
    CallWhoseArgumentsTypeCheckerDidNot,
    /// a choice of arrays of templates with no array in the arm it takes
    ChoiceOfArraysOfTemplatesWithNo,
    /// a composite initial value
    CompositeInitialValue,
    /// a composite value of a type that has no value type
    CompositeValueOfTypeThatHasNo,
    /// a condition that is not a `bool`
    ConditionThatIsNotBool,
    /// a const argument that is not an integer or a `bool`
    ConstArgumentThatIsNotIntegerOr,
    /// a const argument that is still a parameter
    ConstArgumentThatIsStillParameter,
    /// an expression at an `out` carrier port
    ExpressionAtOutCarrierPort,
    /// the feedback of `{}` through an instance whose value is not a placement of a module with a result
    FeedbackOfThroughInstanceWhoseValueIs,
    /// a generation condition that is not a `bool`
    GenerationConditionThatIsNotBool,
    /// a generation `for` over anything but a range with both bounds
    GenerationForOverAnythingButRangeWith,
    /// a generation `for` whose variable is not a name
    GenerationForWhoseVariableIsNotName,
    /// a generic argument that is still a parameter
    GenericArgumentThatIsStillParameter,
    /// a hoisted module that holds {},
    HoistedModuleThatHolds,
    /// an index that is not an integer
    IndexThatIsNotInteger,
    /// an integer or number-width initial value
    IntegerOrNumberWidthInitialValue,
    /// an item declared in a generation `if` that not every iteration of the `for` around it elaborates (a generated array with gaps)
    ItemDeclaredInGenerationIfThatNot,
    /// a `let` declared in a generation `if` that not every iteration of the `for` around it elaborates (a generated array with gaps), read as an array
    LetDeclaredInGenerationIfThatNot,
    /// a `let` in a param value
    LetInParamValue,
    /// a `let` of a child whose type is not known
    LetOfChildWhoseTypeIsNot,
    /// a `let` written in generation read as no array
    LetWrittenInGenerationReadAsNo,
    /// module result
    ModuleResult,
    /// param of type `{}`
    ParamOfType,
    /// a param of a type that has no value in the IR
    ParamOfTypeThatHasNoValue,
    /// a placement that sets `{}`
    PlacementThatSets,
    /// a placement through a template value
    PlacementThroughTemplateValue,
    /// port default value
    PortDefaultValue,
    /// a recursion reached from `analog` whose function returns early or returns an `Option`
    RecursionReachedFromAnalogWhoseFunctionReturns,
    /// a recursion reached from `analog` with an `Option` parameter
    RecursionReachedFromAnalogWithOptionParameter,
    /// a recursion through a function without a body
    RecursionThroughFunctionWithoutBody,
    /// a `str` that is not a text after expansion, inside a `ddx`
    StrThatIsNotTextAfterExpansion,
    /// a string literal bound by a `let`
    StringLiteralBoundByLet,
    /// a window of an array of carriers at the `out` port of a hoisted module
    WindowOfArrayOfCarriersAtOut,
}

impl StructureForm {
    /// Every form of the family, in declaration order.
    pub const ALL: &[StructureForm] = &[
        StructureForm::AnalogLoopWhoseVariableIsNotName,
        StructureForm::ArrayOfTemplatesInsideArrayOfTemplates,
        StructureForm::ArrayOfTemplatesThatIsNotArray,
        StructureForm::ArrayOfTemplatesThatIsNotWritten,
        StructureForm::BoolInitialValue,
        StructureForm::CallOfFunctionThatEmitsInsideDdx,
        StructureForm::CallOfFunctionWithNoBodyInside,
        StructureForm::CallThatLeavesOutArgumentInsideDdx,
        StructureForm::CallWhoseArgumentsTypeCheckerDidNot,
        StructureForm::ChoiceOfArraysOfTemplatesWithNo,
        StructureForm::CompositeInitialValue,
        StructureForm::CompositeValueOfTypeThatHasNo,
        StructureForm::ConditionThatIsNotBool,
        StructureForm::ConstArgumentThatIsNotIntegerOr,
        StructureForm::ConstArgumentThatIsStillParameter,
        StructureForm::ExpressionAtOutCarrierPort,
        StructureForm::FeedbackOfThroughInstanceWhoseValueIs,
        StructureForm::GenerationConditionThatIsNotBool,
        StructureForm::GenerationForOverAnythingButRangeWith,
        StructureForm::GenerationForWhoseVariableIsNotName,
        StructureForm::GenericArgumentThatIsStillParameter,
        StructureForm::HoistedModuleThatHolds,
        StructureForm::IndexThatIsNotInteger,
        StructureForm::IntegerOrNumberWidthInitialValue,
        StructureForm::ItemDeclaredInGenerationIfThatNot,
        StructureForm::LetDeclaredInGenerationIfThatNot,
        StructureForm::LetInParamValue,
        StructureForm::LetOfChildWhoseTypeIsNot,
        StructureForm::LetWrittenInGenerationReadAsNo,
        StructureForm::ModuleResult,
        StructureForm::ParamOfType,
        StructureForm::ParamOfTypeThatHasNoValue,
        StructureForm::PlacementThatSets,
        StructureForm::PlacementThroughTemplateValue,
        StructureForm::PortDefaultValue,
        StructureForm::RecursionReachedFromAnalogWhoseFunctionReturns,
        StructureForm::RecursionReachedFromAnalogWithOptionParameter,
        StructureForm::RecursionThroughFunctionWithoutBody,
        StructureForm::StrThatIsNotTextAfterExpansion,
        StructureForm::StringLiteralBoundByLet,
        StructureForm::WindowOfArrayOfCarriersAtOut,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::Structure(StructureForm::AnalogLoopWhoseVariableIsNotName), text: "an analog loop whose variable is not a name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::ArrayOfTemplatesInsideArrayOfTemplates), text: "an array of templates inside an array of templates", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::ArrayOfTemplatesThatIsNotArray), text: "an array of templates that is not an array", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::ArrayOfTemplatesThatIsNotWritten), text: "an array of templates that is not written out, held by a `let`, a `const` or a `param`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::BoolInitialValue), text: "a `bool` initial value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::CallOfFunctionThatEmitsInsideDdx), text: "a call of a function that emits inside a `ddx`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::CallOfFunctionWithNoBodyInside), text: "a call of a function with no body inside a `ddx`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::CallThatLeavesOutArgumentInsideDdx), text: "a call that leaves out an argument, inside a `ddx`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::CallWhoseArgumentsTypeCheckerDidNot), text: "a call whose arguments the type checker did not bind, inside a `ddx`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::ChoiceOfArraysOfTemplatesWithNo), text: "a choice of arrays of templates with no array in the arm it takes", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::CompositeInitialValue), text: "a composite initial value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::CompositeValueOfTypeThatHasNo), text: "a composite value of a type that has no value type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::ConditionThatIsNotBool), text: "a condition that is not a `bool`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::ConstArgumentThatIsNotIntegerOr), text: "a const argument that is not an integer or a `bool`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::ConstArgumentThatIsStillParameter), text: "a const argument that is still a parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::ExpressionAtOutCarrierPort), text: "an expression at an `out` carrier port", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::FeedbackOfThroughInstanceWhoseValueIs), text: "the feedback of `{}` through an instance whose value is not a placement of a module with a result", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::GenerationConditionThatIsNotBool), text: "a generation condition that is not a `bool`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::GenerationForOverAnythingButRangeWith), text: "a generation `for` over anything but a range with both bounds", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::GenerationForWhoseVariableIsNotName), text: "a generation `for` whose variable is not a name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::GenericArgumentThatIsStillParameter), text: "a generic argument that is still a parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::HoistedModuleThatHolds), text: "a hoisted module that holds {},", owner: RefusalOwner::Gap("L95") },
        RefusalRow { form: RefusedForm::Structure(StructureForm::IndexThatIsNotInteger), text: "an index that is not an integer", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::IntegerOrNumberWidthInitialValue), text: "an integer or number-width initial value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::ItemDeclaredInGenerationIfThatNot), text: "an item declared in a generation `if` that not every iteration of the `for` around it elaborates (a generated array with gaps)", owner: RefusalOwner::Gap("L76") },
        RefusalRow { form: RefusedForm::Structure(StructureForm::LetDeclaredInGenerationIfThatNot), text: "a `let` declared in a generation `if` that not every iteration of the `for` around it elaborates (a generated array with gaps), read as an array", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::LetInParamValue), text: "a `let` in a param value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::LetOfChildWhoseTypeIsNot), text: "a `let` of a child whose type is not known", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::LetWrittenInGenerationReadAsNo), text: "a `let` written in generation read as no array", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::ModuleResult), text: "module result", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::ParamOfType), text: "param of type `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::ParamOfTypeThatHasNoValue), text: "a param of a type that has no value in the IR", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::PlacementThatSets), text: "a placement that sets `{}`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::PlacementThroughTemplateValue), text: "a placement through a template value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::PortDefaultValue), text: "port default value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::RecursionReachedFromAnalogWhoseFunctionReturns), text: "a recursion reached from `analog` whose function returns early or returns an `Option`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::RecursionReachedFromAnalogWithOptionParameter), text: "a recursion reached from `analog` with an `Option` parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::RecursionThroughFunctionWithoutBody), text: "a recursion through a function without a body", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::StrThatIsNotTextAfterExpansion), text: "a `str` that is not a text after expansion, inside a `ddx`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::StringLiteralBoundByLet), text: "a string literal bound by a `let`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Structure(StructureForm::WindowOfArrayOfCarriersAtOut), text: "a window of an array of carriers at the `out` port of a hoisted module", owner: RefusalOwner::Unswept },
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
    /// a method call through a bound that no impl gives for this type
    MethodCallThroughBoundThatNoImpl,
    /// a method of a generic impl whose parameters the receiver leaves open
    MethodOfGenericImplWhoseParametersReceiver,
    /// an operator other than a comparison on `Option`s
    OperatorOtherThanComparisonOnOptions,
    /// `Option::default()` without the payload type written as `Option::<T>::default()`
    OptionDefaultWithoutPayloadTypeWrittenAs,
    /// a placement of a generic module whose arguments the checker did not record
    PlacementOfGenericModuleWhoseArgumentsChecker,
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
        GenericsForm::MethodCallThroughBoundThatNoImpl,
        GenericsForm::MethodOfGenericImplWhoseParametersReceiver,
        GenericsForm::OperatorOtherThanComparisonOnOptions,
        GenericsForm::OptionDefaultWithoutPayloadTypeWrittenAs,
        GenericsForm::PlacementOfGenericModuleWhoseArgumentsChecker,
        GenericsForm::ReprOtherThanSizedInteger,
        GenericsForm::SelfOutsideImplOrTrait,
        GenericsForm::TemplateParameterOfBoundedByFn,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::Generics(GenericsForm::CallOfTraitFunctionWithNoReceiver), text: "a call of a trait's function with no `self`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Generics(GenericsForm::DiscriminantOfThatIsNotIntegerLiteral), text: "a discriminant of `{}` that is not an integer literal, a `const` or integer arithmetic of them", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Generics(GenericsForm::MethodCallThroughBoundThatNoImpl), text: "a method call through a bound that no impl gives for this type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Generics(GenericsForm::MethodOfGenericImplWhoseParametersReceiver), text: "a method of a generic impl whose parameters the receiver leaves open", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Generics(GenericsForm::OperatorOtherThanComparisonOnOptions), text: "an operator other than a comparison on `Option`s", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Generics(GenericsForm::OptionDefaultWithoutPayloadTypeWrittenAs), text: "`Option::default()` without the payload type written as `Option::<T>::default()`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Generics(GenericsForm::PlacementOfGenericModuleWhoseArgumentsChecker), text: "a placement of a generic module whose arguments the checker did not record", owner: RefusalOwner::Unswept },
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
    /// an argument that is not a closure or a function
    ArgumentThatIsNotClosureOrFunction,
    /// an argument with neither a value nor a function
    ArgumentWithNeitherValueNorFunction,
    /// an array length that is not a non-negative integer
    ArrayLengthThatIsNotNonNegative,
    /// an array too long to parse
    ArrayTooLongToParse,
    /// an assignment into a local with no value
    AssignmentIntoLocalWithNoValue,
    /// an assignment to something that is not a local, a field or an element
    AssignmentToSomethingThatIsNotLocal,
    /// a `bool` where an `f64` is required
    BoolWhereFloatIsRequired,
    /// a call of a name that is not a function
    CallOfNameThatIsNotFunction,
    /// this cast
    Cast,
    /// a cast of a composite value
    CastOfCompositeValue,
    /// a cast of a variant outside its enum
    CastOfVariantOutsideItsEnum,
    /// a closure parameter pattern that does not match its argument
    ClosureParameterPatternThatDoesNotMatch,
    /// a closure that does not give a `bool`
    ClosureThatDoesNotGiveBool,
    /// a comparison of an `f64` and a `bool`
    ComparisonOfFloatAndBool,
    /// a composite param in a const expression
    CompositeParamInConstExpression,
    /// a composite value bound from outside the evaluation
    CompositeValueBoundFromOutsideEvaluation,
    /// a composite value input in a const expression
    CompositeValueInputInConstExpression,
    /// a composite value that does not have the type its position needs
    CompositeValueThatDoesNotHaveType,
    /// a composite value where a `bool` is required
    CompositeValueWhereBoolIsRequired,
    /// a composite value where an `f64` is required
    CompositeValueWhereFloatIsRequired,
    /// a composite value where a scalar is required
    CompositeValueWhereScalarIsRequired,
    /// a composite value with a different number of parts than its type
    CompositeValueWithDifferentNumberOfParts,
    /// a const argument of a template that is not an integer or a `bool`
    ConstArgumentOfTemplateThatIsNot,
    /// a `const` composite of a type with no constant form
    ConstCompositeOfTypeWithNoConstant,
    /// a `const` without a value
    ConstWithoutValue,
    /// `contains` of a value that is not a range
    ContainsOfValueThatIsNotRange,
    /// the default of this type
    DefaultOfThisType,
    /// an element index that is not a non-negative integer
    ElementIndexThatIsNotNonNegative,
    /// an `encode` of a value that is no scalar
    EncodeOfValueThatIsNoScalar,
    /// an `encode` that gives a level that is not a `bool`
    EncodeThatGivesLevelThatIsNot,
    /// a field out of range
    FieldOutOfRange,
    /// a field pattern that names no field
    FieldPatternThatNamesNoField,
    /// a field read of a value that is not a template
    FieldReadOfValueThatIsNot,
    /// a field the struct does not have
    FieldStructDoesNotHave,
    /// a field the value does not have
    FieldValueDoesNotHave,
    /// a float literal
    FloatLiteral,
    /// an `f64` where a `bool` is required
    FloatWhereBoolIsRequired,
    /// a `for` range whose bounds are not integers
    ForRangeWhoseBoundsAreNotIntegers,
    /// this form in a const expression
    FormInConstExpression,
    /// a form with no value where a value is needed
    FormWithNoValueWhereValueIs,
    /// a function body with no value
    FunctionBodyWithNoValue,
    /// a function bound by a pattern that is not a name
    FunctionBoundByPatternThatIsNot,
    /// a generation range whose bound is not an integer
    GenerationRangeWhoseBoundIsNotInteger,
    /// a generic argument of a template that is still a parameter
    GenericArgumentOfTemplateThatIsStill,
    /// a hex image of other than integers
    HexImageOfOtherThanIntegers,
    /// an implicit `Some` of a value with no value type
    ImplicitSomeOfValueWithNoValue,
    /// an `include_str!` path with an undefined escape
    IncludeStrPathWithUndefinedEscape,
    /// an integer literal
    IntegerLiteral,
    /// an integer operator outside an integer type
    IntegerOperatorOutsideIntegerType,
    /// an integer operator over a non-integer
    IntegerOperatorOverNonInteger,
    /// an integer where a `bool` is required
    IntegerWhereBoolIsRequired,
    /// an integer where an `f64` is required
    IntegerWhereFloatIsRequired,
    /// an iteration over a value that is not an array, a range or an iterator
    IterationOverValueThatIsNotArray,
    /// a `let .. else` whose `else` does not leave
    LetElseWhoseElseDoesNotLeave,
    /// this literal
    Literal,
    /// a `logic` pattern that is no literal
    LogicPatternThatIsNoLiteral,
    /// a `logic` pattern with a character that names no level
    LogicPatternWithCharacterThatNamesNo,
    /// a `match` no arm of which matches
    MatchNoArmOfWhichMatches,
    /// this method in a const expression
    MethodInConstExpression,
    /// this method of a float
    MethodOfFloat,
    /// a method of a float over a non-float
    MethodOfFloatOverNonFloat,
    /// this method of an integer
    MethodOfInteger,
    /// a method of an integer over a non-integer
    MethodOfIntegerOverNonInteger,
    /// a name
    Name,
    /// this name in a const expression
    NameInConstExpression,
    /// a number-width value where a `bool` is required
    NumberWidthValueWhereBoolIsRequired,
    /// a number-width value where an `f64` is required
    NumberWidthValueWhereFloatIsRequired,
    /// this operator
    Operator,
    /// this operator on a float
    OperatorOnFloat,
    /// this operator on integers
    OperatorOnIntegers,
    /// an operator other than a comparison on a composite value
    OperatorOtherThanComparisonOnCompositeValue,
    /// an operator over a float and another type
    OperatorOverFloatAndAnotherType,
    /// this `Option` row in a const expression
    OptionRowInConstExpression,
    /// the param `{}` of a template, which has no value
    ParamOfTemplateWhichHasNoValue,
    /// a param `{}` the template's module does not have
    ParamTemplateModuleDoesNotHave,
    /// a parsed array whose length is a generic argument
    ParsedArrayWhoseLengthIsGenericArgument,
    /// a parser called with other than one text
    ParserCalledWithOtherThanOneText,
    /// this parser over this element type
    ParserOverThisElementType,
    /// a parser whose array type the checker did not give
    ParserWhoseArrayTypeCheckerDidNot,
    /// a path pattern that is not a constant or a unit variant
    PathPatternThatIsNotConstantOr,
    /// this pattern in a const expression
    PatternInConstExpression,
    /// a pattern that names no struct or variant
    PatternThatNamesNoStructOrVariant,
    /// `pow` by a non-constant exponent
    PowByNonConstantExponent,
    /// `powi` by a non-constant exponent
    PowiByNonConstantExponent,
    /// a range index bound that is not a non-negative integer
    RangeIndexBoundThatIsNotNon,
    /// a range pattern over values that are not both integers or both floats
    RangePatternOverValuesThatAreNot,
    /// a range whose bounds are not integers
    RangeWhoseBoundsAreNotIntegers,
    /// a range with no high bound
    RangeWithNoHighBound,
    /// a range with no low bound
    RangeWithNoLowBound,
    /// a refutable pattern in a `let` without `else`
    RefutablePatternInLetWithoutElse,
    /// a row outside Appendix H.5 to H.7
    RowOutsideAppendixRows,
    /// a shift by a non-integer
    ShiftByNonInteger,
    /// this statement in a const expression
    StatementInConstExpression,
    /// a string with an undefined escape
    StringWithUndefinedEscape,
    /// a struct expression that leaves a field out
    StructExpressionThatLeavesFieldOut,
    /// a text that is no `include_str!` or `const` of one
    TextThatIsNoIncludeStrOr,
    /// a text value other than `include_str!`
    TextValueOtherThanIncludeStr,
    /// {} that is not a composite value
    ThatIsNotCompositeValue,
    /// a value built from a name that is neither a struct nor a variant
    ValueBuiltFromNameThatIsNeither,
}

impl ConstantForm {
    /// Every form of the family, in declaration order.
    pub const ALL: &[ConstantForm] = &[
        ConstantForm::ArgumentThatIsNotClosureOrFunction,
        ConstantForm::ArgumentWithNeitherValueNorFunction,
        ConstantForm::ArrayLengthThatIsNotNonNegative,
        ConstantForm::ArrayTooLongToParse,
        ConstantForm::AssignmentIntoLocalWithNoValue,
        ConstantForm::AssignmentToSomethingThatIsNotLocal,
        ConstantForm::BoolWhereFloatIsRequired,
        ConstantForm::CallOfNameThatIsNotFunction,
        ConstantForm::Cast,
        ConstantForm::CastOfCompositeValue,
        ConstantForm::CastOfVariantOutsideItsEnum,
        ConstantForm::ClosureParameterPatternThatDoesNotMatch,
        ConstantForm::ClosureThatDoesNotGiveBool,
        ConstantForm::ComparisonOfFloatAndBool,
        ConstantForm::CompositeParamInConstExpression,
        ConstantForm::CompositeValueBoundFromOutsideEvaluation,
        ConstantForm::CompositeValueInputInConstExpression,
        ConstantForm::CompositeValueThatDoesNotHaveType,
        ConstantForm::CompositeValueWhereBoolIsRequired,
        ConstantForm::CompositeValueWhereFloatIsRequired,
        ConstantForm::CompositeValueWhereScalarIsRequired,
        ConstantForm::CompositeValueWithDifferentNumberOfParts,
        ConstantForm::ConstArgumentOfTemplateThatIsNot,
        ConstantForm::ConstCompositeOfTypeWithNoConstant,
        ConstantForm::ConstWithoutValue,
        ConstantForm::ContainsOfValueThatIsNotRange,
        ConstantForm::DefaultOfThisType,
        ConstantForm::ElementIndexThatIsNotNonNegative,
        ConstantForm::EncodeOfValueThatIsNoScalar,
        ConstantForm::EncodeThatGivesLevelThatIsNot,
        ConstantForm::FieldOutOfRange,
        ConstantForm::FieldPatternThatNamesNoField,
        ConstantForm::FieldReadOfValueThatIsNot,
        ConstantForm::FieldStructDoesNotHave,
        ConstantForm::FieldValueDoesNotHave,
        ConstantForm::FloatLiteral,
        ConstantForm::FloatWhereBoolIsRequired,
        ConstantForm::ForRangeWhoseBoundsAreNotIntegers,
        ConstantForm::FormInConstExpression,
        ConstantForm::FormWithNoValueWhereValueIs,
        ConstantForm::FunctionBodyWithNoValue,
        ConstantForm::FunctionBoundByPatternThatIsNot,
        ConstantForm::GenerationRangeWhoseBoundIsNotInteger,
        ConstantForm::GenericArgumentOfTemplateThatIsStill,
        ConstantForm::HexImageOfOtherThanIntegers,
        ConstantForm::ImplicitSomeOfValueWithNoValue,
        ConstantForm::IncludeStrPathWithUndefinedEscape,
        ConstantForm::IntegerLiteral,
        ConstantForm::IntegerOperatorOutsideIntegerType,
        ConstantForm::IntegerOperatorOverNonInteger,
        ConstantForm::IntegerWhereBoolIsRequired,
        ConstantForm::IntegerWhereFloatIsRequired,
        ConstantForm::IterationOverValueThatIsNotArray,
        ConstantForm::LetElseWhoseElseDoesNotLeave,
        ConstantForm::Literal,
        ConstantForm::LogicPatternThatIsNoLiteral,
        ConstantForm::LogicPatternWithCharacterThatNamesNo,
        ConstantForm::MatchNoArmOfWhichMatches,
        ConstantForm::MethodInConstExpression,
        ConstantForm::MethodOfFloat,
        ConstantForm::MethodOfFloatOverNonFloat,
        ConstantForm::MethodOfInteger,
        ConstantForm::MethodOfIntegerOverNonInteger,
        ConstantForm::Name,
        ConstantForm::NameInConstExpression,
        ConstantForm::NumberWidthValueWhereBoolIsRequired,
        ConstantForm::NumberWidthValueWhereFloatIsRequired,
        ConstantForm::Operator,
        ConstantForm::OperatorOnFloat,
        ConstantForm::OperatorOnIntegers,
        ConstantForm::OperatorOtherThanComparisonOnCompositeValue,
        ConstantForm::OperatorOverFloatAndAnotherType,
        ConstantForm::OptionRowInConstExpression,
        ConstantForm::ParamOfTemplateWhichHasNoValue,
        ConstantForm::ParamTemplateModuleDoesNotHave,
        ConstantForm::ParsedArrayWhoseLengthIsGenericArgument,
        ConstantForm::ParserCalledWithOtherThanOneText,
        ConstantForm::ParserOverThisElementType,
        ConstantForm::ParserWhoseArrayTypeCheckerDidNot,
        ConstantForm::PathPatternThatIsNotConstantOr,
        ConstantForm::PatternInConstExpression,
        ConstantForm::PatternThatNamesNoStructOrVariant,
        ConstantForm::PowByNonConstantExponent,
        ConstantForm::PowiByNonConstantExponent,
        ConstantForm::RangeIndexBoundThatIsNotNon,
        ConstantForm::RangePatternOverValuesThatAreNot,
        ConstantForm::RangeWhoseBoundsAreNotIntegers,
        ConstantForm::RangeWithNoHighBound,
        ConstantForm::RangeWithNoLowBound,
        ConstantForm::RefutablePatternInLetWithoutElse,
        ConstantForm::RowOutsideAppendixRows,
        ConstantForm::ShiftByNonInteger,
        ConstantForm::StatementInConstExpression,
        ConstantForm::StringWithUndefinedEscape,
        ConstantForm::StructExpressionThatLeavesFieldOut,
        ConstantForm::TextThatIsNoIncludeStrOr,
        ConstantForm::TextValueOtherThanIncludeStr,
        ConstantForm::ThatIsNotCompositeValue,
        ConstantForm::ValueBuiltFromNameThatIsNeither,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ArgumentThatIsNotClosureOrFunction), text: "an argument that is not a closure or a function", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ArgumentWithNeitherValueNorFunction), text: "an argument with neither a value nor a function", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ArrayLengthThatIsNotNonNegative), text: "an array length that is not a non-negative integer", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ArrayTooLongToParse), text: "an array too long to parse", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::AssignmentIntoLocalWithNoValue), text: "an assignment into a local with no value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::AssignmentToSomethingThatIsNotLocal), text: "an assignment to something that is not a local, a field or an element", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::BoolWhereFloatIsRequired), text: "a `bool` where an `f64` is required", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::CallOfNameThatIsNotFunction), text: "a call of a name that is not a function", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::Cast), text: "this cast", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::CastOfCompositeValue), text: "a cast of a composite value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::CastOfVariantOutsideItsEnum), text: "a cast of a variant outside its enum", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ClosureParameterPatternThatDoesNotMatch), text: "a closure parameter pattern that does not match its argument", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ClosureThatDoesNotGiveBool), text: "a closure that does not give a `bool`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ComparisonOfFloatAndBool), text: "a comparison of an `f64` and a `bool`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::CompositeParamInConstExpression), text: "a composite param in a const expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::CompositeValueBoundFromOutsideEvaluation), text: "a composite value bound from outside the evaluation", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::CompositeValueInputInConstExpression), text: "a composite value input in a const expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::CompositeValueThatDoesNotHaveType), text: "a composite value that does not have the type its position needs", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::CompositeValueWhereBoolIsRequired), text: "a composite value where a `bool` is required", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::CompositeValueWhereFloatIsRequired), text: "a composite value where an `f64` is required", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::CompositeValueWhereScalarIsRequired), text: "a composite value where a scalar is required", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::CompositeValueWithDifferentNumberOfParts), text: "a composite value with a different number of parts than its type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ConstArgumentOfTemplateThatIsNot), text: "a const argument of a template that is not an integer or a `bool`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ConstCompositeOfTypeWithNoConstant), text: "a `const` composite of a type with no constant form", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ConstWithoutValue), text: "a `const` without a value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ContainsOfValueThatIsNotRange), text: "`contains` of a value that is not a range", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::DefaultOfThisType), text: "the default of this type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ElementIndexThatIsNotNonNegative), text: "an element index that is not a non-negative integer", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::EncodeOfValueThatIsNoScalar), text: "an `encode` of a value that is no scalar", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::EncodeThatGivesLevelThatIsNot), text: "an `encode` that gives a level that is not a `bool`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::FieldOutOfRange), text: "a field out of range", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::FieldPatternThatNamesNoField), text: "a field pattern that names no field", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::FieldReadOfValueThatIsNot), text: "a field read of a value that is not a template", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::FieldStructDoesNotHave), text: "a field the struct does not have", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::FieldValueDoesNotHave), text: "a field the value does not have", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::FloatLiteral), text: "a float literal", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::FloatWhereBoolIsRequired), text: "an `f64` where a `bool` is required", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ForRangeWhoseBoundsAreNotIntegers), text: "a `for` range whose bounds are not integers", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::FormInConstExpression), text: "this form in a const expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::FormWithNoValueWhereValueIs), text: "a form with no value where a value is needed", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::FunctionBodyWithNoValue), text: "a function body with no value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::FunctionBoundByPatternThatIsNot), text: "a function bound by a pattern that is not a name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::GenerationRangeWhoseBoundIsNotInteger), text: "a generation range whose bound is not an integer", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::GenericArgumentOfTemplateThatIsStill), text: "a generic argument of a template that is still a parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::HexImageOfOtherThanIntegers), text: "a hex image of other than integers", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ImplicitSomeOfValueWithNoValue), text: "an implicit `Some` of a value with no value type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::IncludeStrPathWithUndefinedEscape), text: "an `include_str!` path with an undefined escape", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::IntegerLiteral), text: "an integer literal", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::IntegerOperatorOutsideIntegerType), text: "an integer operator outside an integer type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::IntegerOperatorOverNonInteger), text: "an integer operator over a non-integer", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::IntegerWhereBoolIsRequired), text: "an integer where a `bool` is required", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::IntegerWhereFloatIsRequired), text: "an integer where an `f64` is required", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::IterationOverValueThatIsNotArray), text: "an iteration over a value that is not an array, a range or an iterator", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::LetElseWhoseElseDoesNotLeave), text: "a `let .. else` whose `else` does not leave", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::Literal), text: "this literal", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::LogicPatternThatIsNoLiteral), text: "a `logic` pattern that is no literal", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::LogicPatternWithCharacterThatNamesNo), text: "a `logic` pattern with a character that names no level", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::MatchNoArmOfWhichMatches), text: "a `match` no arm of which matches", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::MethodInConstExpression), text: "this method in a const expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::MethodOfFloat), text: "this method of a float", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::MethodOfFloatOverNonFloat), text: "a method of a float over a non-float", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::MethodOfInteger), text: "this method of an integer", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::MethodOfIntegerOverNonInteger), text: "a method of an integer over a non-integer", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::Name), text: "a name", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::NameInConstExpression), text: "this name in a const expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::NumberWidthValueWhereBoolIsRequired), text: "a number-width value where a `bool` is required", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::NumberWidthValueWhereFloatIsRequired), text: "a number-width value where an `f64` is required", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::Operator), text: "this operator", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::OperatorOnFloat), text: "this operator on a float", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::OperatorOnIntegers), text: "this operator on integers", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::OperatorOtherThanComparisonOnCompositeValue), text: "an operator other than a comparison on a composite value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::OperatorOverFloatAndAnotherType), text: "an operator over a float and another type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::OptionRowInConstExpression), text: "this `Option` row in a const expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ParamOfTemplateWhichHasNoValue), text: "the param `{}` of a template, which has no value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ParamTemplateModuleDoesNotHave), text: "a param `{}` the template's module does not have", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ParsedArrayWhoseLengthIsGenericArgument), text: "a parsed array whose length is a generic argument", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ParserCalledWithOtherThanOneText), text: "a parser called with other than one text", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ParserOverThisElementType), text: "this parser over this element type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ParserWhoseArrayTypeCheckerDidNot), text: "a parser whose array type the checker did not give", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::PathPatternThatIsNotConstantOr), text: "a path pattern that is not a constant or a unit variant", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::PatternInConstExpression), text: "this pattern in a const expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::PatternThatNamesNoStructOrVariant), text: "a pattern that names no struct or variant", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::PowByNonConstantExponent), text: "`pow` by a non-constant exponent", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::PowiByNonConstantExponent), text: "`powi` by a non-constant exponent", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::RangeIndexBoundThatIsNotNon), text: "a range index bound that is not a non-negative integer", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::RangePatternOverValuesThatAreNot), text: "a range pattern over values that are not both integers or both floats", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::RangeWhoseBoundsAreNotIntegers), text: "a range whose bounds are not integers", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::RangeWithNoHighBound), text: "a range with no high bound", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::RangeWithNoLowBound), text: "a range with no low bound", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::RefutablePatternInLetWithoutElse), text: "a refutable pattern in a `let` without `else`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::RowOutsideAppendixRows), text: "a row outside Appendix H.5 to H.7", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ShiftByNonInteger), text: "a shift by a non-integer", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::StatementInConstExpression), text: "this statement in a const expression", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::StringWithUndefinedEscape), text: "a string with an undefined escape", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::StructExpressionThatLeavesFieldOut), text: "a struct expression that leaves a field out", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::TextThatIsNoIncludeStrOr), text: "a text that is no `include_str!` or `const` of one", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::TextValueOtherThanIncludeStr), text: "a text value other than `include_str!`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ThatIsNotCompositeValue), text: "{} that is not a composite value", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Constant(ConstantForm::ValueBuiltFromNameThatIsNeither), text: "a value built from a name that is neither a struct nor a variant", owner: RefusalOwner::Unswept },
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

/// The item forms the front end refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemForm {
    /// behavior block
    BehaviorBlock,
    /// `const` item
    ConstItem,
    /// definition `=`
    Definition,
    /// `enum` item
    EnumItem,
    /// `enum` variant
    EnumVariant,
    /// field
    Field,
    /// `fn` item
    FnItem,
    /// function parameter
    FunctionParameter,
    /// generation `for`/`if`
    GenerationForIf,
    /// generic parameter
    GenericParameter,
    /// `impl` item
    ImplItem,
    /// malformed item
    MalformedItem,
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
    /// `nature` item
    NatureItem,
    /// nature quantity
    NatureQuantity,
    /// `on` reaction
    OnReaction,
    /// `signal` item
    SignalItem,
    /// `struct` item
    StructItem,
    /// `trait` item
    TraitItem,
    /// `use` item
    UseItem,
}

impl ItemForm {
    /// Every form of the family, in declaration order.
    pub const ALL: &[ItemForm] = &[
        ItemForm::BehaviorBlock,
        ItemForm::ConstItem,
        ItemForm::Definition,
        ItemForm::EnumItem,
        ItemForm::EnumVariant,
        ItemForm::Field,
        ItemForm::FnItem,
        ItemForm::FunctionParameter,
        ItemForm::GenerationForIf,
        ItemForm::GenericParameter,
        ItemForm::ImplItem,
        ItemForm::MalformedItem,
        ItemForm::ModItem,
        ItemForm::ModuleBodyLet,
        ItemForm::ModuleItem,
        ItemForm::ModuleParam,
        ItemForm::ModulePort,
        ItemForm::NatureItem,
        ItemForm::NatureQuantity,
        ItemForm::OnReaction,
        ItemForm::SignalItem,
        ItemForm::StructItem,
        ItemForm::TraitItem,
        ItemForm::UseItem,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::Item(ItemForm::BehaviorBlock), text: "behavior block", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::ConstItem), text: "`const` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::Definition), text: "definition `=`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::EnumItem), text: "`enum` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::EnumVariant), text: "`enum` variant", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::Field), text: "field", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::FnItem), text: "`fn` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::FunctionParameter), text: "function parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::GenerationForIf), text: "generation `for`/`if`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::GenericParameter), text: "generic parameter", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::ImplItem), text: "`impl` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::MalformedItem), text: "malformed item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::ModItem), text: "`mod` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::ModuleBodyLet), text: "module-body `let`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::ModuleItem), text: "`module` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::ModuleParam), text: "module param", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::ModulePort), text: "module port", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::NatureItem), text: "`nature` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::NatureQuantity), text: "nature quantity", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::OnReaction), text: "`on` reaction", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::SignalItem), text: "`signal` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::StructItem), text: "`struct` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::TraitItem), text: "`trait` item", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::Item(ItemForm::UseItem), text: "`use` item", owner: RefusalOwner::Unswept },
    ];

    /// The row of the form: its words and its owner.
    pub fn row(self) -> &'static RefusalRow {
        &Self::ROWS[self as usize]
    }
}

impl From<ItemForm> for RefusedForm {
    fn from(form: ItemForm) -> RefusedForm {
        RefusedForm::Item(form)
    }
}

/// The eventfunction forms the front end refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventFunctionForm {
    /// a call whose arguments the type checker did not bind, as an event
    CallWhoseArgumentsTypeCheckerDidNot,
    /// a function that returns an `Event` and has no body
    FunctionThatReturnsEventAndHasNo,
    /// a function that returns an `Event` that does not end in its event
    FunctionThatReturnsEventThatDoesNot,
    /// a function that returns an `Event` with an empty body
    FunctionThatReturnsEventWithEmptyBody,
    /// an `if let` in a function that returns an `Event`
    IfLetInFunctionThatReturnsEvent,
    /// a `match` arm whose pattern binds a name, in a function that returns an `Event`
    MatchArmWhosePatternBindsNameIn,
    /// a `match` arm with a guard in a function that returns an `Event`
    MatchArmWithGuardInFunctionThat,
    /// a method that returns an `Event` that no impl gives for this type
    MethodThatReturnsEventThatNoImpl,
    /// a statement that is no `let`, assignment, `if`, `match` or `for` before the event of a function that returns an `Event`
    StatementThatIsNoLetAssignmentIf,
    /// a statement that is no `let` of a name before the event of a function that returns an `Event`
    StatementThatIsNoLetOfNameBefore,
    /// a `while`, `loop`, `break`, `continue` or `return` with no value in a function that returns an `Event`
    WhileLoopBreakContinueOrReturnWith,
}

impl EventFunctionForm {
    /// Every form of the family, in declaration order.
    pub const ALL: &[EventFunctionForm] = &[
        EventFunctionForm::CallWhoseArgumentsTypeCheckerDidNot,
        EventFunctionForm::FunctionThatReturnsEventAndHasNo,
        EventFunctionForm::FunctionThatReturnsEventThatDoesNot,
        EventFunctionForm::FunctionThatReturnsEventWithEmptyBody,
        EventFunctionForm::IfLetInFunctionThatReturnsEvent,
        EventFunctionForm::MatchArmWhosePatternBindsNameIn,
        EventFunctionForm::MatchArmWithGuardInFunctionThat,
        EventFunctionForm::MethodThatReturnsEventThatNoImpl,
        EventFunctionForm::StatementThatIsNoLetAssignmentIf,
        EventFunctionForm::StatementThatIsNoLetOfNameBefore,
        EventFunctionForm::WhileLoopBreakContinueOrReturnWith,
    ];

    /// The rows of the family, one per form in declaration order.
    const ROWS: &[RefusalRow] = &[
        RefusalRow { form: RefusedForm::EventFunction(EventFunctionForm::CallWhoseArgumentsTypeCheckerDidNot), text: "a call whose arguments the type checker did not bind, as an event", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::EventFunction(EventFunctionForm::FunctionThatReturnsEventAndHasNo), text: "a function that returns an `Event` and has no body", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::EventFunction(EventFunctionForm::FunctionThatReturnsEventThatDoesNot), text: "a function that returns an `Event` that does not end in its event", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::EventFunction(EventFunctionForm::FunctionThatReturnsEventWithEmptyBody), text: "a function that returns an `Event` with an empty body", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::EventFunction(EventFunctionForm::IfLetInFunctionThatReturnsEvent), text: "an `if let` in a function that returns an `Event`", owner: RefusalOwner::Gap("L75") },
        RefusalRow { form: RefusedForm::EventFunction(EventFunctionForm::MatchArmWhosePatternBindsNameIn), text: "a `match` arm whose pattern binds a name, in a function that returns an `Event`", owner: RefusalOwner::Gap("L75") },
        RefusalRow { form: RefusedForm::EventFunction(EventFunctionForm::MatchArmWithGuardInFunctionThat), text: "a `match` arm with a guard in a function that returns an `Event`", owner: RefusalOwner::Gap("L75") },
        RefusalRow { form: RefusedForm::EventFunction(EventFunctionForm::MethodThatReturnsEventThatNoImpl), text: "a method that returns an `Event` that no impl gives for this type", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::EventFunction(EventFunctionForm::StatementThatIsNoLetAssignmentIf), text: "a statement that is no `let`, assignment, `if`, `match` or `for` before the event of a function that returns an `Event`", owner: RefusalOwner::Gap("L75") },
        RefusalRow { form: RefusedForm::EventFunction(EventFunctionForm::StatementThatIsNoLetOfNameBefore), text: "a statement that is no `let` of a name before the event of a function that returns an `Event`", owner: RefusalOwner::Unswept },
        RefusalRow { form: RefusedForm::EventFunction(EventFunctionForm::WhileLoopBreakContinueOrReturnWith), text: "a `while`, `loop`, `break`, `continue` or `return` with no value in a function that returns an `Event`", owner: RefusalOwner::Gap("L75") },
    ];

    /// The row of the form: its words and its owner.
    pub fn row(self) -> &'static RefusalRow {
        &Self::ROWS[self as usize]
    }
}

impl From<EventFunctionForm> for RefusedForm {
    fn from(form: EventFunctionForm) -> RefusedForm {
        RefusedForm::EventFunction(form)
    }
}
