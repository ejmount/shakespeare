use itertools::Itertools;
use syn::{Attribute, Path, Signature, Visibility};

use crate::data::remove_context_param;

pub(crate) struct RoleDecl {
	pub(crate) name:       Path,
	pub(crate) attributes: Vec<Attribute>,
	pub(crate) vis:        Visibility,
	pub(crate) signatures: Vec<Signature>,
}

impl RoleDecl {
	pub(crate) fn new(
		name: Path,
		attributes: Vec<Attribute>,
		vis: Visibility,
		signatures: impl Iterator<Item = Signature>,
	) -> RoleDecl {
		let mut signatures = signatures.collect_vec();

		signatures.iter_mut().for_each(remove_context_param);

		RoleDecl {
			name,
			attributes,
			vis,
			signatures,
		}
	}
}
