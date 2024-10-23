use std::mem::replace;

use itertools::Itertools;
use syn::punctuated::Punctuated;
use syn::{FnArg, PatType, Signature, Type};

pub(crate) fn needs_context(sig: &Signature) -> bool {
	if let Some(FnArg::Typed(PatType { ty, .. })) = sig.inputs.iter().nth(1) {
		if let Type::Reference(r) = &**ty {
			r.lifetime.as_ref().map_or(false, |l| l.ident != "static")
		} else {
			false
		}
	} else {
		false
	}
}

pub(crate) fn remove_context_param(sig: &mut Signature) {
	if needs_context(sig) {
		let mut items = replace(&mut sig.inputs, Punctuated::new())
			.into_iter()
			.collect_vec();
		items.remove(1);
		sig.inputs = items.into_iter().collect();
	}
}
