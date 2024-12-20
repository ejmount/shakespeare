use quote::ToTokens;
use syn::{Item, Result};

mod actor_struct;
mod self_getter;
mod spawning_function;

use actor_struct::ActorStruct;
use self_getter::SelfGetter;
use spawning_function::SpawningFunction;

use super::performance::PerfDispatch;
use super::role::RoleOutput;
use crate::data::{DataItem, HandlerFunctions};
use crate::declarations::ActorDecl;
use crate::macros::map_or_bail;

#[derive(Debug)]
pub(crate) struct ActorOutput {
	data_item:         DataItem,
	actor_struct:      ActorStruct,
	getter:            SelfGetter,
	spawning_function: SpawningFunction,
	handlers:          HandlerFunctions,
	performances:      Vec<PerfDispatch>,
	roles:             Vec<RoleOutput>,
	misc:              Vec<Item>,
}

impl ActorOutput {
	pub(crate) fn new(actor_node: ActorDecl) -> Result<ActorOutput> {
		let actor_struct = ActorStruct::new(&actor_node)?;

		let ActorDecl {
			actor_name,
			data_item,
			performances,
			roles,
			handlers,
			misc,
			..
		} = actor_node;

		let data_name = data_item.name();
		let panic_name = handlers.panic_name();
		let exit_name = handlers.exit_name();

		let getter = SelfGetter::new(&actor_name)?;

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
			getter,
			spawning_function: sf,
			roles,
			actor_struct,
			handlers,
			misc,
		})
	}
}

impl ToTokens for ActorOutput {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		self.actor_struct.to_tokens(tokens);
		self.data_item.to_tokens(tokens);
		self.getter.to_tokens(tokens);
		self.spawning_function.to_tokens(tokens);
		self.handlers.to_tokens(tokens);
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
