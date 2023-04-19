use itertools::Itertools;
use quote::ToTokens;
use syn::{parse_quote, Error, ImplItem, ItemImpl, ItemMod, Type, Visibility};

use super::performance::{PerformanceAttribute, PerformanceDeclaration};
use super::role::RoleDecl;
use crate::data::{ActorName, DataItem};
use crate::macros::{fallible_quote, filter_unwrap};

pub struct ActorDecl {
	pub actor_name:   ActorName,
	pub actor_vis:    Visibility,
	pub data_item:    DataItem,
	pub performances: Vec<PerformanceDeclaration>,
	pub roles:        Vec<RoleDecl>,
	pub others:       Vec<ItemImpl>,
}

impl ActorDecl {
	pub fn new(module: ItemMod) -> Result<ActorDecl, Error> {
		let inline_err = Err(Error::new_spanned(
			&module,
			"Actor declaration cannot be empty",
		));
		let missing_err = Err(Error::new_spanned(
			&module,
			"Actor declaration must contain one struct, enum or union",
		));
		let Some((_, items)) = module.content else {
			return inline_err;
		};

		let actor_vis = module.vis;

		let data_item = match items.iter().flat_map(DataItem::try_from).at_most_one() {
			Ok(Some(item)) => item,
			Ok(None) => return missing_err,
			Err(extras) => {
				let errors = extras.map(|d| {
					Error::new_spanned(d, "Only one data item allowed in actor declaration")
				});
				return Err(errors.reduce(combine_errors).unwrap());
			}
		};

		let actor_name = module.ident;
		let actor_name = fallible_quote! { #actor_name }?;

		let mut performances = vec![];
		let mut roles = vec![];
		let mut others = vec![];

		for item in items {
			if let syn::Item::Impl(imp) = item {
				if let Some(attr) = get_performance_tag(&imp) {
					let arg: PerformanceAttribute = attr.parse_args()?;
					let canonical = arg.canonical;

					let role_name = imp.trait_.clone().unwrap().1;

					if canonical {
						let signatures = filter_unwrap!(imp.items.iter(), ImplItem::Fn)
							.map(|f| &f.sig)
							.cloned();

						let role = RoleDecl::new(role_name.clone(), parse_quote! {pub}, signatures);
						roles.push(role);
					}

					performances.push(PerformanceDeclaration::new(role_name, imp)?);
				} else if let Type::Path(p) = &*imp.self_ty {
					if p.path == data_item.name().0 {
						others.push(imp);
					} else {
						panic!(
							"Path: {}, ident: {:?}",
							p.path.to_token_stream(),
							data_item.name(),
						);
					}
				} else {
					continue;
				}
			}
		}
		assert!(!performances.is_empty(), "Empty perfs"); // Because [SpawningFunction] falls over otherwise

		Ok(ActorDecl {
			actor_name,
			actor_vis,
			data_item,
			performances,
			roles,
			others,
		})
	}
}

fn get_performance_tag(imp: &syn::ItemImpl) -> Option<&syn::Attribute> {
	imp.attrs.iter().find(|attr| {
		attr.path()
			.segments
			.last()
			.is_some_and(|ps| ps.ident == "performance")
	})
}

fn combine_errors(mut one: Error, another: Error) -> Error {
	one.combine(another);
	one
}
