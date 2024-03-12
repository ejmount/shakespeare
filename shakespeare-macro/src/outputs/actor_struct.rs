use quote::{quote, ToTokens};
use syn::parse::Parser;
use syn::{Expr, Field, ImplItemFn, ItemImpl, ItemStruct, Result, Visibility};

use crate::declarations::actor::ActorDecl;
use crate::declarations::performance::PerformanceDecl;
use crate::macros::{fallible_quote, map_or_bail};

#[derive(Debug)]
pub struct ActorStruct {
	strukt:   ItemStruct,
	accesors: ItemImpl,
}

impl ActorStruct {
	pub fn new(actor: &ActorDecl) -> Result<ActorStruct> {
		let ActorDecl {
			actor_name,
			performances,
			actor_vis,
			..
		} = actor;

		let fields = map_or_bail!(performances, make_field_from_name);

		let accessors = map_or_bail!(performances, |perf| make_accessor_from_name(
			perf, actor_vis
		));

		let strukt = fallible_quote! {
			#actor_vis struct #actor_name {
				#(#fields),*
			}
		}?;

		let accessor_impl = fallible_quote! {
			impl #actor_name {
				#(#accessors),*
			}
		}?;

		Ok(ActorStruct {
			accesors: accessor_impl,
			strukt,
		})
	}
}

impl ToTokens for ActorStruct {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.strukt.to_tokens(tokens);
		self.accesors.to_tokens(tokens);
	}
}

fn make_field_from_name(perf: &PerformanceDecl) -> Result<Field> {
	let role_name = &perf.role_name;
	let field_name = role_name.queue_name();

	Field::parse_named
		.parse2(
			quote! {#field_name : <<dyn #role_name as shakespeare::Role>::Channel as shakespeare::Channel>::Sender},
		)
		.map_err(|err| {
			syn::parse::Error::new(err.span(),
				format!("Parse failure trying to create actor field: {err} - this is a bug, please file an issue")
			)
		})
}

fn make_accessor_from_name(perf: &PerformanceDecl, vis: &Visibility) -> Result<ImplItemFn> {
	let role_name = &perf.role_name;
	let payload_path = role_name.payload_path();
	let field_name = role_name.queue_name();

	let error_path: Expr = fallible_quote! { <<<dyn #role_name as  ::shakespeare::Role>::Channel as ::shakespeare::Channel>::Sender as ::shakespeare::RoleSender<#payload_path>>::Error }?;

	let acccessor_name = role_name.acccessor_name();

	fallible_quote! {
		#vis async fn #acccessor_name(&self, payload: #payload_path) -> Result<(), #error_path>
		{
			self.#field_name.send(payload)
		}
	}
}
