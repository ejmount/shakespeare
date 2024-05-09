use std::collections::HashMap;

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
		let variants = map_or_bail!(methods, create_variant);

		let impls = create_from_impls(payload_type, methods)?;

		let definition = fallible_quote! {
			pub enum #payload_type { #(#variants),* }
		}?;

		Ok(PayloadEnum { definition, impls })
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
		let variants = map_or_bail!(methods, create_output_variant);

		let impls = create_from_impls_output(return_payload_type, methods)?;

		let definition = fallible_quote! {
			pub enum #return_payload_type { #(#variants),* }
		}?;

		Ok(ReturnPayload { definition, impls })
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
fn create_from_impls(payload_type: &Path, sigs: &[Signature]) -> Result<Vec<ItemImpl>> {
	let mut type_vector_set: HashMap<Vec<_>, (Ident, usize)> = HashMap::new();

	for sig in sigs {
		let types = filter_unwrap!(&sig.inputs, FnArg::Typed)
			.map(|p| &*p.ty)
			.collect();

		let ident = variant_name_from_sig(sig);

		let entry = type_vector_set.entry(types).or_insert((ident, 0));
		entry.1 += 1;
	}

	let impls: Vec<_> = type_vector_set
		.into_iter()
		.filter(|(_, (_, n))| *n == 1)
		.map(|(types, (name, _))| type_vector_to_from(&types, &name, payload_type))
		.try_collect()?;

	let impls = impls.into_iter().flatten().collect_vec();

	Ok(impls)
}

fn type_vector_to_from(
	types: &Vec<&Type>,
	name: &Ident,
	payload_type: &Path,
) -> Result<Option<ItemImpl>> {
	if types.is_empty() {
		return Ok(None);
	}

	let from_impl = fallible_quote! {
		impl From<#(#types),*> for #payload_type {
			fn from(value: #(#types),*) -> Self {
				Self::#name ( value )
			}
		}
	}?;
	Ok(Some(from_impl))
}

fn variant_name_from_sig(sig: &Signature) -> Ident {
	format_ident!("{}", sig.ident.to_string().to_case(Case::UpperCamel))
}

fn create_from_impls_output(
	payload_type: &Path,
	method_signatures: &[Signature],
) -> Result<Vec<ItemImpl>> {
	let variant_names = method_signatures.iter().map(variant_name_from_sig);

	let group_map = method_signatures
		.iter()
		.map(extract_return_type)
		.zip(variant_names)
		.into_grouping_map();

	let groups = group_map.fold(vec![], |mut group, _, v| {
		group.push(v);
		group
	});

	groups
		.into_iter()
		.map(|(typ, idents)| {
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
		})
		.try_collect()
}

fn extract_return_type(sig: &Signature) -> Type {
	if let ReturnType::Type(_, ret_type) = &sig.output {
		(**ret_type).clone()
	} else {
		parse_quote!(())
	}
}

fn create_output_variant(sig: &Signature) -> Result<Variant> {
	let ret_type = extract_return_type(sig);

	let variant_name = variant_name_from_sig(sig);

	Ok(fallible_quote! { #variant_name (#ret_type) }?)
}

fn create_variant(sig: &Signature) -> Result<Variant> {
	let types = filter_unwrap!(&sig.inputs, FnArg::Typed).map(|p| &*p.ty);

	let variant_name = variant_name_from_sig(sig);

	fallible_quote! { #variant_name ((#(#types),*)) }
}
