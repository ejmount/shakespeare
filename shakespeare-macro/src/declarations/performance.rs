use convert_case::{Case, Casing};
use itertools::Itertools;
use quote::format_ident;
use syn::{Error, Generics, Ident, ImplItem, ImplItemFn, ItemImpl, Path, Result};

use crate::data::{FunctionItem, RoleName};
use crate::macros::filter_unwrap;

#[derive(structmeta::StructMeta)]
pub struct PerformanceAttribute {
	pub canonical: bool,
}

pub struct PerformanceDeclaration {
	pub role_name: RoleName,
	pub handlers:  Vec<FunctionItem>,
}

impl PerformanceDeclaration {
	pub fn new(role_name: Path, imp: ItemImpl) -> Result<PerformanceDeclaration> {
		assert!(!role_name.segments.is_empty());

		let handlers = filter_unwrap!(imp.items, ImplItem::Fn).collect_vec();
		let nongeneric = Generics::default();
		for handler in &handlers {
			if handler.sig.generics != nongeneric {
				Err(Error::new_spanned(
					&handler.sig,
					"Generic performances are not supported",
				))?;
			}
		}

		let role_name = RoleName::new(role_name);

		Ok(PerformanceDeclaration {
			role_name,
			handlers,
		})
	}

	pub fn get_role_name(&self) -> &RoleName {
		&self.role_name
	}
}

pub fn make_variant_name(function: &ImplItemFn) -> Ident {
	let name = function.sig.ident.to_string();
	format_ident!("{}", name.to_case(Case::UpperCamel))
}
