use convert_case::{Case, Casing};
use itertools::Itertools;
use quote::{format_ident, ToTokens};
use syn::{
	parse_quote, FnArg, Ident, ItemEnum, ItemImpl, Path, Result, ReturnType, Signature, Type,
	Variant,
};

use crate::data::RoleName;
use crate::macros::{fallible_quote, filter_unwrap, map_or_bail};

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
			pub enum #payload_type { #(#variants),* }
		}?;

		Ok(PayloadEnum { definition, impls })
	}

	fn create_from_impls(role_name: &RoleName, sigs: &[Signature]) -> Result<Vec<ItemImpl>> {
		let variant_names = sigs.iter().map(variant_name_from_sig);

		let group_map = sigs
			.iter()
			.map(Self::extract_input_type_vector)
			.zip(variant_names.map(|a| (a, 1)))
			.into_grouping_map();

		let type_vector_set = group_map.fold_first(|(ident, count), _, v| (ident, count + v.1));

		let impls: Vec<_> = type_vector_set
			.into_iter()
			.filter(|(types, (_, n))| !types.is_empty() && *n == 1)
			.map(|(types, (ident, _))| Self::type_vector_to_from(&types, &ident, role_name))
			.try_collect()?;

		Ok(impls)
	}

	fn type_vector_to_from(
		types: &Vec<&Type>,
		name: &Ident,
		role_name: &RoleName,
	) -> Result<ItemImpl> {
		fallible_quote! {
			impl ::shakespeare::Accepts<#(#types),*> for dyn #role_name {
				#[allow(unused_parens)]
				fn into_payload(value: #(#types),*) -> Self::Payload {
					Self::Payload::#name ( (value) )
				}
			}
		}
	}

	fn create_variant(sig: &Signature) -> Result<Variant> {
		let types = Self::extract_input_type_vector(sig);

		let variant_name = variant_name_from_sig(sig);

		fallible_quote! { #variant_name ((#(#types),*)) }
	}

	fn extract_input_type_vector(sig: &Signature) -> Vec<&Type> {
		filter_unwrap!(&sig.inputs, FnArg::Typed)
			.map(|p| &*p.ty)
			.collect_vec()
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
		let variants = map_or_bail!(methods, Self::create_variant);

		let impls = Self::create_output_from_impls(return_payload_type, methods, role_name)?;

		let definition = fallible_quote! {
			#[allow(unused_parens)]
			pub enum #return_payload_type { #(#variants),* }
		}?;

		Ok(ReturnPayload { definition, impls })
	}

	fn create_output_from_impls(
		payload_type: &Path,
		sigs: &[Signature],
		role_name: &RoleName,
	) -> Result<Vec<ItemImpl>> {
		let variant_names = sigs.iter().map(variant_name_from_sig);

		let group_map = sigs
			.iter()
			.map(Self::extract_return_type)
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
				fn from_return_payload(value: Self::Return) -> #typ {
					#[allow(irrefutable_let_patterns)]
					if let #(|Self::Return::#idents(val)),* = value {
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

	fn create_variant(sig: &Signature) -> Result<Variant> {
		let ret_type = Self::extract_return_type(sig);

		let variant_name = variant_name_from_sig(sig);

		Ok(fallible_quote! { #variant_name (#ret_type) }?)
	}

	fn extract_return_type(sig: &Signature) -> Type {
		if let ReturnType::Type(_, ret_type) = &sig.output {
			(**ret_type).clone()
		} else {
			parse_quote!(())
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

fn variant_name_from_sig(sig: &Signature) -> Ident {
	format_ident!("{}", sig.ident.to_string().to_case(Case::UpperCamel))
}
