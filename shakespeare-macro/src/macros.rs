macro_rules! filter_unwrap {
	($list:expr, $pat:path) => {
		$list
			.into_iter()
			.filter_map(|item| if let $pat(a) = item { Some(a) } else { None })
	};
}
pub(crate) use filter_unwrap;

macro_rules! fallible_quote {
	($($tt:tt)*) => {
        {
            use syn::parse::Parser;

            let tokens = quote::quote! { $($tt)* };
		    syn::parse::Parse::parse.parse2(tokens.clone()).map_err(|e| {
			syn::parse::Error::new(
				e.span(),
				format!("shakespeare: internal error: {} at file {}:{} - this is likely a bug\n{tokens}", e, file!(), line!()),
	    		)
    		})
        }
	};
}

pub(crate) use fallible_quote;

macro_rules! map_or_bail {
	($iter:expr, $closure:expr) => {{
		let results: ::std::result::Result<Vec<_>, _> = $iter.into_iter().map($closure).collect();
		results?
	}};
}

pub(crate) use map_or_bail;
