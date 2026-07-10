//! Attribute validation pass — validates `@SchemaName(...)` attributes against
//! registered bundle schemas and converts them to POM `Attribute` nodes.

use std::collections::HashMap;

use crate::elab::registry::SchemaRegistry;
use crate::parse::ast::{Attribute, Expr, Literal};
use crate::pom::{ElabError, ElabErrorKind};
use crate::value::Value;

/// What happens when a schema field is omitted at a use site.
enum FieldOmit {
    /// Omission is an error.
    Required,
    /// Omission is fine; the field is simply absent from the data.
    Optional,
    /// Omission fills in this default value.
    Default(Result<Value, String>),
}

/// One schema field, unified across the two shape kinds ([`SchemaShape`]).
struct FieldSpec {
    name: String,
    ty: String,
    omit: FieldOmit,
}

/// The unified field list a schema validates against.
fn schema_fields(
    attr_name: &str,
    schemas: &SchemaRegistry,
    bundles: &HashMap<String, crate::parse::ast::BundleDecl>,
) -> Result<Vec<FieldSpec>, ElabError> {
    use crate::elab::registry::SchemaShape;
    let unknown = || ElabError::from(ElabErrorKind::UnknownAttrSchema(attr_name.to_string()));
    match schemas.shape(attr_name).ok_or_else(unknown)? {
        SchemaShape::Bundle(bundle_name) => {
            let bundle = bundles.get(bundle_name).ok_or_else(unknown)?;
            Ok(bundle
                .fields
                .iter()
                .map(|f| FieldSpec {
                    name: f.name.clone(),
                    ty: f.ty.name.clone(),
                    omit: match &f.default {
                        Some(e) => FieldOmit::Default(eval_attr_value(e)),
                        None => FieldOmit::Required,
                    },
                })
                .collect())
        }
        SchemaShape::Declared(fields) => Ok(fields
            .iter()
            .map(|f| FieldSpec {
                name: f.name.clone(),
                ty: f.ty.clone(),
                omit: match (&f.required, &f.default) {
                    (true, _) => FieldOmit::Required,
                    (false, Some(v)) => FieldOmit::Default(Ok(v.clone())),
                    (false, None) => FieldOmit::Optional,
                },
            })
            .collect()),
    }
}

/// Convert an AST `Attribute` to a POM `Attribute`, validating against the
/// registered schema. If the schema name is not registered, returns
/// `UnknownAttrSchema`.
pub(crate) fn convert_attribute(
    attr: &Attribute,
    schemas: &SchemaRegistry,
    bundles: &HashMap<String, crate::parse::ast::BundleDecl>,
) -> Result<crate::pom::module::Attribute, ElabError> {
    let fields = schema_fields(&attr.name, schemas, bundles)?;
    let field_err = |field: &str, reason: String| {
        ElabError::from(ElabErrorKind::AttrSchemaField {
            schema: attr.name.clone(),
            field: field.to_string(),
            reason,
        })
    };
    let mut data: HashMap<String, Value> = HashMap::new();
    for arg in &attr.args {
        // The field must exist in the schema.
        let field = fields
            .iter()
            .find(|f| f.name == arg.name)
            .ok_or_else(|| field_err(&arg.name, "not a field of this schema".into()))?;
        // Evaluate the argument expression to a Value and type-check it.
        let value = eval_attr_value(&arg.expr).map_err(|reason| field_err(&arg.name, reason))?;
        check_field_type(&field.ty, &value).map_err(|reason| field_err(&arg.name, reason))?;
        data.insert(arg.name.clone(), value);
    }
    // Required fields must be provided; omitted fields with a default take
    // it; optional fields stay absent.
    for field in &fields {
        if data.contains_key(&field.name) {
            continue;
        }
        match &field.omit {
            FieldOmit::Required => {
                return Err(field_err(&field.name, "required field not provided".into()));
            }
            FieldOmit::Optional => {}
            FieldOmit::Default(default) => {
                let value = default.clone().map_err(|reason| field_err(&field.name, reason))?;
                data.insert(field.name.clone(), value);
            }
        }
    }
    Ok(crate::pom::module::Attribute { schema: attr.name.clone(), data })
}

/// Convert a list of AST attributes to POM attributes.
pub(crate) fn convert_attributes(
    attrs: &[Attribute],
    schemas: &SchemaRegistry,
    bundles: &HashMap<String, crate::parse::ast::BundleDecl>,
) -> Result<Vec<crate::pom::module::Attribute>, ElabError> {
    attrs.iter().map(|a| convert_attribute(a, schemas, bundles)).collect()
}

/// Evaluate a simple literal expression to a Value (attribute values must be
/// compile-time constants).
fn eval_attr_value(expr: &Expr) -> Result<Value, String> {
    match expr {
        Expr::Literal(Literal::Real(v)) => Ok(Value::Real(*v)),
        Expr::Literal(Literal::Int(v)) => Ok(Value::Nat(*v)),
        Expr::Literal(Literal::Bool(v)) => Ok(Value::Bool(*v)),
        Expr::Literal(Literal::String(s)) => Ok(Value::Str(s.clone())),
        _ => Err(format!("attribute value must be a literal, got {:?}", expr)),
    }
}

/// Check that a value matches a field's declared type.
fn check_field_type(type_name: &str, value: &Value) -> Result<(), String> {
    let ok = match (type_name, value) {
        ("Real", Value::Real(_)) => true,
        ("Real", Value::Nat(_)) => true, // Nat widens to Real
        ("Natural", Value::Nat(_)) => true,
        ("Integer", Value::Nat(_)) => true,
        ("Integer", Value::Int(_)) => true,
        ("Boolean", Value::Bool(_)) => true,
        ("String", Value::Str(_)) => true,
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(format!("expected type `{}`, got {}", type_name, value.type_name()))
    }
}
