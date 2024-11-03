use itertools::Itertools;
use quote::ToTokens;
use syn::{Ident, ItemEnum, ItemImpl, Path, Result, Signature, Type, Variant};

use crate::data::{RoleName, SignatureExt};
use crate::macros::{fallible_quote, map_or_bail};

#[derive(Debug)]
pub(crate) struct PayloadEnum {
	definition: ItemEnum,
	impls:      Vec<ItemImpl>,
}

impl PayloadEnum {
	pub(crate) fn new(
		payload_type: &Path,
		methods: &[Signature],
		role_name: &RoleName,
	) -> Result<PayloadEnum> {
		let variants = map_or_bail!(methods, Self::create_variant);

		let impls = Self::create_from_impls(role_name, methods)?;

		let definition = fallible_quote! {
			#[allow(unused_parens)]
			#[doc(hidden)]
			pub enum #payload_type { #(#variants),* }
		}?;

		Ok(PayloadEnum { definition, impls })
	}

	fn create_from_impls(role_name: &RoleName, sigs: &[Signature]) -> Result<Vec<ItemImpl>> {
		let variant_names = sigs.iter().map(SignatureExt::enum_variant_name);

		let group_map = sigs
			.iter()
			.map(SignatureExt::extract_input_type_vector)
			.zip(variant_names.map(|a| (a, 1)))
			.into_grouping_map();

		let type_vector_set = group_map.reduce(|(ident, count), _, v| (ident, count + v.1));

		let impls: Vec<_> = type_vector_set
			.into_iter()
			.filter(|(types, (_, n))| !types.is_empty() && *n == 1)
			.map(|(types, (ident, _))| Self::type_vector_to_accepts(&types, &ident, role_name))
			.try_collect()?;

		Ok(impls)
	}

	fn type_vector_to_accepts(
		types: &Vec<&Type>,
		name: &Ident,
		role_name: &RoleName,
	) -> Result<ItemImpl> {
		fallible_quote! {
			impl ::shakespeare::Accepts<(#(#types),*)> for dyn #role_name {
				#[allow(unused_parens)]
				#[doc(hidden)]
				fn into_payload(value: (#(#types),*)) -> Self::Payload {
					Self::Payload::#name ( (value) )
				}
			}
		}
	}

	fn create_variant(sig: &Signature) -> Result<Variant> {
		let types = sig.extract_input_type_vector();

		let variant_name = sig.enum_variant_name();

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

#[derive(Debug)]
pub(crate) struct ReturnPayload {
	definition: ItemEnum,
	impls:      Vec<ItemImpl>,
}

impl ReturnPayload {
	pub(crate) fn new(
		return_payload_type: &Path,
		methods: &[Signature],
		role_name: &RoleName,
	) -> Result<ReturnPayload> {
		let variants = map_or_bail!(methods, SignatureExt::create_return_variant);

		let impls = Self::create_output_from_impls(return_payload_type, methods, role_name)?;

		let definition = fallible_quote! {
			#[allow(unused_parens)]
			#[doc(hidden)]
			pub enum #return_payload_type { #(#variants),* }
		}?;

		Ok(ReturnPayload { definition, impls })
	}

	fn create_output_from_impls(
		payload_type: &Path,
		sigs: &[Signature],
		role_name: &RoleName,
	) -> Result<Vec<ItemImpl>> {
		let variant_names = sigs.iter().map(SignatureExt::enum_variant_name);

		let group_map = sigs
			.iter()
			.map(SignatureExt::extract_return_type)
			.zip(variant_names)
			.into_grouping_map();

		let groups = group_map.fold(vec![], |mut group, _, v| {
			group.push(v);
			group
		});

		groups
			.into_iter()
			.map(|(typ, idents)| Self::create_try_from(payload_type, &typ, &idents, role_name))
			.try_collect()
	}

	fn create_try_from(
		payload_type: &Path,
		typ: &Type,
		idents: &[Ident],
		role_name: &RoleName,
	) -> Result<ItemImpl> {
		fallible_quote! {
			impl ::shakespeare::Emits<#typ> for dyn #role_name {
				#[allow(unreachable_patterns)]
				#[doc(hidden)]
				fn from_return_payload(value: Self::Return) -> #typ {
					#[allow(irrefutable_let_patterns)]
					if let #(Self::Return::#idents(val))|* = value {
						val
					}
					else {
						unimplemented!("Failed to convert discriminant {:?} into type {} in role {}",
							core::mem::discriminant(&value),
							core::any::type_name::<#payload_type>(),
							core::any::type_name::<dyn #role_name>())
					}
				}
			}
		}
	}
}

impl ToTokens for ReturnPayload {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.definition.to_tokens(tokens);
		for i in &self.impls {
			i.to_tokens(tokens);
		}
	}
}
