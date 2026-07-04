//! Type and net-reference resolution: `Type` (parse AST) → `TypeRef` (POM),
//! net-connection expressions → `NetRef`, and port expansion (a bundle port
//! fans out into one `Port` per net-capable field).

use std::collections::HashMap;

use crate::parse::ast::{Expr, Port as AstPort, Type};
use crate::elab::const_eval::ConstEnv;
use crate::pom::{ElabError, ElabErrorKind, NetRef, NetType, Port, TypeRef, ValueType};

use super::Elaborator;

impl Elaborator {
    // ─────────────────────────── Type resolution ──────────────────────────────

    /// Resolves a parse-AST `Type` to a POM `TypeRef`. Handles array
    /// dimensions recursively, value primitives (`Real`, `Natural`, …),
    /// discipline names (→ `NetType::Discipline`), enum names, bundles
    /// (→ `NetType::Discipline` if net-capable), and `fn` function-pointer
    /// types. Falls back to `UndefinedType` on failure.
    pub(crate) fn resolve_type(
        &self,
        ty: &Type,
        env: &ConstEnv,
        type_subst: &HashMap<String, String>,
    ) -> Result<TypeRef, ElabError> {
        let name = type_subst.get(&ty.name).map(|s| s.as_str()).unwrap_or(&ty.name);

        if !ty.dimensions.is_empty() {
            let inner_ty =
                Type { name: ty.name.clone(), args: ty.args.clone(), dimensions: vec![] };
            let inner = self.resolve_type(&inner_ty, env, type_subst)?;
            let mut result = inner;
            for dim_expr in &ty.dimensions {
                let n = env.eval_nat(dim_expr).map_err(|e| ElabError::from(ElabErrorKind::ConstEval {
                    context: format!("array dimension of type `{}`", ty.name),
                    source: e,
                }))?;
                result = match result {
                    TypeRef::Net(nt) => TypeRef::Net(NetType::Array(Box::new(nt), n)),
                    TypeRef::Value(vt) => TypeRef::Value(ValueType::Array(Box::new(vt), n)),
                };
            }
            return Ok(result);
        }

        if let Some(def) = self.ctx.types.lookup(name) {
            if def.as_bundle().is_some() {
                return if self.is_net_capable_bundle(name) {
                    Ok(TypeRef::Net(NetType::Discipline(name.to_owned())))
                } else {
                    // A value bundle used as a type (fn param, method self,
                    // …) — the consumer flattens it per-field or fails loud.
                    Ok(TypeRef::Value(ValueType::Bundle(name.to_owned())))
                };
            }
            return def.resolve(ty, env, type_subst);
        }

        if name == "fn" {
            let params_and_ret = ty
                .args
                .iter()
                .map(|a| self.resolve_type(a, env, type_subst))
                .collect::<Result<Vec<_>, _>>()?;
            let (ret, params) = params_and_ret
                .split_last()
                .ok_or_else(|| {
                    ElabError::from(ElabErrorKind::UndefinedType("fn type requires a return type".to_owned()))
                })?;
            return Ok(TypeRef::Value(ValueType::FnPtr(
                params.to_vec(),
                Box::new(ret.clone()),
            )));
        }

        Err(ElabError::from(ElabErrorKind::UndefinedType(name.to_owned())))
    }

    /// Resolves a type and returns a `NetType`, failing with
    /// `NotNetCapable` if the resolved type is a value type.
    pub(crate) fn resolve_net_type(
        &self,
        ty: &Type,
        env: &ConstEnv,
        type_subst: &HashMap<String, String>,
    ) -> Result<NetType, ElabError> {
        match self.resolve_type(ty, env, type_subst)? {
            TypeRef::Net(nt) => Ok(nt),
            _ => Err(ElabError::from(ElabErrorKind::NotNetCapable(ty.name.clone()))),
        }
    }

    /// Resolves a type and returns a `ValueType`, failing if the resolved
    /// type is a net type.
    pub(crate) fn resolve_value_type(&self, ty: &Type, env: &ConstEnv) -> Result<ValueType, ElabError> {
        match self.resolve_type(ty, env, &HashMap::new())? {
            TypeRef::Value(vt) => Ok(vt),
            // A storage discipline names the value it carries: `var st :
            // Bit` is a `Boolean` var (SPEC §7.2 / B.1). Arrays likewise.
            TypeRef::Net(nt) => self.net_storage_value_type(&nt)?.ok_or_else(|| {
                ElabError::from(ElabErrorKind::Other(format!("expected value type, found net type `{nt:?}`")))
            }),
        }
    }

    /// The value type a net type carries, if it is a storage discipline
    /// (or an array of one). Conservative disciplines carry no single value.
    fn net_storage_value_type(&self, nt: &NetType) -> Result<Option<ValueType>, ElabError> {
        match nt {
            NetType::Discipline(name) => self.storage_value_type(name),
            NetType::Array(inner, n) => Ok(self
                .net_storage_value_type(inner)?
                .map(|vt| ValueType::Array(Box::new(vt), *n))),
        }
    }

