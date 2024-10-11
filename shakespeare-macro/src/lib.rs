//! Hello
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unreachable_pub)]
#![warn(unused)]
#![warn(nonstandard_style)]
#![warn(clippy::pedantic)]
#![warn(clippy::missing_panics_doc)]
#![warn(clippy::dbg_macro)]
#![forbid(unsafe_code)]
#![warn(clippy::todo)]
#![warn(clippy::unimplemented)]
#![forbid(future_incompatible)]
#![warn(unused_crate_dependencies)]

mod data;
mod declarations;
mod interfacerewriter;
mod macros;
mod outputs;

use data::DataName;
use declarations::{ActorDecl, PerformanceDecl, RoleDecl};
use macros::filter_unwrap;
use outputs::{ActorOutput, PerfDispatch, RoleOutput};
use proc_macro::TokenStream;
use quote::ToTokens;
use syn::parse::Parse;
use syn::{parse_quote, ItemImpl, ItemMod, ItemTrait, Result, TraitItem, Type};

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

	let Type::Path(typath) = &*imp.self_ty else {
		return Err(syn::Error::new_spanned(
			&imp.self_ty,
			"Unsupported self type in performance",
		));
	};

	let data_name = DataName::new(typath.clone());
	let actor_path = data_name.get_shell_type_path();

	let decl = PerformanceDecl::new(role_name.clone(), imp)?;

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
fn parse_macro_input<T: Parse>(
	tokens: proc_macro2::TokenStream,
) -> ::std::result::Result<T, proc_macro2::TokenStream> {
	syn::parse2(tokens).map_err(|err| err.to_compile_error())
}

/// The starting point - defines a new actor type
///
/// This macro attaches to an inline `mod` block that must contain the following items:
/// 1. exactly one `struct`, `enum` *or* `union` definition representing the actor's state type. Call this `S`
/// 2. at least one [`performance`] block.
///
/// The `mod` can also optionally contain any of:
/// 1. a function called `stop` that consumes a single `S` value and may return a value of any type. This function will be called when the actor drops.
/// 2. a function called `catch` that consumes a `Box<dyn Any + Send>` and may return a value of any type. This function will be called if any of the actor's performances panic.
///
/// The macro then generates a new type with the same name as the module. This new type:
/// 1. has a constructor function `start(state: S) -> ActorSpawn<Self>`
/// 2. implements each role trait for which it has a performance.
///
/// The `ActorSpawn` contains an `Arc` that refers to the actor object. This value is the interface for sending the actor messages and controls its lifetime. When the last `Arc` goes out of scope, the actor will finish processing any messages it has already received, call its `stop` function if one exists, and then drop its state. If a method handler inside a performance panics, the `catch` function will be called *instead of* `stop`.
///
/// The `ActorSpawn` also contains a `Handle`, which is a future that will yield the value produced by the actor stopping, either successfully or by panic.
#[proc_macro_attribute]
pub fn actor(attr: TokenStream, item: TokenStream) -> TokenStream {
	actor_internal(attr.into(), item.into()).into()
}

/// This exists for testing purposes.
#[expect(clippy::needless_pass_by_value)]
fn actor_internal(
	_attr: proc_macro2::TokenStream,
	item: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	match parse_macro_input(item) {
		Ok(module) => match make_actor(module) {
			Ok(actor_ouput) => actor_ouput.to_token_stream(),
			Err(e) => e.into_compile_error().into_token_stream(),
		},
		Err(err) => err,
	}
}

/// Defines an actor's implementation of a Role.
///
/// This macro applies is applided to an `impl` block naming the role being implemented
#[proc_macro_attribute]
pub fn performance(_attr: TokenStream, item: TokenStream) -> TokenStream {
	match parse_macro_input(item.into()) {
		Ok(imp) => match make_performance(imp) {
			Ok(perf) => perf.to_token_stream().into(),
			Err(e) => e.into_compile_error().into_token_stream().into(),
		},
		Err(err) => err.into(),
	}
}

/// Defines an interface that an actor may implement.
///
/// This macro applies to a `trait` block, and works very similarly to conventional traits.
#[proc_macro_attribute]
pub fn role(_attr: TokenStream, item: TokenStream) -> TokenStream {
	match parse_macro_input(item.into()) {
		Ok(imp) => match make_role(imp) {
			Ok(role) => role.to_token_stream().into(),
			Err(e) => e.into_compile_error().into_token_stream().into(),
		},
		Err(err) => err.into(),
	}
}

#[cfg(test)]
mod tests {

	use std::path::PathBuf;
	use std::str::FromStr;
	use std::{env, fs};

	use runtime_macros::emulate_attributelike_macro_expansion;

	#[test] // EXPANDER EXCLUDE
	fn expand_actor() {
		// This code doesn't check much. Instead, it does macro expansion at run time to let
		// code coverage work for the macro.
		let path = env::var("CARGO_MANIFEST_DIR").unwrap();
		let mut path = PathBuf::from_str(&path).unwrap();
		path.push("tests");
		path.push("successes");
		path.push("basic.rs");
		let file = fs::File::open(path).unwrap();
		emulate_attributelike_macro_expansion(file, &[("actor", crate::actor_internal)]).unwrap();
	}
}
