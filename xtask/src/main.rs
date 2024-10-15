#![warn(clippy::pedantic)]
#![warn(unused_crate_dependencies)]
use std::env::set_current_dir;
use std::path::{Path, PathBuf};

use anyhow::Error;
use clap::{Parser, Subcommand};
use colored::{Color, Colorize};
use dialoguer::Confirm;
use duct::cmd;
use fs_extra::dir::{create_all, get_dir_content};
use fs_extra::file::remove;
#[cfg(test)]
use shakespeare as _;
#[cfg(test)]
use tokio as _;
#[cfg(test)]
use trybuild as _;

mod expander;
#[path = "stripped_macro/lib.rs"]
#[allow(clippy::all)]
#[allow(clippy::pedantic)]
#[allow(warnings)]
mod stripped_macro;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
	#[command(subcommand)]
	subcommand: Commands,
}

#[derive(Subcommand)]
enum Commands {
	/// Run tests and compile coverage reports
	Coverage {
		/// Output coverage results as HTML rather than .lcov
		#[arg(short, long)]
		readable:    bool,
		/// Whether to open HTML reports - only used if `readable` is true. Will
		/// prompt if not given
		#[arg(short, long)]
		open_report: Option<bool>,
	},
	Expand,
}

fn main() -> Result<(), Error> {
	let cli = Cli::parse();

	match cli.subcommand {
		Commands::Coverage {
			readable,
			open_report,
		} => coverage(readable, open_report),
		Commands::Expand => expander::expand_all_tests(),
	}
}

fn coverage(readable: bool, open_report: Option<bool>) -> Result<(), Error> {
	set_current_dir(root_crate_dir())?;

	create_all("coverage", true)?;
	set_current_dir("shakespeare-macro")?;

	print!("Running macro tests... ");
	cmd!("cargo", "test")
		.env("CARGO_INCREMENTAL", "0")
		.env("RUSTFLAGS", "-Cinstrument-coverage")
		.env("LLVM_PROFILE_FILE", "../coverage/cargo-test-%p-%m.profraw")
		.run()?;
	println!("{}", "ok".color(Color::Green));

	set_current_dir(root_crate_dir())?;
	print!("Running main tests... ");
	cmd!("cargo", "test")
		.env("CARGO_INCREMENTAL", "0")
		.env("RUSTFLAGS", "-Cinstrument-coverage")
		.env("LLVM_PROFILE_FILE", "coverage/cargo-test-%p-%m.profraw")
		.run()?;
	println!("{}", "ok".color(Color::Green));

	let (fmt, file) = if readable {
		("html", "coverage/html")
	} else {
		("lcov", "coverage/tests.lcov")
	};

	//set_current_dir(root_crate_dir())?;

	print!("Generating reports as {fmt}... ");
	cmd!(
		"grcov",
		".",
		"--binary-path",
		"./target/debug/deps",
		"-s",
		".",
		"-t",
		fmt,
		"-o",
		file,
		"--branch",
		"--ignore-not-existing",
		"--ignore",
		"**/tests/*",
		"--ignore",
		"xtask/*",
		"--excl-start",
		"mod tests",
	)
	.run()?;
	println!("{}", "ok".color(Color::Green));

	if readable {
		let index_file = format!("{file}/index.html");

		if open_report.map_or_else(|| confirm("open report folder?"), Result::Ok)? {
			match open::that(&index_file) {
				Ok(()) => {
					println!("{}", "Opened".color(Color::Green));
				}
				Err(e) => {
					eprintln!("{e}\n{} to open reports", "Failure".color(Color::Red));
				}
			}
		} else {
			let abs_path = Path::new(&index_file).canonicalize()?;
			println!("report location: {}", abs_path.to_string_lossy());
		}
	}
	print!("Cleaning up... ");
	let dir_content = get_dir_content(".")?;
	for prof_file in dir_content.files.iter().filter(|s| s.ends_with("profraw")) {
		remove(prof_file)?;
	}
	println!("{}", "ok".color(Color::Green));
	Ok(())
}

/// Get the root folder of the larger crate, assuming this is part of a
/// workspace
fn root_crate_dir() -> PathBuf {
	let mut xtask_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	xtask_dir.pop();
	xtask_dir
}

/// Prompt the user to confirm an action
fn confirm(question: &str) -> Result<bool, dialoguer::Error> {
	Confirm::new().with_prompt(question).interact()
}
