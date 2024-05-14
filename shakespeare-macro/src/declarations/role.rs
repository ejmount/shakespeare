use syn::{Path, Signature, Visibility};

pub(crate) struct RoleDecl {
	pub(crate) name: Path,
	pub(crate) vis: Visibility,
	pub(crate) signatures: Vec<Signature>,
}

impl RoleDecl {
	pub(crate) fn new(
		name: Path,
		vis: Visibility,
		signatures: impl Iterator<Item = Signature>,
	) -> RoleDecl {
		let signatures = signatures.collect();
		RoleDecl {
			name,
			vis,
			signatures,
		}
	}
}
