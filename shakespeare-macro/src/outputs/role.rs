use std::collections::HashSet;

use convert_case::{Case, Casing};
use itertools::Itertools;
use quote::{format_ident, ToTokens};
use syn::fold::Fold;
use syn::{
	Fields, FieldsUnnamed, FnArg, ItemEnum, ItemImpl, ItemTrait, Path, Result, Signature,
	TraitItem, Variant,
};

use crate::data::RoleName;
use crate::declarations::role::RoleDecl;
use crate::interfacerewriter::InterfaceRewriter;
use crate::macros::{fallible_quote, filter_unwrap, map_or_bail};

#[derive(Debug)]
pub struct RoleOutput {
	payload_enum:     PayloadEnum,
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

		let mut rewriter = InterfaceRewriter::new(&role_name);
		let signatures = signatures.into_iter().map(|s| rewriter.fold_signature(s));

		let get_sender: TraitItem = fallible_quote! {
			fn get_sender<'a>(&'a self) -> &'a ::shakespeare::Role2Sender<dyn #role_name>;
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
				fn clone_sender(&self) -> ::shakespeare::Role2Sender<dyn #role_name> {
					self.get_sender().clone()
				}
			}
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

fn create_payload_from_impl(payload_type: &Path, methods: &[Signature]) -> Result<PayloadEnum> {
	fn make_variant(sig: &Signature) -> Result<Variant> {
		let variant_name = format_ident!("{}", sig.ident.to_string().to_case(Case::UpperCamel));

		let types = filter_unwrap!(&sig.inputs, FnArg::Typed).map(|p| &*p.ty);
		fallible_quote! { #variant_name ((#(#types),*)) }
	}
	let variants = map_or_bail!(methods, make_variant);

	let payload_name = &payload_type.segments.last().unwrap().ident;

	let definition = fallible_quote! {
		pub enum #payload_name { #(#variants),* }
	}?;

	let impls = create_from_impls(&variants, payload_type)?;

	Ok(PayloadEnum { definition, impls })
}

fn create_from_impls(variants: &Vec<Variant>, payload_type: &Path) -> Result<Vec<ItemImpl>> {
	let fields = variants.iter().map(|v| &v.fields);

	let type_set: HashSet<_> = filter_unwrap!(fields, Fields::Unnamed)
		.map(|f| f.unnamed.iter().map(|f| &f.ty).collect_vec())
		.collect();

	let mut impls = vec![];
	if type_set.len() == variants.len() {
		for var in variants {
			let Fields::Unnamed(FieldsUnnamed { unnamed, .. }) = &var.fields else {
				unreachable!()
			};

			let types = unnamed.iter().map(|p| &p.ty).collect_vec();
			let name = &var.ident;

			let from_impl = fallible_quote! {
				impl From<#(#types),*> for #payload_type {
					fn from(value: #(#types),*) -> Self {
						Self::#name ( value )
					}
				}
			}?;

			impls.push(from_impl);
		}
	}
	Ok(impls)
}

#[derive(Debug)]
struct PayloadEnum {
	definition: ItemEnum,
	impls:      Vec<ItemImpl>,
}

impl ToTokens for PayloadEnum {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.definition.to_tokens(tokens);
		for i in &self.impls {
			i.to_tokens(tokens);
		}
	}
}
