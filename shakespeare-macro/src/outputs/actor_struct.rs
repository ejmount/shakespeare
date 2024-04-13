use quote::{quote, ToTokens};
use syn::parse::Parser;
use syn::{Field, ImplItemFn, ItemImpl, ItemStruct, Path, Result, Visibility};

use crate::data::RoleName;
use crate::declarations::actor::ActorDecl;
use crate::declarations::performance::PerformanceDecl;
use crate::macros::{fallible_quote, map_or_bail};

#[derive(Debug)]
pub struct ActorStruct {
	strukt:     ItemStruct,
	accesors:   ItemImpl,
	meta_trait: ItemImpl,
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

		let accessors = map_or_bail!(performances, |perf| make_accessor_from_name(
			perf.get_role_name(),
			actor_vis
		));

		let getters = map_or_bail!(performances, |perf| make_sender_getter_from_name(
			perf.get_role_name(),
			actor_vis
		));

		let strukt = fallible_quote! {
			#actor_vis struct #actor_name {
				#(#fields),*
			}
		}?;

		let accessor_impl = fallible_quote! {
			impl #actor_name {
				#(#accessors)*
				#(#getters)*
			}
		}?;

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

		let meta_trait = fallible_quote! {
			impl ::shakespeare::ActorShell for #actor_name {
				type ExitType = #exit_return;
				type PanicType = #panic_return;
			}
		}?;

		Ok(ActorStruct {
			accesors: accessor_impl,
			strukt,
			meta_trait,
		})
	}
}

impl ToTokens for ActorStruct {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.strukt.to_tokens(tokens);
		self.meta_trait.to_tokens(tokens);
		self.accesors.to_tokens(tokens);
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

fn make_accessor_from_name(role_name: &RoleName, vis: &Visibility) -> Result<ImplItemFn> {
	let payload_path = role_name.payload_path();
	let field_name = role_name.queue_name();

	let error_path: Path = fallible_quote! { shakespeare::Role2SendError<dyn #role_name> }?;

	let acccessor_name = role_name.acccessor_name();

	fallible_quote! {
		#vis async fn #acccessor_name(&self, payload: #payload_path) -> Result<(), #error_path>
		{
			self.#field_name.send(payload)
		}
	}
}

fn make_sender_getter_from_name(role_name: &RoleName, vis: &Visibility) -> Result<ImplItemFn> {
	let field_name = role_name.queue_name();

	let getter_name = role_name.sender_getter_name();

	fallible_quote! {
		#vis fn #getter_name(&self) -> &::shakespeare::Role2Sender<dyn #role_name>
		{
			&self.#field_name
		}
	}
}
