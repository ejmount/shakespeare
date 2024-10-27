use quote::ToTokens;
use syn::{Item, Result};

use crate::data::ActorName;
use crate::macros::fallible_quote;

#[derive(Debug)]
pub(crate) struct SelfGetter {
	actor_getter: Item,
}

impl SelfGetter {
	pub(crate) fn new(actor_name: &ActorName) -> Result<SelfGetter> {
		let actor_getter: Item = fallible_quote! {
			impl #actor_name {
				#[doc(hidden)]
				// Used internally for creating Envelopes
				pub fn get_shell(&self) -> ::std::sync::Arc<#actor_name> {
					self.this.upgrade().expect("Dead actor?")
				}
			}
		}?;

		Ok(SelfGetter { actor_getter })
	}
}
impl ToTokens for SelfGetter {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.actor_getter.to_tokens(tokens);
	}
}
