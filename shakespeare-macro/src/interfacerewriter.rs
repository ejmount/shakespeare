use syn::fold::Fold;
use syn::{parse_quote, Block, Receiver, ReturnType};

use crate::data::RoleName;

/// Rewrites the plain method from a role/performance block to have the correct signature for the actor wrapper.
/// Can be called on either bare signatures or methods, so we can't fill in the body at this point
pub struct InterfaceRewriter {
	role_name: RoleName,
}
impl InterfaceRewriter {
	pub fn new(role_name: RoleName) -> InterfaceRewriter {
		InterfaceRewriter { role_name }
	}
}

impl Fold for InterfaceRewriter {
	fn fold_receiver(&mut self, _: Receiver) -> Receiver {
		parse_quote! { &self }
	}

	fn fold_return_type(&mut self, _: ReturnType) -> ReturnType {
		let role_name = &self.role_name;
		parse_quote! {-> Result <(), <<<dyn #role_name as ::shakespeare::Role>::Channel as ::shakespeare::Channel>::Sender as ::shakespeare::RoleSender<<dyn #role_name as ::shakespeare::Role>::Payload>>::Error >}
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
