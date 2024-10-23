mod actor_name;
mod data_item;
mod data_name;
mod role_name;
mod signature;

pub(crate) use actor_name::ActorName;
pub(crate) use data_item::DataItem;
pub(crate) use data_name::DataName;
pub(crate) use role_name::RoleName;

pub(crate) type FunctionItem = syn::ImplItemFn;

pub(crate) type MethodName = proc_macro2::Ident;
pub(crate) type PayloadPath = syn::Path;

pub(crate) use signature::{needs_context, remove_context_param};
