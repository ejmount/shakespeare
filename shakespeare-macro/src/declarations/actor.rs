use itertools::Itertools;
use syn::spanned::Spanned;
use syn::{
	Attribute, Error, ImplItem, Item, ItemImpl, ItemMod, Path, Result, TypePath, Visibility,
};

use crate::data::{ActorName, DataItem, HandlerFunctions};
use crate::declarations::performance::PerformanceAttribute;
use crate::macros::filter_unwrap;
use crate::{PerformanceDecl, RoleDecl};

pub(crate) struct ActorDecl {
	pub(crate) actor_name:   ActorName,
	pub(crate) attributes:   Vec<Attribute>,
	pub(crate) actor_vis:    Visibility,
	pub(crate) data_item:    DataItem,
	pub(crate) handlers:     HandlerFunctions,
	pub(crate) performances: Vec<PerformanceDecl>,
	pub(crate) roles:        Vec<RoleDecl>,
	pub(crate) misc:         Vec<Item>,
}

impl ActorDecl {
	pub(crate) fn new(module: ItemMod) -> Result<ActorDecl> {
		let module_span = module.span();
		let ItemMod {
			attrs,
			vis: actor_vis,
			ident,
			content,
			..
		} = module;

		let mut performances = vec![];
		let mut roles = vec![];
		let mut data_items = vec![];
		let mut misc = vec![];

		let mut handlers = HandlerFunctions::new();

		let Some((_, items)) = content else {
			return Err(Error::new(module_span, "Actor declaration cannot be empty"));
		};

		for item in items {
			match &item {
				Item::Impl(imp) => {
					if let Some((perf, role)) = read_performance(imp)? {
						performances.push(perf);
						if let Some(role) = role {
							roles.push(role);
						}
						continue;
					}
				}
				Item::Fn(item) => {
					if handlers.add(item) {
						continue;
					}
				}
				other => {
					if let Ok(item) = DataItem::try_from(other) {
						data_items.push(item);
						continue;
					}
				}
			}

			misc.push(item);
		}

		let data_item = match data_items.into_iter().at_most_one() {
			Ok(Some(item)) => item,
			Ok(None) => {
				return Err(Error::new(
					module_span,
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

		handlers.set_data_name(data_item.name());

		let actor_path = TypePath {
			qself: None,
			path:  Path::from(ident),
		};
		let actor_name = ActorName::new(actor_path);

		if performances.is_empty() {
			return Err(Error::new(
				module_span,
				"Actor must have at least one performance, even if it's externally defined",
			));
		}
		// [SpawningFunction] might fall over otherwise
		// And also doesn't make much sense

		let attributes = attrs
			.iter()
			.filter(is_not_internal_attribute)
			.cloned()
			.collect();

		Ok(ActorDecl {
			actor_name,
			attributes,
			actor_vis,
			data_item,
			handlers,
			performances,
			roles,
			misc,
		})
	}
}

fn read_performance(imp: &ItemImpl) -> Result<Option<(PerformanceDecl, Option<RoleDecl>)>> {
	fn get_performance_tag(imp: &ItemImpl) -> Option<&Attribute> {
		imp.attrs.iter().find(|attr| {
			attr.path()
				.segments
				.last()
				.is_some_and(|ps| ps.ident == "performance")
		})
	}

	let Some(attr) = get_performance_tag(imp) else {
		return Ok(None);
	};

	let (_, role_name, _) = &imp.trait_.as_ref().unwrap();
	let perf = PerformanceDecl::new(role_name.clone(), imp.clone())?;

	let args: Option<PerformanceAttribute> = attr.parse_args().ok();
	let canonical = args.is_some_and(|args| args.canonical.value());

	if canonical {
		let signatures = filter_unwrap!(&imp.items, ImplItem::Fn)
			.map(|f| &f.sig)
			.cloned();

		let attributes = imp
			.attrs
			.iter()
			.filter(is_not_internal_attribute)
			.cloned()
			.collect();

		let role = RoleDecl::new(
			role_name.clone(),
			attributes,
			Visibility::Public(syn::token::Pub::default()),
			signatures,
		);
		Ok(Some((perf, Some(role))))
	} else {
		Ok(Some((perf, None)))
	}
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_not_internal_attribute(a: &&Attribute) -> bool {
	let Some(last) = a.path().segments.last() else {
		return true;
	};
	let ident = &last.ident;

	!(ident == "actor" || ident == "performance" || ident == "role")
}

fn combine_errors(mut one: Error, another: Error) -> Error {
	one.combine(another);
	one
}
