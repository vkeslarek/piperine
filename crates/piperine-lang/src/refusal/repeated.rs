//! The forms refused from more than one place: each is built here once and the sites call the method, so a refusal has
//! one owner whatever the number of places that raise it.

use super::forms::{ValueForm, PatternForm, AnalogForm, DigitalForm, StructureForm, GenericsForm, ConstantForm};
use super::RefusedForm;

impl RefusedForm {
    /// The form of a contribution target that is not a probe, refused from more than one place.
    pub fn contribution_target_that_is_not_probe() -> RefusedForm {
        AnalogForm::ContributionTargetThatIsNotProbe.into()
    }

    /// The form of a probe of more than two nodes, refused from more than one place.
    pub fn probe_of_more_than_two_nodes() -> RefusedForm {
        AnalogForm::ProbeOfMoreThanTwoNodes.into()
    }

    /// The form of this cast, refused from more than one place.
    pub fn cast() -> RefusedForm {
        ConstantForm::Cast.into()
    }

    /// The form of a composite value where a scalar is required, refused from more than one place.
    pub fn composite_value_where_scalar_is_required() -> RefusedForm {
        ConstantForm::CompositeValueWhereScalarIsRequired.into()
    }

    /// The form of a field the struct does not have, refused from more than one place.
    pub fn field_struct_does_not_have() -> RefusedForm {
        ConstantForm::FieldStructDoesNotHave.into()
    }

    /// The form of an `include_str!` path with an undefined escape, refused from more than one place.
    pub fn include_str_path_with_undefined_escape() -> RefusedForm {
        ConstantForm::IncludeStrPathWithUndefinedEscape.into()
    }

    /// The form of an integer literal, refused from more than one place.
    pub fn integer_literal() -> RefusedForm {
        ConstantForm::IntegerLiteral.into()
    }

    /// The form of an integer operator over a non-integer, refused from more than one place.
    pub fn integer_operator_over_non_integer() -> RefusedForm {
        ConstantForm::IntegerOperatorOverNonInteger.into()
    }

    /// The form of this method in a const expression, refused from more than one place.
    pub fn method_in_const_expression() -> RefusedForm {
        ConstantForm::MethodInConstExpression.into()
    }

    /// The form of this method of a float, refused from more than one place.
    pub fn method_of_float() -> RefusedForm {
        ConstantForm::MethodOfFloat.into()
    }

    /// The form of this method of an integer, refused from more than one place.
    pub fn method_of_integer() -> RefusedForm {
        ConstantForm::MethodOfInteger.into()
    }

    /// The form of this operator on a float, refused from more than one place.
    pub fn operator_on_float() -> RefusedForm {
        ConstantForm::OperatorOnFloat.into()
    }

    /// The form of this operator on integers, refused from more than one place.
    pub fn operator_on_integers() -> RefusedForm {
        ConstantForm::OperatorOnIntegers.into()
    }

    /// The form of a text that is no `include_str!` or `const` of one, refused from more than one place.
    pub fn text_that_is_no_include_str_or() -> RefusedForm {
        ConstantForm::TextThatIsNoIncludeStrOr.into()
    }

    /// The form of a carrier whose value type has no value in the IR, refused from more than one place.
    pub fn carrier_whose_value_type_has_no_value() -> RefusedForm {
        DigitalForm::CarrierWhoseValueTypeHasNoValue.into()
    }

    /// The form of an initial value of a nested array of carriers other than `[x; K]`, refused from more than one place.
    pub fn initial_value_of_nested_array_of_carriers() -> RefusedForm {
        DigitalForm::InitialValueOfNestedArrayOfCarriers.into()
    }

    /// The form of a method of a generic impl whose parameters the receiver leaves open, refused from more than one place.
    pub fn method_of_generic_impl_whose_parameters_receiver() -> RefusedForm {
        GenericsForm::MethodOfGenericImplWhoseParametersReceiver.into()
    }

    /// The form of {} as a pattern, refused from more than one place.
    pub fn as_pattern() -> RefusedForm {
        PatternForm::AsPattern.into()
    }

    /// The form of an assignment to a local of type `Option`, refused from more than one place.
    pub fn assignment_to_local_of_type_option() -> RefusedForm {
        PatternForm::AssignmentToLocalOfTypeOption.into()
    }

    /// The form of an `if let` of `Option` values whose block is more than its value, refused from more than one place.
    pub fn if_let_of_option_values_whose_block() -> RefusedForm {
        PatternForm::IfLetOfOptionValuesWhoseBlock.into()
    }

    /// The form of an array of templates that is not an array, refused from more than one place.
    pub fn array_of_templates_that_is_not_array() -> RefusedForm {
        StructureForm::ArrayOfTemplatesThatIsNotArray.into()
    }

    /// The form of an array of templates that is not written out, held by a `let`, a `const` or a `param`, refused from more than one place.
    pub fn array_of_templates_that_is_not_written() -> RefusedForm {
        StructureForm::ArrayOfTemplatesThatIsNotWritten.into()
    }

    /// The form of a composite value of a type that has no value type, refused from more than one place.
    pub fn composite_value_of_type_that_has_no() -> RefusedForm {
        StructureForm::CompositeValueOfTypeThatHasNo.into()
    }

