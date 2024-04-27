use std::collections::HashSet;

use convert_case::{Case, Casing};
use itertools::Itertools;
use quote::{format_ident, ToTokens};
use syn::{FnArg, ItemEnum, ItemImpl, Path, Result, Signature, Variant};

use crate::macros::{fallible_quote, filter_unwrap, map_or_bail};

#[derive(Debug)]
pub(crate) struct PayloadEnum {
	definition: ItemEnum,
	impls:      Vec<ItemImpl>,
}

impl PayloadEnum {
	pub fn new(payload_type: &Path, methods: &[Signature]) -> Result<PayloadEnum> {
		let variants = map_or_bail!(methods, Self::create_variant);

		let impls = Self::create_from_impls(payload_type, methods)?;

		let definition = fallible_quote! {
			pub enum #payload_type { #(#variants),* }
		}?;

		Ok(PayloadEnum { definition, impls })
	}

	fn create_variant(sig: &Signature) -> Result<Variant> {
		let types = filter_unwrap!(&sig.inputs, FnArg::Typed).map(|p| &*p.ty);

		let variant_name = format_ident!("{}", sig.ident.to_string().to_case(Case::UpperCamel));

		fallible_quote! { #variant_name ((#(#types),*)) }
	}

	fn create_from_impls(payload_type: &Path, sigs: &[Signature]) -> Result<Vec<ItemImpl>> {
		let type_vector_set: HashSet<Vec<_>> = sigs
			.iter()
			.map(|s| {
				filter_unwrap!(&s.inputs, FnArg::Typed)
					.map(|p| &*p.ty)
					.collect()
			})
			.collect();

		if type_vector_set.len() == sigs.len() {
			let from_impls = map_or_bail!(&sigs, |s| Self::signature_to_from(s, payload_type));
			Ok(from_impls)
		} else {
			Ok(vec![])
		}
	}

	fn signature_to_from(sig: &Signature, payload_type: &Path) -> Result<ItemImpl> {
		let types = filter_unwrap!(&sig.inputs, FnArg::Typed)
			.map(|p| &*p.ty)
			.collect_vec();
		let name = &sig.ident;
		let from_impl = fallible_quote! {
			impl From<#(#types),*> for #payload_type {
				fn from(value: #(#types),*) -> Self {
					Self::#name ( value )
				}
			}
		}?;
		Ok(from_impl)
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
