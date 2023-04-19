use syn::{Path, Signature, Visibility};

pub struct RoleDecl {
	pub name:       Path,
	pub vis:        Visibility,
	pub signatures: Vec<Signature>,
}

impl RoleDecl {
	pub fn new(
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
