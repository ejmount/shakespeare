use convert_case::Case::Snake;
use convert_case::Casing;
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::parse::Parser;
use syn::{Arm, Attribute, Expr, Ident, ItemImpl, Path, Result};

use crate::data::{DataName, FunctionItem, MethodName, PayloadPath, RoleName, SignatureExt};
use crate::macros::{fallible_quote, map_or_bail};

#[derive(Debug)]
pub(crate) struct DispatchFunction {
	fun: ItemImpl,
}

impl DispatchFunction {
	pub(crate) fn new(
		data_name: &DataName,
		role_name: &RoleName,
		payload_type: &PayloadPath,
		dispatch_method_name: &MethodName,
		handlers: &[FunctionItem],
	) -> Result<DispatchFunction> {
		let hide_doc: Attribute = Attribute::parse_outer
			.parse2(quote!(#[doc(hidden)]))?
			.pop()
			.unwrap();

		let dispatch_with_payload = |fun| dispatch_case(role_name, payload_type, fun);

		let arms: Vec<_> = map_or_bail!(&handlers, dispatch_with_payload);

		let renamed_handlers = handlers
			.iter()
			.map(|h| {
				let mut h = h.clone();
				h.sig.ident = make_method_name(role_name, &h.sig.ident);
				h.attrs.push(hide_doc.clone());
				h
			})
			.collect_vec();

		let fun = fallible_quote! {
			impl #data_name {
				#[doc(hidden)]
				pub async fn #dispatch_method_name(&mut self, #[allow(unused_variables)] context: &mut ::shakespeare::Context<Self>, msg: ::shakespeare::ReturnEnvelope<dyn #role_name>)  {
					#[allow(unused_variables)]
					let ::shakespeare::ReturnEnvelope { payload, return_path } = msg;

					#[allow(unused_variables)]
					#[allow(unused_parens)]
					#[allow(unreachable_code)]
					let return_val = match payload {
						#(#arms),*
					};
					return_path.send(return_val).await;
				}

				#(#renamed_handlers)*
			}
		}?;

		Ok(DispatchFunction { fun })
	}
}

impl ToTokens for DispatchFunction {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.fun.to_tokens(tokens);
	}
}

fn make_method_name(role_name: &RoleName, method_name: &Ident) -> Ident {
	let new = format!("{}_{}", role_name.path_leaf(), method_name).to_case(Snake);
	format_ident!("{}", new)
}

fn dispatch_case(role_name: &RoleName, payload_type: &Path, fun: &FunctionItem) -> Result<Arm> {
	let payload_pattern = fun.sig.payload_pattern();
	let method_call_pattern = fun.sig.method_call_pattern();

	let variant_name = fun.sig.enum_variant_name();

	let fn_name = make_method_name(role_name, &fun.sig.ident);
	let asyncness: Option<TokenStream> = fun.sig.asyncness.is_some().then_some(quote!(.await));

	let into_call: Expr = fallible_quote! {
			<dyn #role_name as ::shakespeare::Role>::Return::#variant_name( self.#fn_name(#(#method_call_pattern),*)#asyncness )
	}?;

	fallible_quote! {
		#payload_type::#variant_name ((#(#payload_pattern),*)) => { #into_call }
	}
}
