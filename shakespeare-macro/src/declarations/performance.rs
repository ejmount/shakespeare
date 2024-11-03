use convert_case::{Case, Casing};
use itertools::Itertools;
use quote::format_ident;
use structmeta::{Flag, StructMeta};
use syn::{Error, Ident, ImplItem, ItemImpl, Path, Result};

use crate::data::{FunctionItem, RoleName};
use crate::macros::filter_unwrap;

#[derive(StructMeta)]
pub(crate) struct PerformanceAttribute {
	pub(crate) canonical: Flag,
}

pub(crate) struct PerformanceDecl {
	pub(crate) role_name: RoleName,
	pub(crate) handlers:  Vec<FunctionItem>,
}

impl PerformanceDecl {
	pub(crate) fn new(role_name: Path, imp: ItemImpl) -> Result<PerformanceDecl> {
		assert!(!role_name.segments.is_empty());

		let handlers = filter_unwrap!(imp.items, ImplItem::Fn).collect_vec();
		for handler in &handlers {
			if handler.sig.generics.type_params().next().is_some() {
				Err(Error::new_spanned(
					&handler.sig,
					"Generic performances are not supported",
				))?;
			}
			if !matches!(handler.sig.inputs.first(), Some(syn::FnArg::Receiver(_))) {
				Err(Error::new_spanned(
					&handler.sig,
					"Performance method must have self-receiver",
				))?;
			}
		}

		let role_name = RoleName::new(role_name);

		Ok(PerformanceDecl {
			role_name,
			handlers,
		})
	}

	pub(crate) fn get_role_name(&self) -> &RoleName {
		&self.role_name
	}
}
