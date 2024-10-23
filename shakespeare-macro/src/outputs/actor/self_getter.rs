use quote::ToTokens;
use syn::{Item, Result};

use crate::data::{ActorName, DataName};
use crate::macros::fallible_quote;

#[derive(Debug)]
pub(crate) struct SelfGetter {
	//	statik_item:  Item,
	data_getter:  Item,
	actor_getter: Item,
}

impl SelfGetter {
	pub(crate) fn new(actor_name: &ActorName, data_name: &DataName) -> Result<SelfGetter> {
		let getter_item = actor_name.get_static_item_name();

		/*let statik_item: Item = fallible_quote! {
			/* ::shakespeare::tokio_export::task_local! {
				#[doc(hidden)]
				//static #getter_item: ::shakespeare::Context<#actor_name>;
			}*/
		}?;*/

		let data_getter: Item = fallible_quote! {
			impl #data_name {
				/// Gets a reference to the running actor
				///
				/// # Panics
				/// Will panic if called from outside a performance of an actor of the appropriate type. (However, which instance it's called on doesn't matter.)
				///
				#[allow(dead_code)]
				pub fn get_context(&self) -> ::shakespeare::Context<#data_name> {
					//#getter_item.with(Clone::clone)
					unimplemented!()
				}
			}
		}?;

		let actor_getter: Item = fallible_quote! {
			impl #actor_name {
				#[doc(hidden)]
				// Used internally for creating Envelopes
				pub fn get_shell(&self) -> ::std::sync::Arc<#actor_name> {
					self.this.upgrade().expect("Dead actor?")
				}
			}
		}?;

		Ok(SelfGetter {
			//			statik_item,
			data_getter,
			actor_getter,
		})
	}
}
impl ToTokens for SelfGetter {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		//self.statik_item.to_tokens(tokens);
		self.data_getter.to_tokens(tokens);
		self.actor_getter.to_tokens(tokens);
	}
}
