use syn::{Attribute, Path, Signature, Visibility};

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
		let signatures = signatures.collect();
		RoleDecl {
			name,
			attributes,
			vis,
			signatures,
		}
	}
}
