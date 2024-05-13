use convert_case::{Case, Casing};
use itertools::Itertools;
use quote::{format_ident, ToTokens};
use syn::{
	parse_quote, FnArg, Ident, ItemEnum, ItemImpl, Path, Result, ReturnType, Signature, Type,
	Variant,
};

use crate::macros::{fallible_quote, filter_unwrap, map_or_bail};

#[derive(Debug)]
pub(crate) struct PayloadEnum {
	definition: ItemEnum,
	impls: Vec<ItemImpl>,
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

	fn create_from_impls(payload_type: &Path, sigs: &[Signature]) -> Result<Vec<ItemImpl>> {
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
			.map(|(types, (ident, _))| Self::type_vector_to_from(&types, &ident, payload_type))
			.try_collect()?;

		Ok(impls)
	}

	fn type_vector_to_from(types: &Vec<&Type>, name: &Ident, payload: &Path) -> Result<ItemImpl> {
		fallible_quote! {
			impl From<#(#types),*> for #payload {
				fn from(value: #(#types),*) -> Self {
					Self::#name ( value )
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
pub struct ReturnPayload {
	definition: ItemEnum,
	impls: Vec<ItemImpl>,
}

impl ReturnPayload {
	pub fn new(return_payload_type: &Path, methods: &[Signature]) -> Result<ReturnPayload> {
		let variants = map_or_bail!(methods, Self::create_variant);

		let impls = Self::create_output_from_impls(return_payload_type, methods)?;

		let definition = fallible_quote! {
			pub enum #return_payload_type { #(#variants),* }
		}?;

		Ok(ReturnPayload { definition, impls })
	}

	fn create_output_from_impls(payload_type: &Path, sigs: &[Signature]) -> Result<Vec<ItemImpl>> {
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
			.map(|(typ, idents)| Self::create_try_from(payload_type, &typ, &idents))
			.try_collect()
	}

	fn create_try_from(payload_type: &Path, typ: &Type, idents: &[Ident]) -> Result<ItemImpl> {
		fallible_quote! {
			impl TryFrom<#payload_type> for #typ {
				type Error = ();
				fn try_from(value: #payload_type) -> ::std::result::Result<Self, Self::Error> {
					match value {
						#(|#payload_type::#idents(val)),* => Ok(val),
						_ => Err(())
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
