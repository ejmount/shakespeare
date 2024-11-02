use syn::fold::Fold;
use syn::{parse_quote, Block, Receiver, ReturnType};

use crate::data::{RoleName, SignatureExt};

/// Rewrites the plain method from a role/performance block to have the correct signature for the actor wrapper.
/// Can be called on either bare signatures or methods, so we can't fill in the body at this point
pub(crate) struct InterfaceRewriter<'a> {
	role_name: &'a RoleName,
}
impl InterfaceRewriter<'_> {
	pub(crate) fn new(role_name: &RoleName) -> InterfaceRewriter {
		InterfaceRewriter { role_name }
	}
}

impl Fold for InterfaceRewriter<'_> {
	fn fold_receiver(&mut self, _: Receiver) -> Receiver {
		parse_quote! { &self }
	}

	fn fold_signature(&mut self, sig: syn::Signature) -> syn::Signature {
		let mut sig = syn::fold::fold_signature(self, sig);

		// Visit deeper
		let role_name = self.role_name;

		sig.asyncness = None;
		let old_return = if let ReturnType::Type(_, ret) = &sig.output {
			(*ret).clone()
		} else {
			parse_quote!(())
		};

		sig.remove_context_param();

		sig.output = parse_quote!(
			-> ::shakespeare::Envelope<dyn #role_name, #old_return>
		);

		sig
	}

	fn fold_block(&mut self, i: Block) -> Block {
		i // Don't recurse because we don't want to modify the contents
	}
}
