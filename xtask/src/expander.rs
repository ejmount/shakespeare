use anyhow::Error;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::visit_mut::VisitMut;
use syn::{parse_file, AttrStyle, Attribute, Item, ItemMod, Meta, MetaList};

use crate::stripped_macro::{make_actor, make_performance, make_role};

fn find_attribute(attrs: &mut Vec<Attribute>, needle: &str) -> Option<Attribute> {
	let mut index = None;

	for (ind, attr) in attrs.iter().enumerate() {
		if attr.style == AttrStyle::Outer {
			match &attr.meta {
				Meta::Path(path) | Meta::List(MetaList { path, .. }) => {
					let Some(leaf) = path.segments.last() else {
						continue;
					};
					if leaf.ident.eq(needle) {
						index = Some(ind);
					}
				}
				Meta::NameValue(_) => continue,
			}
		}
	}

	index.map(|index| attrs.swap_remove(index))
}

#[derive(Default)]
struct Walker(TokenStream);

impl VisitMut for Walker {
	fn visit_item_mod_mut(&mut self, i: &mut syn::ItemMod) {
		let attrs = &mut i.attrs;
		let present = find_attribute(attrs, "actor");
		if present.is_some() {
			let tokens = match make_actor(i.clone()) {
				Ok(actor_ouput) => actor_ouput.to_token_stream(),
				Err(e) => e.into_compile_error(),
			};
			self.0.extend(tokens);
		} else {
			let mut subwalker = Walker::default();

			let ItemMod {
				attrs,
				ident,
				content,
				unsafety,
				vis,
				..
			} = i;

			if let Some((_, items)) = content.as_mut() {
				for item in items {
					subwalker.visit_item_mut(item);
				}
				let Walker(tok) = subwalker;

				self.0.extend(quote! {
					#(#attrs)*
					 #vis #unsafety mod #ident {
						#tok
					}
				});
			} else {
				self.0.extend(i.into_token_stream());
			}
		}
	}

	fn visit_item_trait_mut(&mut self, i: &mut syn::ItemTrait) {
		let attrs = &mut i.attrs;
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

	fn visit_item_impl_mut(&mut self, i: &mut syn::ItemImpl) {
		let attrs = &mut i.attrs;
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

	fn visit_item_mut(&mut self, i: &mut Item) {
		//dbg!(&i);
		match i {
			syn::Item::Impl(i) => self.visit_item_impl_mut(i),
			syn::Item::Mod(i) => self.visit_item_mod_mut(i),
			syn::Item::Trait(i) => self.visit_item_trait_mut(i),
			els => self.0.extend(els.into_token_stream()),
		}
	}
}

fn expand_test(contents: &str) -> Result<TokenStream, Error> {
	let mut file = parse_file(contents)?;

	let mut walker = Walker::default();
	walker.visit_file_mut(&mut file);
	let Walker(tok) = walker;
	Ok(tok)
}

pub fn expand_all_tests() -> Result<(), Error> {
	use std::fs::create_dir_all;
	use std::io::{Read, Write};
	use std::path::PathBuf;
	use std::process::{Command, Stdio};

	let root = PathBuf::from(std::env!("CARGO_MANIFEST_DIR")).join("..");
	let src = root.join("tests/");
	let macro_src = root.join("shakespeare-macro/tests/");
	let dest = PathBuf::from(std::env!("CARGO_MANIFEST_DIR")).join("tests/expanded/");

	let main_test_files = walkdir::WalkDir::new(&src)
		.into_iter()
		.filter_entry(is_a_rust_file);
	let macro_test_files = walkdir::WalkDir::new(&macro_src)
		.into_iter()
		.filter_entry(is_a_rust_file);

	for test_file in main_test_files.chain(macro_test_files) {
		let test_file = test_file?;

		let new_path = dest.join(test_file.path().strip_prefix(&root)?);

		if test_file.metadata()?.is_dir() {
			create_dir_all(&new_path).unwrap_or_else(|_| panic!("{new_path:?} invalid to create"));
			continue;
		}
		let contents = std::fs::read_to_string(test_file.path())?;
		let contents = contents.replace("crate::", "crate::expanded::");

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

fn is_a_rust_file(f: &walkdir::DirEntry) -> bool {
	f.metadata().unwrap().is_dir() || f.path().extension().is_some_and(|p| p == "rs")
}