    /// Returns `true` if the named bundle has only net-capable fields,
    /// making the bundle itself suitable as a net type (i.e. a bundle of
    /// disciplines or net-capable sub-bundles).
    pub(crate) fn is_net_capable_bundle(&self, name: &str) -> bool {
        let Some(bundle) = self.bundles.get(name) else { return false };
        bundle.fields.iter().all(|f| self.is_net_type_name(&f.ty.name))
    }

    /// Returns `true` if the name refers to a known discipline or a
    /// net-capable bundle, i.e. the name denotes a net type.
    pub(crate) fn is_net_type_name(&self, name: &str) -> bool {
        self.disciplines.contains_key(name) || self.is_net_capable_bundle(name)
    }

    /// Returns the storage value type of a named storage discipline, or
    /// `None` if the discipline is conservative (has potential/flow but no
    /// `storage` clause). Used to recover the value type of a `var` whose
    /// source type is a storage discipline (e.g. `var st : Bit = 0;` →
    /// `Bit` is `storage Boolean`, so the var's value type is `Boolean`).
    pub(crate) fn storage_value_type(
        &self,
        discipline_name: &str,
    ) -> Result<Option<ValueType>, ElabError> {
        let Some(decl) = self.disciplines.get(discipline_name) else {
            return Ok(None);
        };
        for item in &decl.items {
            if let crate::parse::ast::DisciplineItem::Storage(ty) = item {
                let vt = self.resolve_value_type(ty, &ConstEnv::new())?;
                return Ok(Some(vt));
            }
        }
        Ok(None)
    }

    // ─────────────────────────── Net reference ────────────────────────────────

    /// Reduce a port-connection or net-connection expression to a concrete
    /// `NetRef`. Supported forms:
    ///
    /// - `name` → `NetRef::simple(name)`
    /// - `name[i]` — `i` evaluated via `env` → `NetRef::indexed(name, i)`
    /// - `base.field` → `NetRef::simple("{base}_{field}")` (bundle-field naming)
    pub(crate) fn eval_net_ref(&self, expr: &Expr, env: &ConstEnv) -> Result<NetRef, ElabError> {
        match expr {
            Expr::Ident(name) => Ok(NetRef::simple(name)),
            Expr::Index(base, idx) => {
                let base_name = match base.as_ref() {
                    Expr::Ident(n) => n.clone(),
                    other => {
                        return Err(ElabError::from(ElabErrorKind::NotANetRef(format!(
                            "indexed net ref base must be an identifier, got `{:?}`",
                            other
                        ))))
                    }
                };
                let i = env.eval_nat(idx).map_err(|e| ElabError::from(ElabErrorKind::ConstEval {
                    context: format!("net ref index on `{}`", base_name),
                    source: e,
                }))?;
                Ok(NetRef::indexed(base_name, i))
            }
            Expr::Field(base, field) => {
                let base_name = match base.as_ref() {
                    Expr::Ident(n) => n.clone(),
                    other => {
                        return Err(ElabError::from(ElabErrorKind::NotANetRef(format!(
                            "field net ref base must be an identifier, got `{:?}`",
                            other
                        ))))
                    }
                };
                Ok(NetRef::simple(format!("{}_{}", base_name, field)))
            }
            other => Err(ElabError::from(ElabErrorKind::NotANetRef(format!(
                "expected net reference (identifier, index, or field), got `{:?}`",
                other
            )))),
        }
    }

    // ─────────────────────────── Port expansion ───────────────────────────────

    /// Expands a single AST port into one or more POM `Port`s. If the
    /// port's type is a net-capable bundle, each bundle field becomes a
    /// separate port with a compound name (`port_field`). Otherwise a
    /// single port is returned with the resolved net type.
    pub(crate) fn expand_port(
        &self,
        port: &AstPort,
        env: &ConstEnv,
        type_subst: &HashMap<String, String>,
    ) -> Result<Vec<Port>, ElabError> {
        let resolved_name =
            type_subst.get(&port.ty.name).map(|s| s.as_str()).unwrap_or(&port.ty.name);

        if let Some(bundle) = self.bundles.get(resolved_name).cloned() {
            if !self.is_net_capable_bundle(resolved_name) {
                return Err(ElabError::from(ElabErrorKind::NotNetCapable(resolved_name.to_owned())));
            }
            let mut out = Vec::new();
            for field in &bundle.fields {
                let field_ty = self.resolve_net_type(&field.ty, env, type_subst)?;
                out.push(Port {
                    span: None,
                    attributes: Vec::new(),
                    direction: port.direction.clone(),
                    name: format!("{}_{}", port.name, field.name),
                    ty: field_ty,
                });
            }
            return Ok(out);
        }

        let net_ty = self.resolve_net_type(&port.ty, env, type_subst)?;
        Ok(vec![Port {
            span: None,
            attributes: Vec::new(),
            direction: port.direction.clone(),
            name: port.name.clone(),
            ty: net_ty,
        }])
    }

}
