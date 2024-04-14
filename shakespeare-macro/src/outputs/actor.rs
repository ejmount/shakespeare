use quote::ToTokens;
use syn::{Item, ItemFn, Result};

use crate::data::DataItem;
use crate::declarations::actor::ActorDecl;
use crate::macros::map_or_bail;
use crate::outputs::actor_struct::ActorStruct;
use crate::outputs::perfdispatch::PerfDispatch;
use crate::outputs::role::RoleOutput;
use crate::outputs::spawning_function::SpawningFunction;

#[derive(Debug)]
pub struct ActorOutput {
	data_item:         DataItem,
	actor_struct:      ActorStruct,
	spawning_function: SpawningFunction,
	panic_handler:     Option<ItemFn>,
	exit_handler:      Option<ItemFn>,
	performances:      Vec<PerfDispatch>,
	roles:             Vec<RoleOutput>,
	misc:              Vec<Item>,
}

impl ActorOutput {
	pub fn new(actor_node: ActorDecl) -> Result<ActorOutput> {
		let actor_struct = ActorStruct::new(&actor_node)?;

		let ActorDecl {
			actor_name,
			data_item,
			performances,
			roles,
			panic_handler,
			exit_handler,
			misc,
			..
		} = actor_node;

		let data_name = data_item.name();
		let panic_name = panic_handler.as_ref().map(|i| i.sig.ident.clone());
		let exit_name = exit_handler.as_ref().map(|i| i.sig.ident.clone());

		assert!(!performances.is_empty());
		let sf = SpawningFunction::new(
			&actor_name,
			&data_name,
			&performances,
			panic_name,
			exit_name,
		)?;

		let roles = map_or_bail!(roles, RoleOutput::new);

		let performances = map_or_bail!(&performances, |perf| PerfDispatch::new(
			perf,
			&actor_name,
			&data_name
		));

		let performances = performances.into_iter().flatten().collect();

		Ok(ActorOutput {
			data_item,
			performances,
			spawning_function: sf,
			roles,
			actor_struct,
			panic_handler,
			exit_handler,
			misc,
		})
	}
}

impl ToTokens for ActorOutput {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.actor_struct.to_tokens(tokens);
		self.data_item.to_tokens(tokens);
		self.spawning_function.to_tokens(tokens);
		self.panic_handler.to_tokens(tokens);
		self.exit_handler.to_tokens(tokens);
		for p in &self.performances {
			p.to_tokens(tokens);
		}
		for r in &self.roles {
			r.to_tokens(tokens);
		}
		for i in &self.misc {
			i.to_tokens(tokens);
		}
	}
}
