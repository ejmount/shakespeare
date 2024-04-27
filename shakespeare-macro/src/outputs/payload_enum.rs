use std::collections::HashSet;

use convert_case::{Case, Casing};
use itertools::Itertools;
use quote::{format_ident, ToTokens};
use syn::{Fields, FieldsUnnamed, FnArg, ItemEnum, ItemImpl, Path, Result, Signature, Variant};

use crate::macros::{fallible_quote, filter_unwrap, map_or_bail};

#[derive(Debug)]
pub(crate) struct PayloadEnum {
	definition: ItemEnum,
	impls:      Vec<ItemImpl>,
}

impl PayloadEnum {
	pub fn new(payload_type: &Path, methods: &[Signature]) -> Result<PayloadEnum> {
		let variants = map_or_bail!(methods, Self::make_variant);

		let payload_name = &payload_type.segments.last().unwrap().ident;

		let definition = fallible_quote! {
			pub enum #payload_name { #(#variants),* }
		}?;

		let impls = create_from_impls(&variants, payload_type)?;

		Ok(PayloadEnum { definition, impls })
	}

	fn make_variant(sig: &Signature) -> Result<Variant> {
		let variant_name = format_ident!("{}", sig.ident.to_string().to_case(Case::UpperCamel));

		let types = filter_unwrap!(&sig.inputs, FnArg::Typed).map(|p| &*p.ty);
		fallible_quote! { #variant_name ((#(#types),*)) }
	}
}

impl ToTokens for PayloadEnum {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.definition.to_tokens(tokens);
		for i in &self.impls {
			i.to_tokens(tokens);
		}
	}
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
