use convert_case::{Case, Casing};
use itertools::Itertools;
use quote::format_ident;
use syn::{parse_quote, Attribute, Error, ImplItem, Item, ItemFn, ItemImpl, ItemMod, Visibility};

use crate::data::{ActorName, DataItem};
use crate::declarations::performance::PerformanceAttribute;
use crate::macros::{fallible_quote, filter_unwrap};
use crate::{PerformanceDecl, RoleDecl};

enum ActorInternal {
	Performance(PerformanceDecl),
	CanonPerformance(PerformanceDecl, RoleDecl),
	Data(DataItem),
	PanicHandler(ItemFn),
	ExitHandler(ItemFn),
}

pub(crate) struct ActorDecl {
	pub(crate) actor_name:    ActorName,
	pub(crate) actor_vis:     Visibility,
	pub(crate) data_item:     DataItem,
	pub(crate) panic_handler: Option<ItemFn>,
	pub(crate) exit_handler:  Option<ItemFn>,
	pub(crate) performances:  Vec<PerformanceDecl>,
	pub(crate) roles:         Vec<RoleDecl>,
	pub(crate) misc:          Vec<Item>,
}

type Fallible<T> = Result<Option<T>, Error>;

static HANDLERS: &[fn(&Item) -> Fallible<ActorInternal>] = &[
	read_performance as _,
	read_data_item as _,
	read_panic_handler as _,
	read_exit_handler as _,
];

impl ActorDecl {
	pub(crate) fn new(module: ItemMod) -> Result<ActorDecl, Error> {
		let mut performances = vec![];
		let mut roles = vec![];
		let mut data = vec![];
		let mut panic_handler = None;
		let mut exit_handler = None;
		let mut misc = vec![];

		let Some((_, items)) = &module.content else {
			return Err(Error::new_spanned(
				module,
				"Actor declaration cannot be empty",
			));
		};

		for item in items {
			let mut done = false;
			for handler in HANDLERS {
				let result = handler(item)?;
				done |= result.is_some();
				match result {
					Some(ActorInternal::CanonPerformance(perf, role)) => {
						performances.push(perf);
						roles.push(role);
					}
					Some(ActorInternal::Performance(perf)) => performances.push(perf),
					Some(ActorInternal::Data(item)) => data.push(item),
					Some(ActorInternal::PanicHandler(f)) => {
						if let Some(panic_fn) = panic_handler.replace(f) {
							return Err(Error::new_spanned(panic_fn, "Duplicate panic handler"));
						}
					}
					Some(ActorInternal::ExitHandler(f)) => {
						if let Some(exit_fn) = exit_handler.replace(f) {
							return Err(Error::new_spanned(exit_fn, "Duplicate exit handler"));
						}
					}
					None => continue,
				}
			}
			if !done {
				misc.push(item.clone());
			}
		}

		let data_item = match data.into_iter().at_most_one() {
			Ok(Some(item)) => item,
			Ok(None) => {
				return Err(Error::new_spanned(
					module,
					"Actor declaration must contain one struct, enum or union",
				))
			}
			Err(extras) => {
				let errors = extras.map(|d| {
					Error::new_spanned(d, "Only one data item allowed in actor declaration")
				});
				return Err(errors.reduce(combine_errors).unwrap());
			}
		};

		let actor_ident = &module.ident;
		let actor_name = ActorName::new(fallible_quote! { #actor_ident }?);

		// Replace this with an impl block so we don't have to disambigate
		if let Some(handler) = panic_handler.as_mut() {
			let s = format!("{}_{}", actor_ident, handler.sig.ident);
			handler.sig.ident = format_ident!("{}", s.to_case(Case::Snake));
		}
		if let Some(handler) = exit_handler.as_mut() {
			let s = format!("{}_{}", actor_ident, handler.sig.ident);
			handler.sig.ident = format_ident!("{}", s.to_case(Case::Snake));
		}

		assert!(!performances.is_empty(), "Empty perfs");
		// [SpawningFunction] might fall over otherwise
		// And also doesn't make much sense

		Ok(ActorDecl {
			actor_name,
			actor_vis: module.vis,
			data_item,
			panic_handler,
			exit_handler,
			performances,
			roles,
			misc,
		})
	}
}

fn read_performance(item: &Item) -> Fallible<ActorInternal> {
	fn get_performance_tag(imp: &ItemImpl) -> Option<&Attribute> {
		imp.attrs.iter().find(|attr| {
			attr.path()
				.segments
				.last()
				.is_some_and(|ps| ps.ident == "performance")
		})
	}
	let syn::Item::Impl(imp) = item else {
		return Ok(None);
	};

	let Some(attr) = get_performance_tag(imp) else {
		return Ok(None);
	};

	let role_name = &imp.trait_.as_ref().unwrap().1;
	let perf = PerformanceDecl::new(role_name.clone(), imp.clone())?;

	let args: Option<PerformanceAttribute> = attr.parse_args().ok();
	let canonical = args.map_or(false, |args| args.canonical.value());

	if canonical {
		let signatures = filter_unwrap!(imp.items.iter(), ImplItem::Fn)
			.map(|f| &f.sig)
			.cloned();

		let role = RoleDecl::new(role_name.clone(), parse_quote! {pub}, signatures);
		Ok(ActorInternal::CanonPerformance(perf, role).into())
	} else {
		Ok(ActorInternal::Performance(perf).into())
	}
}

#[allow(clippy::unnecessary_wraps)]
fn read_data_item(item: &Item) -> Fallible<ActorInternal> {
	let Ok(data_item) = DataItem::try_from(item) else {
		return Ok(None);
	};
	Ok(Some(ActorInternal::Data(data_item)))
}

#[allow(clippy::unnecessary_wraps)]
fn read_panic_handler(item: &Item) -> Fallible<ActorInternal> {
	match item {
		Item::Fn(f) if f.sig.ident.eq("catch") => Ok(Some(ActorInternal::PanicHandler(f.clone()))),
		_ => Ok(None),
	}
}

#[allow(clippy::unnecessary_wraps)]
fn read_exit_handler(item: &Item) -> Fallible<ActorInternal> {
	match item {
		Item::Fn(f) if f.sig.ident.eq("stop") => Ok(Some(ActorInternal::ExitHandler(f.clone()))),
		_ => Ok(None),
	}
}

fn combine_errors(mut one: Error, another: Error) -> Error {
	one.combine(another);
	one
}
