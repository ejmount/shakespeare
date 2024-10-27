use quote::ToTokens;
use syn::TypePath;

pub(crate) struct ActorName(TypePath);

impl ActorName {
	pub(crate) fn new(p: TypePath) -> ActorName {
		ActorName(p)
	}
}

impl ToTokens for ActorName {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.0.to_tokens(tokens);
	}
}
