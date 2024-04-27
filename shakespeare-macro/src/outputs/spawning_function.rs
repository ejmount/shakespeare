use itertools::{izip, Itertools};
use proc_macro2::TokenStream;
use quote::{format_ident, ToTokens};
use syn::parse::Parser;
use syn::{Expr, Field, Ident, ItemImpl, Result, Stmt};

use crate::data::{ActorName, DataName, RoleName};
use crate::declarations::performance::PerformanceDecl;
use crate::macros::{fallible_quote, map_or_bail};

#[derive(Debug)]
pub struct SpawningFunction {
	fun: ItemImpl,
}

impl SpawningFunction {
	pub fn new(
		actor_name: &ActorName,
		data_name: &DataName,
		performances: &[PerformanceDecl],
		panic_name: Option<Ident>,
		exit_name: Option<Ident>,
	) -> Result<SpawningFunction> {
		let field_names = performances
			.iter()
			.map(PerformanceDecl::get_role_name)
			.map(RoleName::queue_name)
			.collect_vec();

		let input_field_names = field_names
			.iter()
			.map(|name| format_ident!("{}_input", name))
			.collect_vec();

		let output_field_names = field_names
			.iter()
			.map(|name| format_ident!("{}_output", name))
			.collect_vec();

		let queue_constructions = map_or_bail!(
			izip!(performances, &input_field_names, &output_field_names),
			|(role, inn, out)| -> Result<Stmt> {
				let role_name = &role.role_name;
				fallible_quote! { let (#inn, mut #out) = <dyn #role_name as ::shakespeare::Role>::Channel::new_default(); }
			}
		);

		let actor_fields =
			map_or_bail!(
				izip!(performances, &input_field_names),
				|(role, input)| -> Result<Field> {
					let field_name = role.role_name.queue_name();
					Field::parse_named.parse2(fallible_quote! {#field_name : #input}?)
				}
			);

		assert!(!performances.is_empty());
		assert!(!output_field_names.is_empty());

		let select_branches = map_or_bail!(
			izip!(performances, &output_field_names),
			|(role, output)| -> Result<TokenStream> {
				let fn_name = role.role_name.method_name();
				fallible_quote! { Some(msg) = #output.recv(), if !(#output.is_empty() && outstanding_clients == 1) => {
					timeout_sleep.as_mut().reset(Instant::now() + IDLE_TIMEOUT);
					state.#fn_name(msg).await
				} }
			}
		);
		assert!(!select_branches.is_empty());

		let constructor: Expr = fallible_quote! {
			#actor_name {
				#(#actor_fields),*
			}
		}?;

		let run_panic_handler: Option<syn::Stmt> = panic_name
			.map(|p| fallible_quote! { let result = result.map_err(#p); })
			.transpose()?;

		let run_exit_handler: Option<syn::Stmt> = exit_name
			.map(|p| fallible_quote! { let result = result.map(|_| #p(state)); })
			.transpose()?;

		let getter_name = actor_name.get_static_item_name();

		let fun: ItemImpl = fallible_quote! {
			impl #data_name {
				pub fn start(mut state: #data_name) -> shakespeare::ActorSpawn<#actor_name> {
					use ::shakespeare::{ActorSpawn, Channel, RoleReceiver, catch_future};
					use ::std::sync::Arc;
					use ::tokio::{select, pin};
					use ::tokio::time::{sleep, Duration, Instant};

					const IDLE_TIMEOUT: Duration = Duration::from_millis(50);

					#(#queue_constructions)*
					let actor = Arc::new(#constructor);
					let stored_actor = Arc::clone(&actor);

					let event_loop = async move {
						let loop_lambda = async {
							let timeout_sleep = sleep(IDLE_TIMEOUT);
							pin!(timeout_sleep);
							loop {
								let outstanding_clients = #getter_name.with(Arc::strong_count);
								select! {
									#(#select_branches),*
									_ = &mut timeout_sleep, if outstanding_clients > 1 => {
										if outstanding_clients == 1 {
											break;
										}
										else {
											timeout_sleep.as_mut().reset(Instant::now() + IDLE_TIMEOUT)
										}
									},
									else => { break; }
								};
							}
						};

						// SAFETY: The receive handles inside the branches are not safe to unwind
						// But they're consumed by the closure, so we can never see them
						// The senders might interact with a dead receiver though.
						// If we assume that a panic will not happen **during** an operation on the receiver,
						// then the control block will still be consistent at any point the sender looks at it
						// even if the receiver was destroyed
						let guarded_future = catch_future(#getter_name.scope(stored_actor, loop_lambda));

						let result = guarded_future.await;

						#run_panic_handler
						#run_exit_handler
						result
					};

					let join_handle = ::tokio::task::spawn(event_loop);
					ActorSpawn::new(actor, join_handle)
				}
			}
		}?;

		Ok(SpawningFunction { fun })
	}
}

impl ToTokens for SpawningFunction {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.fun.to_tokens(tokens);
	}
}
