use quote::ToTokens;
use syn::Result;

use super::actor_role_perf::ActorPerf;
use super::dispatch_core_fn::DispatchFunction;
use crate::data::{ActorName, DataName};
use crate::declarations::performance::PerformanceDecl;
#[derive(Debug)]
pub struct PerfDispatch {
	actor_impl:  ActorPerf,
	dispatch_fn: DispatchFunction,
}

impl PerfDispatch {
	pub fn new(
		perf: &PerformanceDecl,
		actor_path: &ActorName,
		data_name: &DataName,
	) -> Result<Option<PerfDispatch>> {
		let data_name = data_name.clone();
		let role_name = perf.get_role_name().clone();
		let handlers = &perf.handlers;
		let dispatch_method_name = role_name.method_name();
		let payload_type = role_name.payload_path();
		let return_payload_type = role_name.return_payload_path();

		if perf.handlers.is_empty() {
			Ok(None)
		} else {
			Ok(PerfDispatch {
				actor_impl:  ActorPerf::new(
					actor_path,
					&payload_type,
					&return_payload_type,
					&role_name,
					handlers,
				)?,
				dispatch_fn: DispatchFunction::new(
					&data_name,
					&role_name,
					&payload_type,
					&dispatch_method_name,
					handlers,
				)?,
			}
			.into())
		}
	}
}

impl ToTokens for PerfDispatch {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.actor_impl.to_tokens(tokens);
		self.dispatch_fn.to_tokens(tokens);
	}
}
