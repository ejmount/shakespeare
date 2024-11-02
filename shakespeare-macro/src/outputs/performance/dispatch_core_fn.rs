use convert_case::Case::Snake;
use convert_case::Casing;
use itertools::{Either, Itertools};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::{Arm, Attribute, Expr, Ident, ItemImpl, Path, Result};

use crate::data::{DataName, FunctionItem, MethodName, PayloadPath, RoleName, SignatureExt};
use crate::declarations::make_variant_name;
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
				pub async fn #dispatch_method_name(&mut self, context: &mut ::shakespeare::Context<Self>, msg: ::shakespeare::ReturnEnvelope<dyn #role_name>)  {
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

fn make_method_name(role_name: &RoleName, old_ident: &Ident) -> Ident {
	let new = format!("{}_{}", role_name.path_leaf(), old_ident).to_case(Snake);
	format_ident!("{}", new)
}

fn dispatch_case(role_name: &RoleName, payload_type: &Path, fun: &FunctionItem) -> Result<Arm> {
	let mut num_parameters = fun.sig.inputs.len();
	if fun.sig.has_context_input() {
		num_parameters -= 1;
	}
	if num_parameters == 0 {
		return Err(syn::Error::new(
			fun.span(),
			"Performance method cannot have no receiver",
		));
	}
	let names = (0..num_parameters - 1).map(|n| format_ident!("_{n}"));

	let call_params = if fun.sig.has_context_input() {
		Either::Left(std::iter::once(format_ident!("context")).chain(names.clone()))
	} else {
		Either::Right(names.clone())
	};

	let variant_name = make_variant_name(fun);

	let fn_name = make_method_name(role_name, &fun.sig.ident);
	let asyncness: Option<TokenStream> = fun.sig.asyncness.is_some().then_some(quote!(.await));

	let into_call: Expr = fallible_quote! {
			<dyn #role_name as ::shakespeare::Role>::Return::#variant_name( self.#fn_name(#(#call_params),*)#asyncness )
	}?;

	fallible_quote! {
		#payload_type::#variant_name ((#(#names),*)) => { #into_call }
	}
}
