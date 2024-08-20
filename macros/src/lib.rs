use {
    proc_macro2::TokenStream,
    quote::quote,
    regex::Regex,
    std::path::{Path, PathBuf},
    syn::{parse_macro_input, LitStr},
};

#[proc_macro]
pub fn build_from_filesystem(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let dir = Path::new(&parse_macro_input!(input as LitStr).value()).join("routes");
    process_dir(&dir, None)
        .map(|x| x.0)
        .unwrap_or_else(|| {
            quote! {
                svelte_path_matcher::SveltePathMatcher {
                    children: &[],
                    terminating: false,
                }
            }
        })
        .into()
}

enum SegmentType {
    Ignore,
    OptionalWildcard,
    RepeatedWildcard,
    Wildcard,
    Static,
    ComplexWildcard,
}

impl SegmentType {
    fn from_str(segment: &str) -> Self {
        match segment {
            x if x.starts_with("(") && x.ends_with(")") => Self::Ignore,
            x if x.starts_with("[[") && x.ends_with("]]") => Self::OptionalWildcard,
            x if x.starts_with("[...") && x.ends_with("]") => Self::RepeatedWildcard,
            x if x.starts_with("[") && x.ends_with("]") => Self::Wildcard,
            segment => {
                let mut wildcard_started = false;
                for char in segment.chars() {
                    if char == '[' {
                        wildcard_started = true;
                    } else if char == ']' && wildcard_started {
                        return Self::ComplexWildcard;
                    }
                }
                Self::Static
            }
        }
    }
}

fn process_dir(path: &PathBuf, segment: Option<&str>) -> Option<(TokenStream, bool)> {
    lazy_static::lazy_static! {
        static ref TERMINATING_REGEX: Regex = Regex::new(r"\+page(|@.*)\.svelte").unwrap();
    }
    let mut items = TokenStream::new();
    let mut terminating = false;

    for file in std::fs::read_dir(path).unwrap() {
        let file = file.unwrap().path();
        let file_name = file.file_name().unwrap().to_str().unwrap();
        if file.is_dir() {
            if let Some((new_items, terminating_parent)) = process_dir(&file, Some(file_name)) {
                items.extend(Some(new_items));
                if terminating_parent {
                    terminating = true;
                }
            }
        } else if file.is_file() && TERMINATING_REGEX.is_match(file_name) {
            terminating = true;
        }
    }

    if !terminating && items.is_empty() {
        return None;
    }

    let segment = match segment {
        Some(x) => x, // TODO: escape sequences
        None => {
            return Some((
                quote! {
                    svelte_path_matcher::SveltePathMatcher {
                        children: &[#items],
                        terminating: #terminating,
                    }
                },
                false,
            ));
        }
    };

    let segment_type = SegmentType::from_str(segment);
    match segment_type {
        SegmentType::Ignore => Some((items, terminating)),
        segment_type => {
            let segment = match segment_type {
                SegmentType::Ignore => unreachable!(),
                SegmentType::Static => quote! { static_str(#segment) },
                SegmentType::Wildcard => quote! { Wildcard },
                SegmentType::OptionalWildcard => quote! { OptionalWildcard },
                SegmentType::RepeatedWildcard => quote! { RepeatedWildcard },
                SegmentType::ComplexWildcard => {
                    let segment = segment.as_bytes();
                    let mut items = TokenStream::new();
                    let mut segment_start = 0;
                    let mut wildcard = None; // (start, level)

                    for (i, char) in segment.iter().enumerate() {
                        match &mut wildcard {
                            None => {
                                if *char == b'[' {
                                    wildcard = Some((i, 1));
                                }
                            }
                            Some((start, level)) => {
                                if *char == b'[' {
                                    *level += 1;
                                } else if *char == b']' {
                                    *level -= 1;
                                    if *level == 0 {
                                        let static_part = unsafe {
                                            std::str::from_utf8_unchecked(
                                                &segment[segment_start..*start],
                                            )
                                        };
                                        if !static_part.is_empty() {
                                            items.extend(Some(quote! { svelte_path_matcher::ComplexWildcard::static_str(#static_part), }));
                                        }
                                        items.extend(Some(quote! { svelte_path_matcher::ComplexWildcard::Wildcard, }));
                                        segment_start = i + 1;
                                        wildcard = None;
                                    }
                                }
                            }
                        }
                    }

                    let static_part =
                        unsafe { std::str::from_utf8_unchecked(&segment[segment_start..]) };
                    if !static_part.is_empty() {
                        items.extend(Some(quote! { svelte_path_matcher::ComplexWildcard::static_str(#static_part), }));
                    }
                    quote! {
                        ComplexWildcard({
                            const WILDCARD: &[svelte_path_matcher::ComplexWildcard] = &[#items];
                            WILDCARD
                        })
                    }
                }
            };
            Some((
                quote! {
                    svelte_path_matcher::Item {
                        segment: svelte_path_matcher::Segment::#segment,
                        terminating: #terminating,
                        children: &[#items],
                    },
                },
                false,
            ))
        }
    }
}
