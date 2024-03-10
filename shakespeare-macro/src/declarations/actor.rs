use itertools::Itertools;
use syn::{parse_quote, Error, ImplItem, Item, ItemMod, Visibility};

use crate::data::{ActorName, DataItem};
use crate::declarations::performance::PerformanceAttribute;
use crate::macros::{fallible_quote, filter_unwrap};
use crate::{PerformanceDeclaration, RoleDecl};

enum ActorInternal {
	Performance(PerformanceDeclaration),
	CanonPerformance(PerformanceDeclaration, RoleDecl),
	Data(DataItem),
}

pub struct ActorDecl {
	pub actor_name:   ActorName,
	pub actor_vis:    Visibility,
	pub data_item:    DataItem,
	pub performances: Vec<PerformanceDeclaration>,
	pub roles:        Vec<RoleDecl>,
}

type Fallible<T> = Result<Option<T>, Error>;

static HANDLERS: &[fn(&Item) -> Fallible<ActorInternal>] =
	&[read_performance as _, read_data_item as _];

impl ActorDecl {
	pub fn new(module: &ItemMod) -> Result<ActorDecl, Error> {
		let Some((_, items)) = &module.content else {
			return Err(Error::new_spanned(
				module,
				"Actor declaration cannot be empty",
			));
		};

		let actor_vis = module.vis.clone();

		let actor_name = &module.ident;
		let actor_name = fallible_quote! { #actor_name }?;

		let mut performances = vec![];
		let mut roles = vec![];
		let mut data = vec![];

		for item in items {
			for handler in HANDLERS {
				match handler(item)? {
					Some(ActorInternal::CanonPerformance(perf, role)) => {
						performances.push(perf);
						roles.push(role);
					}
					Some(ActorInternal::Performance(perf)) => performances.push(perf),
					Some(ActorInternal::Data(item)) => data.push(item),
					None => continue,
				}
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

		assert!(!performances.is_empty(), "Empty perfs"); // Because [SpawningFunction] falls over otherwise

		Ok(ActorDecl {
			actor_name,
			actor_vis,
			data_item,
			performances,
			roles,
		})
	}
}

fn read_performance(item: &Item) -> Fallible<ActorInternal> {
	fn get_performance_tag(imp: &syn::ItemImpl) -> Option<&syn::Attribute> {
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
	let arg: PerformanceAttribute = attr.parse_args()?;
	let canonical = arg.canonical;
	let role_name = imp.trait_.clone().unwrap().1;
	let perf = PerformanceDeclaration::new(role_name.clone(), imp.clone())?;
	if canonical {
		let signatures = filter_unwrap!(imp.items.iter(), ImplItem::Fn)
			.map(|f| &f.sig)
			.cloned();

		let role = RoleDecl::new(role_name, parse_quote! {pub}, signatures);
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

fn combine_errors(mut one: Error, another: Error) -> Error {
	one.combine(another);
	one
}
