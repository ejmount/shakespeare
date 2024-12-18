use quote::{quote, ToTokens};
use syn::{Ident, ItemFn, ReturnType};

use super::DataName;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum HandlerFunctionType {
	Exit,
	Panic,
}

/// Functions that hook various aspects of the actor
#[derive(Debug)]
pub(crate) struct HandlerFunctions {
	state_name: Option<DataName>,
	panic:      Option<ItemFn>,
	exit:       Option<ItemFn>,
}

impl HandlerFunctions {
	pub(crate) fn new() -> HandlerFunctions {
		HandlerFunctions {
			state_name: None,
			exit:       None,
			panic:      None,
		}
	}

	pub(crate) fn set_data_name(&mut self, name: DataName) {
		self.state_name = Some(name);
	}

	pub(crate) fn add(&mut self, fun: &ItemFn) -> bool {
		let storage = match &fun.sig.ident.to_string()[..] {
			"stop" => &mut self.exit,
			"catch" => &mut self.panic,
			_ => return false,
		};

		*storage = Some(fun.clone());
		true
	}

	pub(crate) fn exit_name(&self) -> Option<&Ident> {
		self.exit.as_ref().map(|i| &i.sig.ident)
	}

	pub(crate) fn panic_name(&self) -> Option<&Ident> {
		self.panic.as_ref().map(|i| &i.sig.ident)
	}

	pub(crate) fn panic_return(&self) -> FuncReturnType<'_> {
		FuncReturnType(self.panic.as_ref(), HandlerFunctionType::Panic)
	}

	pub(crate) fn exit_return(&self) -> FuncReturnType<'_> {
		FuncReturnType(self.exit.as_ref(), HandlerFunctionType::Exit)
	}
}

impl ToTokens for HandlerFunctions {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		let HandlerFunctions {
			state_name: Some(state_name),
			panic,
			exit,
		} = self
		else {
			panic!("Actor is missing internal state type")
		};
		quote! {
			impl #state_name {
				#panic
				#exit
			}
		}
		.to_tokens(tokens);
	}
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FuncReturnType<'a>(Option<&'a ItemFn>, HandlerFunctionType);
impl ToTokens for FuncReturnType<'_> {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		let return_type = self.0.map(|fun| &fun.sig.output);

		match return_type {
			Some(ReturnType::Type(_, b)) => b.to_tokens(tokens),
			Some(ReturnType::Default) => quote! {()}.to_tokens(tokens),
			None => match self.1 {
				HandlerFunctionType::Exit => quote! {()}.to_tokens(tokens),
				HandlerFunctionType::Panic => {
					quote! {std::boxed::Box<dyn std::any::Any + std::marker::Send>}
						.to_tokens(tokens);
				}
			},
		}
	}
}
