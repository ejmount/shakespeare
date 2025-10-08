use convert_case::{Case, Casing};
use proc_macro2::Ident;
use quote::{ToTokens, format_ident};
use syn::{Path, PathSegment};

use super::MethodName;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RoleName(Path);

impl RoleName {
	pub(crate) fn new(p: Path) -> RoleName {
		debug_assert!(!p.segments.is_empty());
		RoleName(p)
	}

	pub(crate) fn path_leaf(&self) -> String {
		self.0.segments.last().as_ref().unwrap().ident.to_string()
	}

	pub(crate) fn queue_name(&self) -> Ident {
		format_ident!("{}", self.path_leaf().to_case(Case::Snake))
	}

	pub(crate) fn method_name(&self) -> MethodName {
		let role_name = self.path_leaf();
		format_ident!("{}", format!("perform_{role_name}").to_case(Case::Snake))
	}

	pub(crate) fn payload_path(&self) -> syn::Path {
		map_path_leaf(self.0.clone(), |data_name| {
			format_ident!("{}Payload", data_name)
		})
	}

	pub(crate) fn return_payload_path(&self) -> syn::Path {
		map_path_leaf(self.0.clone(), |p| format_ident!("{}ReturnPayload", p))
	}

	pub(crate) fn sender_method_name(&self) -> Ident {
		let field_name = self.queue_name();
		format_ident!("push_to_{field_name}")
	}
}

impl ToTokens for RoleName {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.0.to_tokens(tokens);
	}
}

fn map_path_leaf<F>(mut p: Path, f: F) -> Path
where
	F: Fn(Ident) -> Ident,
{
	debug_assert!(!p.segments.is_empty());
	let leaf = p.segments.pop().unwrap().into_value();
	let new_leaf = PathSegment {
		ident: f(leaf.ident),
		..leaf
	};
	p.segments.push(new_leaf);
	p
}