    /// The form of an index that is not an integer, refused from more than one place.
    pub fn index_that_is_not_integer() -> RefusedForm {
        StructureForm::IndexThatIsNotInteger.into()
    }

    /// The form of a `let` declared in a generation `if` that not every iteration of the `for` around it elaborates (a generated array with gaps), read as an array, refused from more than one place.
    pub fn let_declared_in_generation_if_that_not() -> RefusedForm {
        StructureForm::LetDeclaredInGenerationIfThatNot.into()
    }

    /// The form of an assignment to a name that is not a local, refused from more than one place.
    pub fn assignment_to_name_that_is_not_local() -> RefusedForm {
        ValueForm::AssignmentToNameThatIsNotLocal.into()
    }

    /// The form of a closure argument that is not a closure literal, refused from more than one place.
    pub fn closure_argument_that_is_not_closure_literal() -> RefusedForm {
        ValueForm::ClosureArgumentThatIsNotClosureLiteral.into()
    }

    /// The form of a closure parameter that is not a name, refused from more than one place.
    pub fn closure_parameter_that_is_not_name() -> RefusedForm {
        ValueForm::ClosureParameterThatIsNotName.into()
    }

    /// The form of a field of a child of a child, refused from more than one place.
    pub fn field_of_child_of_child() -> RefusedForm {
        ValueForm::FieldOfChildOfChild.into()
    }

    /// The form of a field of something that is not a name bound to a placement, refused from more than one place.
    pub fn field_of_something_that_is_not_name() -> RefusedForm {
        ValueForm::FieldOfSomethingThatIsNotName.into()
    }

    /// The form of a function returning an `Option` whose payload has no value in the IR, refused from more than one place.
    pub fn function_returning_option_whose_payload_has_no() -> RefusedForm {
        ValueForm::FunctionReturningOptionWhosePayloadHasNo.into()
    }

    /// The form of `is_empty` of a range with a missing bound, refused from more than one place.
    pub fn is_empty_of_range_with_missing_bound() -> RefusedForm {
        ValueForm::IsEmptyOfRangeWithMissingBound.into()
    }

    /// The form of a function without a body, refused from more than one place.
    pub fn lowered_function_without_body() -> RefusedForm {
        ValueForm::LoweredFunctionWithoutBody.into()
    }

    /// The form of method `{}`, refused from more than one place.
    pub fn method() -> RefusedForm {
        ValueForm::Method.into()
    }

    /// The form of a method outside the catalogue, refused from more than one place.
    pub fn method_outside_catalogue() -> RefusedForm {
        ValueForm::MethodOutsideCatalogue.into()
    }

    /// The form of a {} of a whole array of carriers or of a part of one: name an element, refused from more than one place.
    pub fn of_whole_array_of_carriers_or_of() -> RefusedForm {
        ValueForm::OfWholeArrayOfCarriersOrOf.into()
    }

    /// The form of an `Option` parameter whose payload has no value in the IR, refused from more than one place.
    pub fn option_parameter_whose_payload_has_no_value() -> RefusedForm {
        ValueForm::OptionParameterWhosePayloadHasNoValue.into()
    }

    /// The form of an `Option` whose payload has no zero in the IR, refused from more than one place.
    pub fn option_whose_payload_has_no_zero_in() -> RefusedForm {
        ValueForm::OptionWhosePayloadHasNoZeroIn.into()
    }

    /// The form of a range that is not written where it is used or bound by a `let`, refused from more than one place.
    pub fn range_that_is_not_written_where_it() -> RefusedForm {
        ValueForm::RangeThatIsNotWrittenWhereIt.into()
    }

    /// The form of a resolved carrier whose drive type has no value in the IR, refused from more than one place.
    pub fn resolved_carrier_whose_drive_type_has_no() -> RefusedForm {
        ValueForm::ResolvedCarrierWhoseDriveTypeHasNo.into()
    }

    /// The form of this statement, refused from more than one place.
    pub fn statement_that_failed_to_lower() -> RefusedForm {
        ValueForm::Statement.into()
    }

    /// The form of a `str` that is not a text after inlining, refused from more than one place.
    pub fn str_that_is_not_text_after_inlining() -> RefusedForm {
        ValueForm::StrThatIsNotTextAfterInlining.into()
    }

    /// The form of a struct expression that names neither a struct nor a variant, refused from more than one place.
    pub fn struct_expression_that_names_neither_struct_nor() -> RefusedForm {
        ValueForm::StructExpressionThatNamesNeitherStructNor.into()
    }

    /// The form of an unresolved name, refused from more than one place.
    pub fn unresolved_name() -> RefusedForm {
        ValueForm::UnresolvedName.into()
    }

    /// The form of a variant outside its enum, refused from more than one place.
    pub fn variant_outside_its_enum() -> RefusedForm {
        ValueForm::VariantOutsideItsEnum.into()
    }

    /// The form of `?` where the payload has no zero in the IR, refused from more than one place.
    pub fn where_payload_has_no_zero_in_ir() -> RefusedForm {
        ValueForm::WherePayloadHasNoZeroInIr.into()
    }
}
