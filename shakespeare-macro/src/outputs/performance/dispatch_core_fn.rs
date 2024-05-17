use quote::ToTokens;
use syn::{Arm, Expr, FnArg, ItemImpl, Path, Result};

use crate::data::{DataName, FunctionItem, MethodName, PayloadPath, RoleName};
use crate::declarations::make_variant_name;
use crate::macros::{fallible_quote, filter_unwrap, map_or_bail};

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
		let dispatch_with_payload = |fun| dispatch_case(role_name, payload_type, fun);
		let arms: Vec<_> = map_or_bail!(handlers, dispatch_with_payload);

		let fun = fallible_quote! {
			impl #data_name {
				pub async fn #dispatch_method_name(&mut self, msg: ::shakespeare::ReturnEnvelope<dyn #role_name>)  {
					let ::shakespeare::ReturnEnvelope { payload: msg, return_path } = msg;

					let return_val = match msg {
						#(#arms),*
					};
					//todo!("Work out what this is supposed to do when the user is expecting a response but the return type is empty");
					//if let Some(return_val) = return_val {
						return_path.send(return_val).await;
					//}
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

fn dispatch_case(role_name: &RoleName, payload_type: &Path, fun: &FunctionItem) -> Result<Arm> {
	let patterns = filter_unwrap!(fun.sig.inputs.clone(), FnArg::Typed).map(|p| p.pat);

	let variant_name = make_variant_name(fun);

	let body = &fun.block;

	let into_call: Expr = fallible_quote! {
			<dyn #role_name as ::shakespeare::Role>::Return::#variant_name({ #body })
	}?;

	fallible_quote! {
		#payload_type::#variant_name ((#(#patterns),*)) => { #into_call }
	}
}
