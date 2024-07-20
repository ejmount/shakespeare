use proc_macro2::Ident;
use syn::{Path, PathSegment};

mod actor_name;
mod data_item;
mod data_name;
mod role_name;

pub(crate) use actor_name::ActorName;
pub(crate) use data_item::DataItem;
pub(crate) use data_name::DataName;
pub(crate) use role_name::RoleName;

pub(crate) type FunctionItem = syn::ImplItemFn;

pub(crate) type MethodName = Ident;
pub(crate) type PayloadPath = Path;

pub(crate) fn map_path_leaf<F>(mut p: Path, f: F) -> Path
where
	F: Fn(Ident) -> Ident,
{
	debug_assert!(!p.segments.is_empty());
	let leaf = p.segments.pop().unwrap().into_value();
	let new_leaf = PathSegment {
		ident: f(leaf.ident),
		..leaf
	};
	p.segments.push(new_leaf);
	p
}
