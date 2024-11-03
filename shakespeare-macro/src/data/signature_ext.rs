use std::mem::replace;

use itertools::Itertools;
use syn::punctuated::Punctuated;
use syn::{FnArg, PatType, Signature, Type};

pub(crate) trait SignatureExt {
	fn has_context_input(&self) -> bool;
	fn remove_context_param(&mut self);
}

impl SignatureExt for Signature {
	fn has_context_input(&self) -> bool {
		if let Some(FnArg::Typed(PatType { ty, .. })) = self.inputs.iter().nth(1) {
			if let Type::Reference(r) = &**ty {
				r.lifetime.as_ref().map_or(false, |l| l.ident != "static")
			} else {
				false
			}
		} else {
			false
		}
	}

	fn remove_context_param(&mut self) {
		if self.has_context_input() {
			let mut items = std::mem::replace(&mut self.inputs, Punctuated::new())
				.into_iter()
				.collect_vec();
			items.remove(1);
			self.inputs = items.into_iter().collect();
		}
	}
}
