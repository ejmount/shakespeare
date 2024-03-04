use quote::ToTokens;
use syn::{Arm, FnArg, ItemImpl, Path, Result};

use crate::data::{DataName, FunctionItem, MethodName, PayloadPath};
use crate::declarations::performance::make_variant_name;
use crate::macros::{fallible_quote, filter_unwrap, map_or_bail};

#[derive(Debug)]
pub struct DispatchFunction {
	fun: ItemImpl,
}

impl DispatchFunction {
	pub fn new(
		data_name: &DataName,
		payload_type: &PayloadPath,
		dispatch_method_name: &MethodName,
		handlers: &[FunctionItem],
	) -> Result<DispatchFunction> {
		let dispatch_with_payload = |fun| dispatch_case(payload_type, fun);
		let arms: Vec<_> = map_or_bail!(handlers, dispatch_with_payload);

		let fun = fallible_quote! {
			impl #data_name {
				pub async fn #dispatch_method_name(&mut self, msg: #payload_type)  {
					match msg {
						#(#arms),*
					};
				}
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

fn dispatch_case(payload_type: &Path, fun: &FunctionItem) -> Result<Arm> {
	let patterns = filter_unwrap!(fun.sig.inputs.clone(), FnArg::Typed).map(|p| p.pat);

	let variant_name = make_variant_name(fun);

	let body = &fun.block;
	fallible_quote! {
		#payload_type::#variant_name ((#(#patterns),*)) => { #body },
	}
}
