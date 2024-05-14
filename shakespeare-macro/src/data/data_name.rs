use quote::{format_ident, ToTokens};
use syn::Path;

use super::ActorName;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DataName(Path);

impl DataName {
	pub(crate) fn new(p: Path) -> Self {
		debug_assert!(!p.segments.is_empty());
		Self(p)
	}

	pub(crate) fn actor_path(&self) -> ActorName {
		ActorName::new(super::update_path_leaf(self.0.clone(), |data_name| {
			format_ident!("{}Actor", data_name)
		}))
	}
}

impl ToTokens for DataName {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.0.to_tokens(tokens);
	}
}
