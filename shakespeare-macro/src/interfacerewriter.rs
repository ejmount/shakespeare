use syn::fold::Fold;
use syn::{parse_quote, Block, Receiver, ReturnType};

use crate::data::RoleName;

/// Rewrites the plain method from a role/performance block to have the correct signature for the actor wrapper.
/// Can be called on either bare signatures or methods, so we can't fill in the body at this point
pub struct InterfaceRewriter<'a> {
	role_name: &'a RoleName,
}
impl<'a> InterfaceRewriter<'a> {
	pub fn new(role_name: &RoleName) -> InterfaceRewriter {
		InterfaceRewriter { role_name }
	}
}

impl<'a> Fold for InterfaceRewriter<'a> {
	fn fold_receiver(&mut self, _: Receiver) -> Receiver {
		parse_quote! { &self }
	}

	fn fold_return_type(&mut self, _: ReturnType) -> ReturnType {
		let role_name = &self.role_name;
		parse_quote! {-> Result <(), ::shakespeare::Role2SendError<dyn #role_name>> }
	}

	fn fold_signature(&mut self, i: syn::Signature) -> syn::Signature {
		// Visit deeper
		let sig = syn::fold::fold_signature(self, i);

		if sig.asyncness.is_none() {
			parse_quote! { async #sig }
		} else {
			sig
		}
	}

	fn fold_block(&mut self, i: Block) -> Block {
		i // Don't recurse because we don't want to modify the contents
	}
}
