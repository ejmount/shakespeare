use convert_case::{Case, Casing};
use quote::{format_ident, ToTokens};
use syn::fold::Fold;
use syn::{FnArg, ItemEnum, ItemImpl, ItemTrait, Path, Result, Signature, Variant};

use crate::data::RoleName;
use crate::declarations::role::RoleDecl;
use crate::interfacerewriter::InterfaceRewriter;
use crate::macros::{fallible_quote, filter_unwrap, map_or_bail};

#[derive(Debug)]
pub struct RoleOutput {
	payload_enum:     ItemEnum,
	trait_definition: ItemTrait,
	role_impl:        ItemImpl,
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
		let payload_enum = create_payload_from_impl(&payload_type, &signatures)?;

		let trait_ = fallible_quote! {
			#vis trait #role_name {
				#(#signatures;)*
			}
		}?;

		let role_impl = fallible_quote! {
			impl ::shakespeare::Role for dyn #role_name {
				type Payload = #payload_type;
				type Channel = ::shakespeare::TokioUnbounded<Self::Payload>;
			}
		}?;

		let trt: ItemTrait = InterfaceRewriter::new(role_name).fold_item_trait(trait_);

		let trait_definition = fallible_quote! {
			#[::async_trait::async_trait]
			#trt
		}?;

		Ok(RoleOutput {
			payload_enum,
			trait_definition,
			role_impl,
		})
	}
}

impl ToTokens for RoleOutput {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.payload_enum.to_tokens(tokens);
		self.trait_definition.to_tokens(tokens);
		self.role_impl.to_tokens(tokens);
	}
}

fn create_payload_from_impl(payload_type: &Path, methods: &[Signature]) -> Result<ItemEnum> {
	fn make_variant(sig: &Signature) -> Result<Variant> {
		let variant_name = format_ident!("{}", sig.ident.to_string().to_case(Case::UpperCamel));

		let types = filter_unwrap!(&sig.inputs, FnArg::Typed).map(|p| &*p.ty);
		fallible_quote! { #variant_name ((#(#types),*)) }
	}
	let variants = map_or_bail!(methods, make_variant);

	let payload_name = &payload_type.segments.last().unwrap().ident;

	fallible_quote! {
		pub enum #payload_name { #(#variants),* }
	}
}
