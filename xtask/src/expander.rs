use std::fs::create_dir_all;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;

use anyhow::Error;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::visit::Visit;
use syn::{
	parse2, parse_file, AttrStyle, Attribute, ItemConst, ItemEnum, ItemFn, ItemImpl, ItemMacro,
	ItemMod, ItemStatic, ItemStruct, ItemTrait, ItemTraitAlias, ItemType, ItemUnion, ItemUse, Meta,
	MetaList,
};

use crate::stripped_macro::{actor, performance, role};

fn expand_macro(attrs: &Vec<syn::Attribute>, thing: impl ToTokens) -> TokenStream {
	for a in attrs {
		if a.style == AttrStyle::Outer {
			match &a.meta {
				Meta::Path(path) | Meta::List(MetaList { path, .. }) => {
					let Some(leaf) = path.segments.last() else {
						continue;
					};
					if leaf.ident.eq("actor") {
						let attr_tokens = attrs
							.iter()
							.cloned()
							.map(Attribute::into_token_stream)
							.collect();

						return actor(attr_tokens, thing.into_token_stream());
					} else if leaf.ident.eq("role") {
						let attr_tokens = attrs
							.iter()
							.cloned()
							.map(Attribute::into_token_stream)
							.collect();

						return role(attr_tokens, thing.into_token_stream());
					} else if leaf.ident.eq("performance") {
						let attr_tokens = attrs
							.iter()
							.cloned()
							.map(Attribute::into_token_stream)
							.collect();

						return performance(attr_tokens, thing.into_token_stream());
					}
				}
				Meta::NameValue(_) => continue,
			}
		}
	}
	thing.into_token_stream()
}

#[derive(Default)]
struct Walker(TokenStream);

impl<'ast> Visit<'ast> for Walker {
	fn visit_item(&mut self, i: &'ast syn::Item) {
		let attrs = match &i {
			syn::Item::Const(ItemConst { attrs, .. })
			| syn::Item::Enum(ItemEnum { attrs, .. })
			| syn::Item::Fn(ItemFn { attrs, .. })
			| syn::Item::Impl(ItemImpl { attrs, .. })
			| syn::Item::Macro(ItemMacro { attrs, .. })
			| syn::Item::Mod(ItemMod { attrs, .. })
			| syn::Item::Static(ItemStatic { attrs, .. })
			| syn::Item::Struct(ItemStruct { attrs, .. })
			| syn::Item::Trait(ItemTrait { attrs, .. })
			| syn::Item::TraitAlias(ItemTraitAlias { attrs, .. })
			| syn::Item::Type(ItemType { attrs, .. })
			| syn::Item::Union(ItemUnion { attrs, .. })
			| syn::Item::Use(ItemUse { attrs, .. }) => attrs.clone(),

			_ => unimplemented!(),
		};
		if attrs.is_empty()
			|| matches!(
				i,
				syn::Item::Fn(_) | syn::Item::Struct(_) | syn::Item::Use(_) | syn::Item::Static(_)
			) {
			self.0.extend(i.into_token_stream());
		} else {
			let first_pass = expand_macro(&attrs, i);
			let new_items: syn::File = parse2(first_pass).unwrap();
			let tokens = if new_items.items.len() > 1
				&& new_items
					.items
					.iter()
					.any(|i| matches!(i, syn::Item::Mod(_)))
			{
				let mut subwalker = Walker::default();
				subwalker.visit_file(&new_items);
				subwalker.0
			} else {
				new_items.into_token_stream()
			};
			self.0.extend(tokens);
		}
	}
}

fn expand_test(contents: &str) -> Result<TokenStream, Error> {
	let file = parse_file(contents)?;

	let mut walker = Walker::default();
	walker.visit_file(&file);
	let Walker(tok) = walker;
	Ok(tok)
}

pub fn expand_all_tests() -> Result<(), Error> {
	let src = PathBuf::from(std::env!("CARGO_MANIFEST_DIR")).join("tests/");
	let dest = PathBuf::from_str("../expanded/tests/")?;

	for test_file in walkdir::WalkDir::new(&src).into_iter().filter_entry(|f| {
		f.metadata().unwrap().is_dir() || f.path().extension().is_some_and(|p| p == "rs")
	}) {
		let test_file = test_file?;

		let new_path = dest.join(test_file.path().strip_prefix(&src)?);

		if test_file.metadata()?.is_dir() {
			create_dir_all(&new_path).unwrap_or_else(|_| panic!("{new_path:?} invalid to create"));
			continue;
		}
		let contents = std::fs::read_to_string(test_file.path())?;

		let expanded = expand_test(&contents)?;

		let child = Command::new("rustup")
			.arg("run")
			.arg("nightly")
			.arg("rustfmt")
			.arg("--edition")
			.arg("2021")
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.spawn()?;

		child
			.stdin
			.unwrap()
			.write_all(expanded.to_string().as_bytes())
			.unwrap();
		let mut output = String::default();
		unsafe {
			child
				.stdout
				.unwrap()
				.read_to_end(output.as_mut_vec())
				.unwrap();
		}

		std::fs::write(new_path, output)?;
	}
	Ok(())
}
