use itertools::Itertools;
use quote::ToTokens;
use syn::{FnArg, ItemImpl, Path, Result, ReturnType, Signature, parse_quote};

use crate::data::{ActorName, FunctionItem, RoleName, SignatureExt};
use crate::macros::{fallible_quote, filter_unwrap, map_or_bail};

#[derive(Debug)]
pub(crate) struct ActorPerf {
	imp: ItemImpl,
}

impl ActorPerf {
	pub(crate) fn new(
		actor_path: &ActorName,
		payload_type: &Path,
		role_name: &RoleName,
		handlers: &[FunctionItem],
	) -> Result<ActorPerf> {
		let sending_methods = map_or_bail!(handlers, |fun| create_sending_method(
			payload_type,
			fun,
			role_name
		));

		let sender_name = role_name.sender_method_name();

		let imp = fallible_quote! {
			#[::shakespeare::async_trait_export::async_trait] // Can't be removed because it makes the trait not obj-safe
			impl #role_name for #actor_path {
				#(#sending_methods)*
				#[doc(hidden)]
				async fn enqueue(&self, val: ::shakespeare::ReturnEnvelope<dyn #role_name>) -> Result<(), ::shakespeare::Role2SendError<dyn #role_name>>{
					self.#sender_name(val).await
				}
			}
		}?;

		Ok(ActorPerf { imp })
	}
}

impl ToTokens for ActorPerf {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.imp.to_tokens(tokens);
	}
}

fn create_sending_method(
	payload_type: &Path,
	fun: &FunctionItem,
	role_name: &RoleName,
) -> Result<FunctionItem> {
	let attributes = fun.attrs.iter();

	let mut sig = fun.sig.clone();

	sig.remove_context_param();
	let Signature {
		ident,
		inputs,
		output,
		..
	} = &sig;
	let params = filter_unwrap!(inputs, FnArg::Typed).collect_vec();
	let patterns = params.iter().map(|t| &(*t.pat)).collect_vec();
	let variant_name = fun.sig.enum_variant_name();

	let return_type = if let ReturnType::Type(_, ret) = output {
		*ret.clone()
	} else {
		parse_quote!(())
	};

	let fn_block = fallible_quote! {
		#[allow(unused_parens)]
		#[allow(dead_code)]
		#(#attributes)*
		fn #ident(&self, #(#params),*) -> ::shakespeare::Envelope<dyn #role_name, #return_type> {
			let msg = (#(#patterns),*);
			let payload = #payload_type::#variant_name(msg);
			::shakespeare::Envelope::new(payload, self.get_shell())
		}
	}?;

	Ok(fn_block)
}
