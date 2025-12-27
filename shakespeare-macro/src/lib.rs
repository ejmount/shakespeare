//! This crate contains the macros powering [`shakespeare`](https://docs.rs/shakespeare/latest/shakespeare/). More information can be found in that crate.
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
use syn::{ItemImpl, ItemMod, ItemTrait, Result, TraitItem, Type, parse_quote};
use visibility as _;

// The following three functions exist as entry points to the macros that can be called outside of a proc-macro context.
// This is so that the xtask expand script can call them to manually do code expansion.
// They must be public so that the other module can see them, but cannot be public if this being built as a proc-macro crate because they have the wrong signatures.

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
/// 1. a function called `stop` that consumes `self` and has any return type, so long as that type is concrete (i.e. not `impl Trait` or with unbound generic types) and `Sized + 'static`. This function will be called with the actor's state value (of type `S`) when the actor drops or when the `Context` is explicitly called to do so.
/// 2. a function called `catch` that consumes `self` and also consumes a `Box<dyn Any + Send>`, with a return type with the same conditions as `stop`. This function will be called with the state value and any value provided to the `panic!` call if any of the actor's performance methods panic.
///
/// Other items, including inherent `impl S` blocks, will be passed through unmodified into the surrounding module.
///
/// The macro then generates a new type with the same name as the module. This new type:
/// 1. has a constructor function `start(state: S) -> ActorHandles<Self>`. (This function is currently *always* private to the parent module containg the `#[actor]` block - for now, you will need to write a wrapper to access it from a wider scope)
/// 2. implements each role trait for which it has a performance.
///
/// The `ActorHandles` contains an `Arc` that refers to the actor object. This value is the interface for sending the actor messages and controls its lifetime. When the last `Arc` goes out of scope, the actor will finish processing any messages it has already received, call its `stop` function if one exists, and then drop its state. If a method handler inside a performance panics, the `catch` function will be called *instead of* `stop`.
///
/// The actor `Arc` can be upcast to a `Arc<dyn MyRole>` (for an actor with a performance of `MyRole`) to allow for code that works generically over a given role.
///
/// The `ActorHandles` also contains a `ExitHandle`, which is a future that will yield the value produced by the actor stopping, either successfully or by panic. It  is not necessary to implement `stop` or `catch` as above to use the `ExitHandle`.
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
/// This macro applies to an `impl...for` block that names the actor **state type** and implements the Role methods. The body of the block is written as per a normal trait implementation:
/// ```
/// # use shakespeare::{actor, performance, role};
/// #[role]
/// trait MyRole { // <-- Roles can be named anything that's a valid trait name
///     // methods can have any number of parameters and any return type;
///     // see the documentation for the #[role] macro for limitations
/// 	fn a_method(&self, a_param: usize, /* ... */);
/// }
///
/// #[actor]
/// mod MyActor {
/// 	struct State(u32);
///     // /\ Can have any unique name and any number of fields; see the documentation for the #[actor] macro
/// 	#[performance]
/// 	impl MyRole for State {
/// 		async fn a_method(&mut self, a_param: usize) { // <-- Parameters and return type must match trait above, except as noted below
/// 			/* todo */
/// 		}
/// 	}
/// }
/// ```
/// As per the normal rules for trait impls, `self` within the method body refers to the state type - `State` in the example above. However, there are a number of caveats:
/// 1) the Role trait always defines the method to take `&self` and to be synchronous.
/// 2) the method as written in the `impl` block may be `async` (and so may `await` other futures) and it may take `&mut self` instead of `&self` as its own logic requires. If it does not need either of these capabilities, it can instead be written without the keywords, i.e. the `impl` method can be written to take `&self` instead of `&mut self` if the body does not mutate the actor state, and can be written without the `async` keyword if it does not `await` anything.
/// 3) while role methods can have any number of parameters and any type of return value, they must conform to the requirements for a valid Role, see the [`macro@role`] documentation.
///
/// Also as per normal language rules, the names of the trait and implementing type are allowed to be extended path names as well as single identifiers. Additionally, the `#[performance]` block is allowed to be outside the `#[actor]` module, but used this way, there must be an empty `#[performance]` block naming the  trait inside the module. As per the language allowing `impl` blocks outside the module defining the type, the `#[performance]` is allowed to be in a different path than the `#[actor]`. Doc-tests do not interact with modules in the usual manner, but see [this unit test][1] for an example.
///
/// Given a trait implementation, this macro generates corresponding methods for the actor shell type (`MyActor`) that pass the method's parameters as a message to the actor's mailbox and return an [`Envelope`][2], a Future-like value representing the return value of the method call. The actor continously services messages from the mailbox by calling the corresponding method from the `performance` for each message in turn, and the return value of each call is provided back to the caller via the [`Envelope`][2] value. **Note**: that the actor cannot service later messages until the method returns. This has potential to create a deadlock if an actor has a method that awaits a `Future`, and the `Future` needs a pending message from the same actor to be serviced. (e.g. if actor A sends a message to actor B and awaits its return value and actor B's response to the message is to send a message to A and await the answer before returning, the two deadlock as they both wait for the other to return.)
///
/// [1]: https://github.com/ejmount/shakespeare/blob/main/shakespeare-macro/tests/successes/modules.rs
/// [2]: https://docs.rs/shakespeare/latest/shakespeare/struct.Envelope.html
///
/// As described in the [`macro@role`] documentation, the generated methods on the shell type have a different signature than the one written in the trait. The shell method differs in that:
/// * it is always synchronous, even if the implementation is `async`
/// * it always takes `&self`, not `&mut self`
/// * it does not include a `Context` parameter even if the implementation does (see below)
/// * instead of returning a type `T`, it returns `Envelope<R, T>` where `R` is the role being implemented
///
/// ## Context
///
/// A [`Context`](https://docs.rs/shakespeare/latest/shakespeare/struct.Context.html) is how an implementation can manipulate the actor machinery itself, to e.g. explicitly stop the actor's message loop, or get a copy of the actor's message handle. To get a `Context`, the method inside the trait *implementation* should be written with `&'_ Context<Self>` (or, as needed, `&'_ mut Context<Self>`) as its **second** parameter, directly after the receiver and before any parameters written in the `trait` definition. (While it is allowed to name the lifetime, there is no use for doing so as no other value coming in or out of the method is allowed to be non-`'static`.) `Context` parameters are *not* needed in the trait definition and should not be written there under any circumstances. Multiple actors implementing a Role may use (or not use) a `Context` inside a given method independently, regardless of the choices made by the other actors.  If the previous example needed access to the `Context` inside `a_method`, that would be written:
/// ```
/// use shakespeare::{Context, actor, performance, role};
/// #[role]
/// trait MyRole {
/// 	// the trait remains the same as before
/// 	fn a_method(&self, a_param: usize /* ... */) {
/// 		/* */
/// 	}
/// }
/// #[actor]
/// mod MyActor {
/// 	struct State(u32);
/// 	#[performance]
/// 	impl MyRole for State {
/// 		async fn a_method(&mut self, ctx: &'_ mut Context<Self>, _param: usize) {
/// 			// ..
/// 		}
/// 	}
/// }
/// ```
///
/// ## Canonical performances
///
/// Currently, all external method calls into an actor must be defined by some Role that the actor performs. However, it is expected that some Roles will have a single "primary" implementation, with other implementations (if any exist) being conceptually subsidary to that one, e.g. an actor would have some interface as required by the domain logic, but a mock implementation of that same interface (for testing outside interactions with the actor) would be subsidary, because the mock interface's only responsibility is to match the domain-logic original, and the mock will never drive changes in the original's interfaces. Conversely, some cases will involve multiple "equal" implementations, such as differing implementations for a database connection - in these cases, it's advisable to define the Role using the [macro](`macro@role`), and then define separate `#[performance]` blocks.
///
/// For cases that do have a single primary implementation, the Role can be defined *implicitly* by the performance, by passing the `canonical` flag to the `#[performance]` attribute. The previous example can be equivalently written:
/// ```
/// use shakespeare::{Context, actor, performance};
/// #[actor]
/// mod MyActor {
/// 	struct State(u32);
/// 	#[performance(canonical)]
/// 	impl MyRole for State {
/// 		async fn a_method(&mut self, ctx: &'_ mut Context<Self>, _param: usize) {
/// 			// ..
/// 		}
/// 	}
/// }
/// ```
/// In addition to defining the implementation for how `MyActor` implements `MyRole` as with the `#[performance]` examples seen so far, the above *also* defines the overall Role called `MyRole`. It is defined to match the signatures that `MyActor` implements - it contains a single method, `a_method`, which in turn takes a single `usize` as its parameter. Methods inside a canonical performance *are* allowed to use `Context` parameters as described previously, and the generated Role will remove the `Context` parameters automatically. As a result, if a second actor implements a Role defined by a canonical performance, then that actor's performances of the methods may use (or not use) a `Context` independently of the canonical one.
///
/// Currently, a performance must be included inside the `#[actor]` module in order to be `canonical`.
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
/// This macro applies to a `trait` definition, and for now has no attributes.
///
/// The trait has the following restrictions:
/// 1. it cannot have any associated constants or types
/// 2. all functions must be methods and should take `&self` as receiver. (But see the documentation for [macro@performance]) Currently, `&mut self` is allowed but redundant - other receiver types are not allowed at all.
/// 3. all other parameters and all return types must be `Send`, `Sized` and have a lifetime of `'static`
/// 4. Neither methods nor parameters can have "unbound" generic parameters, nor use `impl Trait` in either parameter or return position. (`Option<u32>` is allowed, `Option<T>` is not) Role methods *are* allowed to be `async`, but it is not allowed to have the function return `impl Future`.
///
/// Role methods may be async, and if they are, may `await` other futures. However, be aware that the actor's message loop will be blocked while awaiting - this risks deadlocks if other actors have sent it messages and are waiting for the return values. [`Envelope::forward_to`](https://docs.rs/shakespeare/latest/shakespeare/struct.Envelope.html#method.forward_to) may be useful to avoid this situation.
///
/// Except for the above restrictions, a role is otherwise a normal trait and its methods can have any number of methods, input parameters, and return values of any type.
/// (Be aware that, as with any other trait, extremely large inline types may cause performance impacts - these can be avoided by passing `Box`, etc, instead. Performance may also degrade faster than would be the case with synchronous function calls because large parameters will imply large message queue slots and the like.)
///
/// Note that it is always a mistake to include a [`Context`][1] parameter in a signature inside a standalone `#[role]` definition. Instead, it should be written only in the corresponding `#[performance]` method as the second parameter, directly after the `&self`/`&mut self`. This is currently not detected as an error, but calling such a method would require owning a [`Context`][1] value and it should not be possible to do that from external code.
///
/// [1]: https://docs.rs/shakespeare/latest/shakespeare/struct.Context.html
///
/// Note the generated trait's methods will *not* have exactly the signatures written in the trait block. Instead, given a Role defined by:
/// ```
/// # use shakespeare::role;
/// # struct ReturnType;
/// #[role]
/// trait MyRole {
/// 	fn a_method(&mut self, input: u32) -> ReturnType;
/// }
/// ```
/// An actor implementing this role via a [`macro@performance`] block will have a method with the following signature on the shell type:
/// ```ignore
/// fn a_method(&self, input: u32) -> Envelope<MyRole, ReturnType>;
/// ```
///
/// In addition, calling `a_method` won't immediately do any work - see the documentation for [`Envelope`](https://docs.rs/shakespeare/latest/shakespeare/struct.Envelope.html)
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
