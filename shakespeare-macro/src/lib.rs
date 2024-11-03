//! This crate contains the macros powering [`shakespeare`]. More information can be found in that crate.
#![forbid(unsafe_code)]
#![forbid(future_incompatible)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unreachable_pub)]
#![warn(unused)]
#![warn(nonstandard_style)]
#![warn(clippy::pedantic)]
#![warn(clippy::missing_panics_doc)]
#![warn(clippy::dbg_macro)]
#![warn(clippy::todo)]
#![warn(clippy::unimplemented)]
#![warn(unused_crate_dependencies)]
#![allow(clippy::tabs_in_doc_comments)]

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
use visibility as _;

#[cfg_attr(not(proc_macro), visibility::make(pub(crate)))]
fn make_actor(module: ItemMod) -> Result<ActorOutput> {
	ActorOutput::new(ActorDecl::new(module)?)
}

#[cfg_attr(not(proc_macro), visibility::make(pub(crate)))]
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

#[cfg_attr(not(proc_macro), visibility::make(pub(crate)))]
fn make_role(imp: ItemTrait) -> Result<RoleOutput> {
	let ItemTrait {
		ident: name,
		attrs,
		items,
		vis,
		..
	} = imp;

	let signatures = filter_unwrap!(items, TraitItem::Fn).map(|f| f.sig);

	let decl = RoleDecl::new(parse_quote! { #name }, attrs, vis, signatures);

	RoleOutput::new(decl)
}

/// The `syn::parse_macro_input` macro is unsuitable for how this code works outside of an actually proc-macro crate because it hardcodes the error return type as `proc_macro::TokenStream`
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
/// 2. at least one [`macro@performance`] block.
///
/// The `mod` can also optionally contain any of:
/// 1. a function called `stop` that consumes a single `S` value and may return a value of any type. This function will be called when the actor drops.
/// 2. a function called `catch` that consumes a `Box<dyn Any + Send>` and may return a value of any type. This function will be called if any of the actor's performances panic.
///
/// Inherent `impl S` blocks are allowed, but should be placed outside the actor `mod`. The state should not define a method called, `get_shell`, see [`macro@performance`].
///
/// The macro then generates a new type with the same name as the module. This new type:
/// 1. has a constructor function `start(state: S) -> ActorSpawn<Self>`
/// 2. implements each role trait for which it has a performance.
///
/// The `ActorSpawn` contains an `Arc` that refers to the actor object. This value is the interface for sending the actor messages and controls its lifetime. When the last `Arc` goes out of scope, the actor will finish processing any messages it has already received, call its `stop` function if one exists, and then drop its state. If a method handler inside a performance panics, the `catch` function will be called *instead of* `stop`.
///
/// The actor `Arc` can be upcast to a `Arc<dyn MyRole>` (for an actor with a performance of `MyRole`) to allow for code that works generically over a given role.
///
/// The `ActorSpawn` also contains a `Handle`, which is a future that will yield the value produced by the actor stopping, either successfully or by panic.
#[proc_macro_attribute]
pub fn actor(attr: TokenStream, item: TokenStream) -> TokenStream {
	actor_internal(attr.into(), item.into()).into()
}

/// This exists for test coverage purposes.
fn actor_internal(
	attr: proc_macro2::TokenStream,
	item: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	std::mem::drop(attr); // <-- Removes a clippy warning, because we need this exact signature for tests
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
/// This macro applies to an `impl...for` block that names the role being implemented and the *state* type, `impl Role for State`. The result will be that the trait's methods can be called on the *actor* type to pass messages into the actor and, if applicable, await any return value.
///
/// ```ignore
/// // A Role method
/// fn a_method(&mut self, value: T /* ... */) -> ReturnType;
/// // becomes an actor method:
/// async fn a_method(&self, value: T /* ... */) -> ReturnType;
/// ```
///
/// The trait's methods will be called with the actor's *state type* as the `self` type. The body of the methods are allowed to be arbitrary code like any other trait implementation, within the type signature restrictions required by [`macro@role`]. Additionally, the macro generates a new method for the state type, `Self::get_shell`, which will return an `Arc` of the running actor's message handle, which can then be, e.g. sent as a value to other actors. Be aware that an actor storing its own handle will effectively leak, as an actor normally exits only when all copies of its handle are dropped.
///
/// The implementation of a performance does not have to be contained within the module that defines the associated actor, but if it is *not*, the actor module must contain an empty impl block naming the appropriate role. That is, the following is allowed:
/// ```
/// # use shakespeare::{actor, performance, role};
///
/// #[actor]
/// mod MyActor {
/// 	struct S;
/// 	#[performance]
/// 	impl MyRole for S {}
/// }
///
/// #[performance]
/// impl MyRole for S {
/// 	fn a_method(&self) {
/// 		/* ... */
/// 	}
/// }
///
/// #[role]
/// trait MyRole {
/// 	fn a_method(&self) {}
/// }
/// ```
/// The names of the state and role types are resolved by normal language rules, so performance blocks do not need to be in the same module as the actor or role they name.
///
/// It is expected that many roles will have one "primary" implementation that defines the interface that, e.g. mock objects, are expected to follow. To reduce boilerplate, the macro takes a `canonical` flag, which will implicitly define a Role by the performance. (For now, a canonical performance *must* be inside the actor module - this restriction may be removed in the future) This means the previous example can be simplified to:
///
/// ```
/// # use shakespeare::{actor, performance};
///
/// #[actor]
/// mod MyActor {
/// 	struct S;
/// 	#[performance(canonical)]
/// 	impl MyRole for S {
/// 		fn foo(&self) {
/// 			/* ... */
/// 		}
/// 	}
/// }
/// ```
#[proc_macro_attribute]
pub fn performance(attr: TokenStream, item: TokenStream) -> TokenStream {
	performance_internal(attr.into(), item.into()).into()
}

fn performance_internal(
	attr: proc_macro2::TokenStream,
	item: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	std::mem::drop(attr); // <-- Removes a clippy warning, because we need this exact signature for tests
	match parse_macro_input(item) {
		Ok(imp) => match make_performance(imp) {
			Ok(perf) => perf.to_token_stream(),
			Err(e) => e.into_compile_error().into_token_stream(),
		},
		Err(err) => err,
	}
}

/// Defines an interface that an actor may implement.
///
/// This macro applies to a `trait` block, and for now takes no parameters.
///
/// The trait has the following restrictions:
/// 1. it cannot have any associated constants or types
/// 2. all functions must be methods and must take either `&self` or `&mut self` as receiver.
/// 3. all other parameters and all return types must have a lifetime of `'static`
/// 4. Neither methods nor parameters can have "free" generic parameters, nor `impl Trait` return values. (`Option<u32>` is allowed, `Option<T>` is not)
///
/// Role methods may be async, and if they are, may `await` other futures. However, be aware that the actor's message loop will be blocked while awaiting - this risks deadlocks if other actors have sent it messages and are waiting for the return values. [`send_return_to`][`send_return_to`] may be useful to avoid this situation.
///
/// Except for the above restrictions, a role is otherwise a normal trait and its methods can have any number of methods, input parameters, and return values of any type.
/// (Be aware that extremely large types being passed by value may cause performance impacts - these can be avoided by passing `Box` etc instead)
#[proc_macro_attribute]
pub fn role(attr: TokenStream, item: TokenStream) -> TokenStream {
	role_internal(attr.into(), item.into()).into()
}

fn role_internal(
	attr: proc_macro2::TokenStream,
	item: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	std::mem::drop(attr); // <-- Removes a clippy warning, because we need this exact signature for tests
	match parse_macro_input(item) {
		Ok(imp) => match make_role(imp) {
			Ok(role) => role.to_token_stream(),
			Err(e) => e.into_compile_error().into_token_stream(),
		},
		Err(err) => err,
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
		let dir = fs::read_dir(path).expect("Can't access {path}");

		expand_for_dir(dir);
	}

	fn expand_for_dir(dir: fs::ReadDir) {
		let macros: &[(
			&str,
			fn(proc_macro2::TokenStream, proc_macro2::TokenStream) -> proc_macro2::TokenStream,
		); 3] = &[
			("actor", crate::actor_internal),
			("performance", crate::performance_internal),
			("role", crate::role_internal),
		];

		for entry in dir {
			let entry = entry.expect("Can't access {entry}");
			let typ = entry.file_type().expect("Doesn't have a type??");
			if typ.is_file() {
				emulate_attributelike_macro_expansion(
					fs::File::open(entry.path()).unwrap(),
					macros,
				)
				.unwrap();
			} else if typ.is_dir() {
				expand_for_dir(fs::read_dir(entry.path()).unwrap());
			}
		}
	}
}
