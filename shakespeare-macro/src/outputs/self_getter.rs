use quote::ToTokens;
use syn::{Item, Result};

use crate::data::{ActorName, DataName};
use crate::macros::fallible_quote;

#[derive(Debug)]
pub(crate) struct SelfGetter {
	statik_item: Item,
	getter:      Item,
}

impl SelfGetter {
	pub fn new(actor_name: &ActorName, data_name: &DataName) -> Result<SelfGetter> {
		let getter_item = actor_name.get_static_item_name();

		let statik_item: Item = fallible_quote! {
			::shakespeare::tokio_export::task_local! {
				static #getter_item: ::std::sync::Arc<#actor_name>;
			}
		}?;

		let getter: Item = fallible_quote! {
			impl #data_name {
				pub fn get_shell() -> ::std::sync::Arc<#actor_name> {
					#getter_item.with(Clone::clone)
				}
			}
		}?;

		Ok(SelfGetter {
			statik_item,
			getter,
		})
	}
}
impl ToTokens for SelfGetter {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.statik_item.to_tokens(tokens);
		self.getter.to_tokens(tokens);
	}
}
