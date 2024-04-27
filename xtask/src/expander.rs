use anyhow::Error;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::visit::Visit;
use syn::{parse_file, AttrStyle, Attribute, Item, Meta, MetaList};

use crate::stripped_macro::{make_actor, make_performance, make_role};

fn find_attribute<'a>(attrs: &'a Vec<Attribute>, needle: &str) -> Option<&'a Attribute> {
	for a in attrs {
		if a.style == AttrStyle::Outer {
			match &a.meta {
				Meta::Path(path) | Meta::List(MetaList { path, .. }) => {
					let Some(leaf) = path.segments.last() else {
						continue;
					};
					if leaf.ident.eq(needle) {
						return Some(a);
					}
				}
				Meta::NameValue(_) => continue,
			}
		}
	}
	None
}
/*
fn expand_macro(attrs: &Vec<Attribute>, thing: impl ToTokens) -> TokenStream {
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
} */

#[derive(Default)]
struct Walker(TokenStream);

impl<'ast> Visit<'ast> for Walker {
	fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
		let attrs = &i.attrs;
		let present = find_attribute(attrs, "actor");
		if present.is_some() {
			let tokens = match make_actor(i.clone()) {
				Ok(actor_ouput) => actor_ouput.to_token_stream(),
				Err(e) => e.into_compile_error(),
			};
			self.0.extend(tokens);
		} else {
			let mut subwalker = Walker::default();

			if let Some((_, items)) = i.content.as_ref() {
				for item in items {
					subwalker.visit_item(item);
				}

				self.0.extend(subwalker.0);
			} else {
				self.0.extend(i.into_token_stream());
			}
		}
	}

	fn visit_item_trait(&mut self, i: &'ast syn::ItemTrait) {
		let attrs = &i.attrs;
		let present = find_attribute(attrs, "role");
		if present.is_some() {
			let tokens = match make_role(i.clone()) {
				Ok(actor_ouput) => actor_ouput.to_token_stream(),
				Err(e) => e.into_compile_error(),
			};
			self.0.extend(tokens);
		} else {
			self.0.extend(i.into_token_stream());
		}
	}

	fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
		let attrs = &i.attrs;
		let present = find_attribute(attrs, "performance");
		if present.is_some() {
			let tokens = match make_performance(i.clone()) {
				Ok(actor_ouput) => actor_ouput.to_token_stream(),
				Err(e) => e.into_compile_error(),
			};
			self.0.extend(tokens);
		} else {
			self.0.extend(i.into_token_stream());
		}
	}

	fn visit_item(&mut self, i: &'ast Item) {
		//dbg!(&i);
		match i {
			syn::Item::Impl(i) => self.visit_item_impl(i),
			syn::Item::Mod(i) => self.visit_item_mod(i),
			syn::Item::Trait(i) => self.visit_item_trait(i),
			els => self.0.extend(els.into_token_stream()),
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
	use std::fs::create_dir_all;
	use std::io::{Read, Write};
	use std::path::PathBuf;
	use std::process::{Command, Stdio};

	let src = PathBuf::from(std::env!("CARGO_MANIFEST_DIR")).join("../tests/");
	let dest = PathBuf::from(std::env!("CARGO_MANIFEST_DIR")).join("tests/expanded/");

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
