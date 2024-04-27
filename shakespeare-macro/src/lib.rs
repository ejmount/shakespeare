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
#![warn(clippy::todo)]
#![warn(clippy::unimplemented)]
#![forbid(future_incompatible)]

mod data;
mod declarations;
mod interfacerewriter;
mod macros;
mod outputs;

use proc_macro::TokenStream;
use quote::ToTokens;
use syn::parse::Parse;
use syn::{parse_quote, ItemImpl, ItemMod, ItemTrait, Result, TraitItem};

use crate::data::DataName;
use crate::declarations::actor::ActorDecl;
use crate::declarations::performance::PerformanceDecl;
use crate::declarations::role::RoleDecl;
use crate::macros::filter_unwrap;
use crate::outputs::actor::ActorOutput;
use crate::outputs::perfdispatch::PerfDispatch;
use crate::outputs::role::RoleOutput;

#[cfg_attr(not(proc_macro), visibility::make(pub))]
fn make_actor(module: ItemMod) -> Result<ActorOutput> {
	ActorOutput::new(ActorDecl::new(module)?)
}

#[cfg_attr(not(proc_macro), visibility::make(pub))]
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

#[cfg_attr(not(proc_macro), visibility::make(pub))]
fn make_role(imp: ItemTrait) -> Result<RoleOutput> {
	let name = imp.ident;
	let items = imp.items;
	let vis = imp.vis;
	let signatures = filter_unwrap!(items, TraitItem::Fn).map(|f| f.sig);

	let decl = RoleDecl::new(parse_quote! { #name }, vis, signatures);

	RoleOutput::new(decl)
}

/// The `syn::parse_macro_input` macro is unsuitable for how this code works outside of an actually proc-macro crate beacuse it hardcodes the error return type as `proc_macro::TokenStream`
/// This creates problems when the xtask module tries to import it into a non-macro context.
/// This code is functionally the same, except that, being an ordinary function, we can't return early.
fn parse_macro_input<T: Parse>(tokens: TokenStream) -> ::std::result::Result<T, TokenStream> {
	let tokens = proc_macro2::TokenStream::from(tokens); // Yes this looks redundant but it's so that TokenStream can be swapped out
	syn::parse2(tokens).map_err(|err| TokenStream::from(err.to_compile_error()))
}

#[proc_macro_attribute]
pub fn actor(_attr: TokenStream, item: TokenStream) -> TokenStream {
	match parse_macro_input(item) {
		Ok(module) => match make_actor(module) {
			Ok(actor_ouput) => actor_ouput.to_token_stream().into(),
			Err(e) => e.into_compile_error().into_token_stream().into(),
		},
		Err(err) => err,
	}
}

#[proc_macro_attribute]
pub fn performance(_attr: TokenStream, item: TokenStream) -> TokenStream {
	match parse_macro_input(item) {
		Ok(imp) => match make_performance(imp) {
			Ok(perf) => perf.to_token_stream().into(),
			Err(e) => e.into_compile_error().into_token_stream().into(),
		},
		Err(err) => err,
	}
}

#[proc_macro_attribute]
pub fn role(_attr: TokenStream, item: TokenStream) -> TokenStream {
	match parse_macro_input(item) {
		Ok(imp) => match make_role(imp) {
			Ok(role) => role.to_token_stream().into(),
			Err(e) => e.into_compile_error().into_token_stream().into(),
		},
		Err(err) => err,
	}
}
