use itertools::Itertools;
use quote::{quote, ToTokens};
use syn::parse::Parser;
use syn::{Field, ImplItemFn, ItemFn, ItemImpl, ItemStruct, Path, Result, Visibility};

use crate::data::{ActorName, RoleName};
use crate::declarations::actor::ActorDecl;
use crate::declarations::performance::PerformanceDecl;
use crate::macros::{fallible_quote, map_or_bail};

#[derive(Debug)]
pub struct ActorStruct {
	strukt:                  ItemStruct,
	sender_method_name_impl: ItemImpl,
	meta_trait:              ItemImpl,
}

impl ActorStruct {
	pub fn new(actor: &ActorDecl) -> Result<ActorStruct> {
		let ActorDecl {
			actor_name,
			performances,
			actor_vis,
			panic_handler,
			exit_handler,
			..
		} = actor;

		let fields = map_or_bail!(performances, make_field_from_name);

		let strukt = fallible_quote! {
			#[derive(Clone)]
			#actor_vis struct #actor_name {
				#(#fields),*
			}
		}?;

		let role_names = performances
			.iter()
			.map(PerformanceDecl::get_role_name)
			.collect_vec();

		let sender_method_name_impl = create_inherent_impl(&role_names, actor_vis, actor_name)?;

		let meta_trait = create_meta_trait_impl(panic_handler, exit_handler, actor_name)?;

		Ok(ActorStruct {
			strukt,
			sender_method_name_impl,
			meta_trait,
		})
	}
}

impl ToTokens for ActorStruct {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.strukt.to_tokens(tokens);
		self.meta_trait.to_tokens(tokens);
		self.sender_method_name_impl.to_tokens(tokens);
	}
}

fn create_meta_trait_impl(
	panic_handler: &Option<ItemFn>,
	exit_handler: &Option<ItemFn>,
	actor_name: &ActorName,
) -> Result<ItemImpl> {
	let unit_type = fallible_quote!(()).unwrap();
	let panic_type =
		fallible_quote!(std::boxed::Box<dyn std::any::Any + std::marker::Send>).unwrap();
	let panic_return = panic_handler.as_ref().map_or(&panic_type, |f| {
		if let syn::ReturnType::Type(_, ref b) = f.sig.output {
			&**b
		} else {
			&unit_type
		}
	});
	let exit_return = exit_handler.as_ref().map_or(&unit_type, |f| {
		if let syn::ReturnType::Type(_, ref b) = f.sig.output {
			&**b
		} else {
			&unit_type
		}
	});
	fallible_quote! {
		impl ::shakespeare::ActorShell for #actor_name {
			type ExitType = #exit_return;
			type PanicType = #panic_return;
		}
	}
}

fn create_inherent_impl(
	role_names: &Vec<&RoleName>,
	actor_vis: &Visibility,
	actor_name: &ActorName,
) -> Result<ItemImpl> {
	fn sender_method_name_from_name(role_name: &RoleName, vis: &Visibility) -> Result<ImplItemFn> {
		let error_path: Path = fallible_quote! { shakespeare::Role2SendError<dyn #role_name> }?;
		let payload_path = role_name.payload_path();
		let field_name = role_name.queue_name();

		let acccessor_name = role_name.sender_method_name();

		fallible_quote! {
			#vis async fn #acccessor_name(&self, payload: #payload_path) -> Result<(), #error_path>
			{
				self.#field_name.send(payload)
			}
		}
	}
	fn sender_getter_from_name(role_name: &RoleName, vis: &Visibility) -> Result<ImplItemFn> {
		let field_name = role_name.queue_name();
		let getter_name = role_name.sender_getter_name();

		fallible_quote! {
			#vis fn #getter_name(&self) -> &::shakespeare::Role2Sender<dyn #role_name>
			{
				&self.#field_name
			}
		}
	}

	let sender_method_names = map_or_bail!(&role_names, |name| sender_method_name_from_name(
		name, actor_vis
	));
	let getters = map_or_bail!(&role_names, |name| sender_getter_from_name(name, actor_vis));
	fallible_quote! {
		impl #actor_name {
			#(#sender_method_names)*
			#(#getters)*
		}
	}
}

fn make_field_from_name(perf: &PerformanceDecl) -> Result<Field> {
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
