use quote::ToTokens;
use syn::{Item, ItemEnum, ItemStruct, ItemUnion, Path, PathSegment};

use super::DataName;

#[derive(Clone, Debug)]
pub(crate) enum DataItem {
	Struct(ItemStruct),
	Enum(ItemEnum),
	Union(ItemUnion),
}

impl DataItem {
	pub(crate) fn name(&self) -> DataName {
		match self {
			DataItem::Struct(ItemStruct { ident, .. })
			| DataItem::Enum(ItemEnum { ident, .. })
			| DataItem::Union(ItemUnion { ident, .. }) => {
				let seg = PathSegment::from(ident.clone());
				let segments = std::iter::once(seg).collect();
				DataName::new(Path {
					segments,
					leading_colon: None,
				})
			}
		}
	}
}
impl TryFrom<&Item> for DataItem {
	type Error = ();

	fn try_from(value: &Item) -> Result<Self, Self::Error> {
		match value {
			Item::Struct(s) => Ok(DataItem::Struct(s.clone())),
			Item::Enum(e) => Ok(DataItem::Enum(e.clone())),
			Item::Union(u) => Ok(DataItem::Union(u.clone())),
			_ => Err(()),
		}
	}
}

impl ToTokens for DataItem {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		match self {
			DataItem::Struct(s) => s.to_tokens(tokens),
			DataItem::Enum(e) => e.to_tokens(tokens),
			DataItem::Union(u) => u.to_tokens(tokens),
		}
	}
}
