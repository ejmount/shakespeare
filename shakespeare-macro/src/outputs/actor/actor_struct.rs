use itertools::Itertools;
use quote::{quote, ToTokens};
use syn::parse::Parser;
use syn::{
	Field, ImplItem, ItemFn, ItemImpl, ItemStruct, Result, ReturnType, Signature, Visibility,
};

use crate::data::{ActorName, DataName, RoleName};
use crate::declarations::{ActorDecl, PerformanceDecl};
use crate::macros::{fallible_quote, map_or_bail};

#[derive(Debug)]
pub(crate) struct ActorStruct {
	strukt:                  ItemStruct,
	sender_method_name_impl: ItemImpl,
	meta_traits:             [ItemImpl; 2],
}

impl ActorStruct {
	pub(crate) fn new(actor: &ActorDecl) -> Result<ActorStruct> {
		let ActorDecl {
			actor_name,
			performances,
			actor_vis,
			panic_handler,
			exit_handler,
			data_item,
			..
		} = actor;

		let fields = map_or_bail!(performances, shell_field_from_performance);

		let strukt = fallible_quote! {
			#actor_vis struct #actor_name {
				this: ::std::sync::Weak<Self>,
				#(#fields),*
			}
		}?;

		let role_names = performances
			.iter()
			.map(PerformanceDecl::get_role_name)
			.collect_vec();

		let sender_method_name_impl = create_inherent_impl(&role_names, actor_vis, actor_name)?;

		let meta_trait = create_meta_trait_impl(
			panic_handler.as_ref(),
			exit_handler.as_ref(),
			actor_name,
			&data_item.name(),
		)?;

		Ok(ActorStruct {
			strukt,
			sender_method_name_impl,
			meta_traits: meta_trait,
		})
	}
}

impl ToTokens for ActorStruct {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.strukt.to_tokens(tokens);
		self.meta_traits.each_ref().map(|f| f.to_tokens(tokens));
		self.sender_method_name_impl.to_tokens(tokens);
	}
}

fn create_meta_trait_impl(
	panic_handler: Option<&ItemFn>,
	exit_handler: Option<&ItemFn>,
	actor_name: &ActorName,
	data_name: &DataName,
) -> Result<[ItemImpl; 2]> {
	let unit_type = fallible_quote!(())?;
	let panic_type = fallible_quote!(std::boxed::Box<dyn std::any::Any + std::marker::Send>)?;

	let panic_return = match panic_handler.as_ref() {
		Some(ItemFn {
			sig: Signature {
				output: ReturnType::Type(_, b),
				..
			},
			..
		}) => &**b,
		Some(_) => &unit_type,
		None => &panic_type,
	};
	let exit_return = if let Some(ItemFn {
		sig: Signature {
			output: ReturnType::Type(_, b),
			..
		},
		..
	}) = exit_handler.as_ref()
	{
		&**b
	} else {
		&unit_type
	};

	let actor_trait = fallible_quote! {
		impl ::shakespeare::ActorShell for #actor_name {
			type StateType = #data_name;
			type ExitType = #exit_return;
			type PanicType = #panic_return;
		}
	}?;

	let state_trait = fallible_quote! {
		impl ::shakespeare::ActorState for #data_name {
			type ShellType = #actor_name;
		}
	}?;

	Ok([actor_trait, state_trait])
}

fn create_inherent_impl(
	role_names: &Vec<&RoleName>,
	actor_vis: &Visibility,
	actor_name: &ActorName,
) -> Result<ItemImpl> {
	let make_methods = |role_name: &&RoleName| -> Result<ImplItem> {
		let field_name = role_name.queue_name();
		let acccessor_name = role_name.sender_method_name();

		fallible_quote! {
			#actor_vis async fn #acccessor_name(&self, payload: ::shakespeare::ReturnEnvelope<dyn #role_name>) -> Result<(), ::shakespeare::Role2SendError<dyn #role_name>>
			{
				self.#field_name.send(payload)
			}
		}
	};

	let methods = map_or_bail!(role_names, make_methods);

	fallible_quote! {
		impl #actor_name {
			#(#methods)*
		}
	}
}

fn shell_field_from_performance(perf: &PerformanceDecl) -> Result<Field> {
	let role_name = &perf.role_name;
	let field_name = role_name.queue_name();

	Field::parse_named
		.parse2(quote! {#field_name : shakespeare::Role2Sender<dyn #role_name> })
		.map_err(|err| {
			syn::parse::Error::new(err.span(),
				format!("Parse failure trying to create actor field: {err} - this is a bug, please file an issue")
			)
		})
}
