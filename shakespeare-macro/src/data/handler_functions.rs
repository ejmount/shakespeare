use quote::{quote, ToTokens};
use syn::{Ident, ItemFn, ReturnType};

use super::DataName;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum HandlerFunction {
	Exit,
	Panic,
}

/// Functions that hook various aspects of the actor
#[derive(Debug)]
pub(crate) struct HandlerFunctions {
	state_name: DataName,
	panic:      Option<ItemFn>,
	exit:       Option<ItemFn>,
}

impl HandlerFunctions {
	pub(crate) fn new(state_name: DataName) -> HandlerFunctions {
		HandlerFunctions {
			state_name,
			exit: None,
			panic: None,
		}
	}

	pub(crate) fn add(&mut self, fun: ItemFn) -> bool {
		let storage = match &fun.sig.ident.to_string()[..] {
			"stop" => &mut self.exit,
			"catch" => &mut self.panic,
			_ => return false,
		};

		*storage = Some(fun);
		true
	}

	pub(crate) fn exit_name(&self) -> Option<&Ident> {
		self.exit.as_ref().map(|i| &i.sig.ident)
	}

	pub(crate) fn panic_name(&self) -> Option<&Ident> {
		self.panic.as_ref().map(|i| &i.sig.ident)
	}

	pub(crate) fn panic_return(&self) -> FuncReturnType {
		FuncReturnType(self.panic.as_ref(), HandlerFunction::Panic)
	}

	pub(crate) fn exit_return(&self) -> FuncReturnType {
		FuncReturnType(self.exit.as_ref(), HandlerFunction::Exit)
	}
}

impl ToTokens for HandlerFunctions {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		let HandlerFunctions {
			state_name,
			panic,
			exit,
		} = self;
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
pub(crate) struct FuncReturnType<'a>(Option<&'a ItemFn>, HandlerFunction);
impl ToTokens for FuncReturnType<'_> {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		let return_type = self.0.map(|fun| &fun.sig.output);

		match return_type {
			Some(ReturnType::Type(_, b)) => b.to_tokens(tokens),
			Some(ReturnType::Default) => quote! {()}.to_tokens(tokens),
			None => match self.1 {
				HandlerFunction::Exit => quote! {()}.to_tokens(tokens),
				HandlerFunction::Panic => {
					quote! {std::boxed::Box<dyn std::any::Any + std::marker::Send>}
						.to_tokens(tokens);
				}
			},
		};
	}
}
