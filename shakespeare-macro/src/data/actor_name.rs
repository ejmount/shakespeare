use convert_case::{Case, Casing};
use quote::{format_ident, ToTokens};
use syn::{Ident, Path};

pub(crate) struct ActorName(Path);

impl ActorName {
	pub(crate) fn new(p: Path) -> ActorName {
		ActorName(p)
	}

	pub(crate) fn get_static_item_name(&self) -> Ident {
		let ident = format!("{}Ref", self.0.segments.last().unwrap().ident);
		format_ident!("{}", ident.to_case(Case::ScreamingSnake))
	}
}

impl ToTokens for ActorName {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.0.to_tokens(tokens);
	}
}
