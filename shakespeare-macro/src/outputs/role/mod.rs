mod payload_enum;

use payload_enum::{PayloadEnum, ReturnPayload};
use quote::ToTokens;
use syn::fold::Fold;
use syn::{ItemImpl, ItemTrait, Result};

use crate::data::RoleName;
use crate::declarations::RoleDecl;
use crate::interfacerewriter::InterfaceRewriter;
use crate::macros::fallible_quote;

#[derive(Debug)]
pub(crate) struct RoleOutput {
	payload_enum:        PayloadEnum,
	return_payload_enum: ReturnPayload,
	trait_definition:    ItemTrait,
	role_impl:           ItemImpl,
}

impl RoleOutput {
	pub(crate) fn new(role: RoleDecl) -> Result<RoleOutput> {
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

		let trait_definition = fallible_quote! {
			#[::shakespeare::async_trait_export::async_trait]
			#vis trait #role_name: 'static + Send + Sync {
				#(#signatures;)*
				fn send(&self, val: #payload_type) -> ::shakespeare::Envelope<dyn #role_name, #return_payload_type>;
				async fn enqueue(&self, val: ::shakespeare::ReturnEnvelope<dyn #role_name>) -> Result<(), ::shakespeare::Role2SendError<dyn #role_name>>;
				//fn listen_for(&self, msg: ::shakespeare::Envelope<dyn #role_name>);
			}
		}?;

		let role_impl = fallible_quote! {
			impl<'a> ::shakespeare::Role for dyn #role_name+'a {
				type Payload = #payload_type;
				type Return = #return_payload_type;
				type Channel = ::shakespeare::TokioUnbounded<::shakespeare::ReturnEnvelope<dyn #role_name>>;
				fn send(&self, val: #payload_type) {
					<Self as #role_name>::send(self, val);
				}
				async fn enqueue(&self, val: ::shakespeare::ReturnEnvelope<Self>) -> Result<(), ::shakespeare::Role2SendError<Self>> {
					self.enqueue(val).await
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
