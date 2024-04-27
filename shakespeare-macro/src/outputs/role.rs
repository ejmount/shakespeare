use quote::ToTokens;
use syn::fold::Fold;
use syn::{ItemImpl, ItemTrait, Result, TraitItem};

use super::payload_enum::ReturnPayload;
use crate::data::RoleName;
use crate::declarations::role::RoleDecl;
use crate::interfacerewriter::InterfaceRewriter;
use crate::macros::fallible_quote;
use crate::outputs::payload_enum::PayloadEnum;

#[derive(Debug)]
pub struct RoleOutput {
	payload_enum:        PayloadEnum,
	return_payload_enum: Option<ReturnPayload>,
	trait_definition:    ItemTrait,
	role_impl:           ItemImpl,
}

impl RoleOutput {
	pub fn new(role: RoleDecl) -> Result<RoleOutput> {
		let RoleDecl {
			name: role_name,
			signatures,
			vis,
		} = role;
		let role_name = RoleName::new(role_name);
		let payload_type = role_name.payload_path();
		let return_payload_type = role_name.return_payload_path();

		let payload_enum = PayloadEnum::new(&payload_type, &signatures)?;
		let return_payload_enum = ReturnPayload::new(&return_payload_type, &signatures)?;

		let mut rewriter = InterfaceRewriter::new(&role_name);
		let signatures = signatures.into_iter().map(|s| rewriter.fold_signature(s));

		let get_sender: TraitItem = fallible_quote! {
			async fn send(&self, val: #payload_type);
		}?;

		let trait_definition = fallible_quote! {
			#[::async_trait::async_trait]
			#vis trait #role_name: 'static + Send + Sync  {
				#(#signatures;)*
				#get_sender
			}
		}?;

		let role_impl = fallible_quote! {
			impl ::shakespeare::Role for dyn #role_name {
				type Payload = #payload_type;
				type Channel = ::shakespeare::TokioUnbounded<Self::Payload>;
				async fn send(&self, val: #payload_type) {
					self.send(val).await;
				}
			}
		}?;

		Ok(RoleOutput {
			payload_enum,
			return_payload_enum,
			trait_definition,
			role_impl,
		})
	}
}

impl ToTokens for RoleOutput {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.payload_enum.to_tokens(tokens);
		self.return_payload_enum.to_tokens(tokens);
		self.trait_definition.to_tokens(tokens);
		self.role_impl.to_tokens(tokens);
	}
}
