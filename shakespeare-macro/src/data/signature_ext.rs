use convert_case::{Case, Casing};
use itertools::{Either, Itertools};
use quote::format_ident;
use syn::{FnArg, Ident, PatType, Signature, Type, Variant, parse_quote};

use crate::macros::{fallible_quote, filter_unwrap};

pub(crate) trait SignatureExt {
	fn has_context_input(&self) -> bool;
	fn remove_context_param(&mut self);
	fn extract_return_type(&self) -> Type;
	fn enum_variant_name(&self) -> Ident;
	fn extract_input_type_vector(&self) -> Vec<&Type>;
	fn payload_pattern(&self) -> impl Iterator<Item = Ident>;
	fn method_call_pattern(&self) -> impl Iterator<Item = Ident>;
	fn create_return_variant(&self) -> syn::Result<Variant>;
}

impl SignatureExt for Signature {
	fn has_context_input(&self) -> bool {
		if let Some(FnArg::Typed(PatType { ty, .. })) = self.inputs.iter().nth(1) {
			if let Type::Reference(r) = &**ty {
				r.lifetime.as_ref().is_some_and(|l| l.ident != "static")
			} else {
				false
			}
		} else {
			false
		}
	}

	fn remove_context_param(&mut self) {
		if self.has_context_input() {
			let mut items = std::mem::take(&mut self.inputs).into_iter().collect_vec();
			items.remove(1);
			self.inputs = items.into_iter().collect();
		}
	}

	fn extract_return_type(&self) -> Type {
		if let syn::ReturnType::Type(_, ret_type) = &self.output {
			(**ret_type).clone()
		} else {
			parse_quote!(())
		}
	}

	fn enum_variant_name(&self) -> Ident {
		format_ident!("{}", self.ident.to_string().to_case(Case::UpperCamel))
	}

	fn extract_input_type_vector(&self) -> Vec<&Type> {
		filter_unwrap!(&self.inputs, FnArg::Typed)
			.map(|p| &*p.ty)
			.collect_vec()
	}

	fn payload_pattern(&self) -> impl Iterator<Item = Ident> {
		let num_parameters = if self.has_context_input() {
			self.inputs.len() - 1
		} else {
			self.inputs.len()
		};
		(0..num_parameters - 1).map(|n| format_ident!("_{n}"))
	}

	fn method_call_pattern(&self) -> impl Iterator<Item = Ident> {
		let names = self.payload_pattern();
		if self.has_context_input() {
			Either::Left(std::iter::once(format_ident!("context")).chain(names))
		} else {
			Either::Right(names)
		}
	}

	fn create_return_variant(&self) -> syn::Result<Variant> {
		let ret_type = self.extract_return_type();

		let variant_name = self.enum_variant_name();

		fallible_quote! { #variant_name (#ret_type) }
	}
}
