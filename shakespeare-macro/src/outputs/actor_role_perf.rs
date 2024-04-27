use itertools::Itertools;
use proc_macro2::Ident;
use quote::ToTokens;
use syn::fold::Fold;
use syn::{FnArg, ImplItemFn, ItemImpl, Path, Result, Signature};

use crate::data::{ActorName, FunctionItem, RoleName};
use crate::declarations::performance::make_variant_name;
use crate::interfacerewriter::InterfaceRewriter;
use crate::macros::{fallible_quote, filter_unwrap, map_or_bail};

#[derive(Debug)]
pub struct ActorPerf {
	imp: ItemImpl,
}

impl ActorPerf {
	pub fn new(
		actor_path: &ActorName,
		payload_type: &Path,
		role_name: &RoleName,
		handlers: &[FunctionItem],
	) -> Result<ActorPerf> {
		let accessor = role_name.acccessor_name();
		let sending_methods = map_or_bail!(handlers, |fun| create_sending_method(
			payload_type,
			fun,
			&accessor
		));

		let mut rewriter = InterfaceRewriter::new(role_name);
		let sending_methods = sending_methods
			.into_iter()
			.map(|i| rewriter.fold_impl_item_fn(i))
			.collect_vec();

		let getter_name = role_name.sender_getter_name();

		let sender_get: ImplItemFn = fallible_quote! {
			fn get_sender(&self) -> &::shakespeare::Role2Sender<dyn #role_name> {
				self.#getter_name()
			}
		}?;

		let imp = fallible_quote! {
			#[::async_trait::async_trait] // Can't be removed because it makes the trait not obj-safe
			impl #role_name for #actor_path {
				#(#sending_methods)*
				#sender_get
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
	accessor: &Ident,
) -> Result<FunctionItem> {
	let params: Vec<_> = filter_unwrap!(&fun.sig.inputs, FnArg::Typed)
		.map(|t| &t.pat)
		.collect();
	let variant_name = make_variant_name(fun);
	let sig = Signature {
		asyncness: None,
		..fun.sig.clone()
	};

	let fn_block = fallible_quote! {
		async #sig {
			use shakespeare::{RoleReceiver, RoleSender};
			let msg = (#(#params),*);
			let payload = #payload_type::#variant_name(msg);
			let _ = self.#accessor(payload).await;
			Ok(())
		}
	}?;

	Ok(fn_block)
}
