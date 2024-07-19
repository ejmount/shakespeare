use quote::ToTokens;
use syn::{parse_quote, TypePath};

use super::ActorName;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DataName(TypePath);

impl DataName {
	pub(crate) fn new(p: TypePath) -> Self {
		debug_assert!(!p.path.segments.is_empty());
		Self(p)
	}

	pub(crate) fn get_shell_type_path(&self) -> ActorName {
		let path = &self.0;
		ActorName::new(parse_quote! {
			<#path as ::shakespeare::ActorState>::ShellType
		})
	}
}

impl ToTokens for DataName {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.0.to_tokens(tokens);
	}
}
