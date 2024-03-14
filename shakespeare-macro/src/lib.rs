#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
//#![warn(missing_docs)]
//#![warn(unreachable_pub)]
#![warn(unused)]
#![warn(nonstandard_style)]
#![warn(clippy::pedantic)]
#![warn(clippy::missing_panics_doc)]
#![warn(clippy::dbg_macro)]
#![allow(clippy::module_name_repetitions)]
#![forbid(unsafe_code)]
#![forbid(clippy::todo)]
#![forbid(clippy::unimplemented)]
#![forbid(future_incompatible)]

mod data;
mod declarations;
mod interfacerewriter;
mod macros;
mod outputs;

use proc_macro::TokenStream as TokenStream1;
use quote::ToTokens;
use syn::{parse_quote, ItemImpl, ItemMod, ItemTrait, Result, TraitItem};

use crate::data::DataName;
use crate::declarations::actor::ActorDecl;
use crate::declarations::performance::PerformanceDecl;
use crate::declarations::role::RoleDecl;
use crate::macros::filter_unwrap;
use crate::outputs::actor::ActorOutput;
use crate::outputs::perfdispatch::PerfDispatch;
use crate::outputs::role::RoleOutput;

fn make_actor(module: ItemMod) -> Result<ActorOutput> {
	ActorOutput::new(ActorDecl::new(module)?)
}

fn make_performance(imp: ItemImpl) -> Result<PerfDispatch> {
	let empty_perf_error = syn::Error::new_spanned(&imp, "Standalone performance needs methods");

	let Some((_, role_name, _)) = &imp.trait_ else {
		return Err(syn::Error::new_spanned(
			&imp,
			"Cannot define standalone performance with no role name",
		));
	};

	let syn::Type::Path(typath) = &*imp.self_ty else {
		return Err(syn::Error::new_spanned(
			&imp.self_ty,
			"Unsupported self type in performance",
		));
	};
	let data_name = DataName::new(typath.path.clone());

	let decl = PerformanceDecl::new(role_name.clone(), imp)?;

	let actor_path = data_name.actor_path();

	match PerfDispatch::new(&decl, &actor_path, &data_name)? {
		Some(pd) => Ok(pd),
		None => Err(empty_perf_error),
	}
}

fn make_role(imp: ItemTrait) -> Result<RoleOutput> {
	let name = imp.ident;
	let items = imp.items;
	let vis = imp.vis;
	let signatures = filter_unwrap!(items, TraitItem::Fn).map(|f| f.sig);

	let decl = RoleDecl::new(parse_quote! { #name }, vis, signatures);

	RoleOutput::new(decl)
}

#[proc_macro_attribute]
pub fn actor(_attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
	let module = syn::parse_macro_input!(item as ItemMod);

	match make_actor(module) {
		Ok(actor_ouput) => actor_ouput.to_token_stream().into(),
		Err(e) => e.into_compile_error().into_token_stream().into(),
	}
}

#[proc_macro_attribute]
pub fn performance(_attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
	let imp = syn::parse_macro_input!(item as ItemImpl);
	match make_performance(imp) {
		Ok(perf) => perf.to_token_stream().into(),
		Err(e) => e.into_compile_error().into_token_stream().into(),
	}
}

#[proc_macro_attribute]
pub fn role(_attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
	let imp = syn::parse_macro_input!(item as ItemTrait);
	match make_role(imp) {
		Ok(role) => role.to_token_stream().into(),
		Err(e) => e.into_compile_error().into_token_stream().into(),
	}
}
