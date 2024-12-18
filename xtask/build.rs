use std::fs::{create_dir_all, read};
use std::path::PathBuf;
use std::str::FromStr;

use walkdir::WalkDir;

/// This is a tremendous hack
fn main() -> Result<(), walkdir::Error> {
	let original_src = "../shakespeare-macro/src";
	let dest = PathBuf::from_str("src/stripped_macro").unwrap();

	for entry in WalkDir::new(original_src) {
		let entry = entry?;

		let original_path = entry.path();
		println!("cargo::rerun-if-changed={original_path:?}");

		let new_path = dest.join(
			original_path
				.strip_prefix(original_src)
				.expect("Prefix missing???"),
		);

		if entry.metadata()?.is_file() && entry.path().extension().unwrap() == "rs" {
			use std::io::Write;
			let contents = String::from_utf8(read(original_path).expect("Read error")).unwrap();

			let mut new_file = std::fs::File::create(new_path).unwrap();

			for line in contents.lines() {
				if line.starts_with("#![warn(")
					|| line.starts_with("#![deny(")
					|| line.starts_with("#![forbid(")
					|| line.contains("#[test] // EXPANDER EXCLUDE")
				{
					continue;
				}

				let line = line.replace("#[proc_macro_attribute]", "");
				let line = line.replace("crate::", "crate::stripped_macro::");
				let line = line.replace("proc_macro::", "proc_macro2::");

				writeln!(new_file, "{line}").unwrap();
			}
		} else if entry.metadata()?.is_dir() && !new_path.exists() {
			create_dir_all(&new_path).unwrap_or_else(|_| panic!("{new_path:?} invalid to create"));
		}
	}

	Ok(())
}
