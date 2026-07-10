//! Attribute validation pass — validates `@SchemaName(...)` attributes against
//! registered bundle schemas and converts them to POM `Attribute` nodes.

use std::collections::HashMap;

use crate::elab::registry::SchemaRegistry;
use crate::parse::ast::{Attribute, Expr, Literal};
use crate::pom::{ElabError, ElabErrorKind};
use crate::value::Value;

/// Convert an AST `Attribute` to a POM `Attribute`, validating against the
/// registered schema. If the schema name is not registered, returns
/// `UnknownAttrSchema`.
pub(crate) fn convert_attribute(
    attr: &Attribute,
    schemas: &SchemaRegistry,
    bundles: &HashMap<String, crate::parse::ast::BundleDecl>,
) -> Result<crate::pom::module::Attribute, ElabError> {
    let bundle_name = schemas.lookup(&attr.name).ok_or_else(|| {
        ElabError::from(ElabErrorKind::UnknownAttrSchema(attr.name.clone()))
    })?;
    let bundle = bundles.get(bundle_name).ok_or_else(|| {
        ElabError::from(ElabErrorKind::UnknownAttrSchema(attr.name.clone()))
    })?;
    let mut data: HashMap<String, Value> = HashMap::new();
    for arg in &attr.args {
        // The field must exist in the bundle.
        let field = bundle.fields.iter().find(|f| f.name == arg.name).ok_or_else(|| {
            ElabError::from(ElabErrorKind::AttrSchemaField {
                schema: attr.name.clone(),
                field: arg.name.clone(),
                reason: "not a field of this bundle".into(),
            })
        })?;
        // Evaluate the argument expression to a Value.
        let value = eval_attr_value(&arg.expr).map_err(|reason| {
            ElabError::from(ElabErrorKind::AttrSchemaField {
                schema: attr.name.clone(),
                field: arg.name.clone(),
                reason,
            })
        })?;
        // Basic type check: field type name must match value type.
        check_field_type(&field.ty.name, &value).map_err(|reason| {
            ElabError::from(ElabErrorKind::AttrSchemaField {
                schema: attr.name.clone(),
                field: arg.name.clone(),
                reason,
            })
        })?;
        data.insert(arg.name.clone(), value);
    }
    // Required fields (no default) must be provided.
    for field in &bundle.fields {
        if field.default.is_none() && !data.contains_key(&field.name) {
            return Err(ElabError::from(ElabErrorKind::AttrSchemaField {
                schema: attr.name.clone(),
                field: field.name.clone(),
                reason: "required field not provided".into(),
            }));
        }
    }
    // Fill in defaults for omitted fields that have a default.
    for field in &bundle.fields {
        if !data.contains_key(&field.name)
            && let Some(default_expr) = &field.default {
                let value = eval_attr_value(default_expr).map_err(|reason| {
                    ElabError::from(ElabErrorKind::AttrSchemaField {
                        schema: attr.name.clone(),
                        field: field.name.clone(),
                        reason,
                    })
                })?;
                data.insert(field.name.clone(), value);
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
