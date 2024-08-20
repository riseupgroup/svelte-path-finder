use std::fmt::{self, Display};

pub use macros::build_from_filesystem;

#[derive(Debug)]
pub enum ComplexWildcard<'a> {
    Static(&'a [u8]),
    Wildcard,
}

impl<'a> ComplexWildcard<'a> {
    pub const fn static_str(str: &'a str) -> Self {
        Self::Static(str.as_bytes())
    }

    pub fn matches(items: &[Self], mut segment: &[u8]) -> bool {
        if items.is_empty() && segment.is_empty() {
            return true;
        }
        if items.is_empty() {
            return false;
        }
        match items[0] {
            Self::Static(x) if segment.starts_with(x) => {
                Self::matches(&items[1..], &segment[x.len()..])
            }
            Self::Static(_) => false,
            Self::Wildcard => {
                if items.len() == 1 {
                    return true;
                }
                while !segment.is_empty() {
                    if Self::matches(&items[1..], segment) {
                        return true;
                    }
                    segment = &segment[1..];
                }
                Self::matches(&items[1..], segment)
            }
        }
    }

    fn fmt(items: &[Self], f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for item in items {
            match item {
                Self::Static(x) => unsafe { std::str::from_utf8_unchecked(x) }.fmt(f)?,
                Self::Wildcard => write!(f, "*")?,
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum Segment<'a> {
    Static(&'a [u8]),
    Wildcard,
    OptionalWildcard,
    RepeatedWildcard,
    ComplexWildcard(&'a [ComplexWildcard<'a>]),
}

impl<'a> Segment<'a> {
    pub const fn static_str(str: &'a str) -> Self {
        Self::Static(str.as_bytes())
    }
}

#[derive(Debug)]
pub struct Item<'a> {
    pub segment: Segment<'a>,
    pub terminating: bool,
    pub children: &'a [Item<'a>],
}

impl Item<'_> {
    pub fn matches(&self, path: &[u8]) -> bool {
        let (segment, remaining) = match path.iter().position(|c| *c == b'/') {
            Some(pos) => (&path[..pos], &path[pos + 1..]),
            None => (path, (&[] as &[u8])),
        };

        let matches = match &self.segment {
            Segment::Static(_) | Segment::ComplexWildcard(_) => {
                let segment = urlencoding::decode_binary(segment);
                match &self.segment {
                    Segment::Static(x) => **x == *segment,
                    Segment::ComplexWildcard(items) => ComplexWildcard::matches(items, &segment),
                    _ => unreachable!(),
                }
            }
            Segment::Wildcard => true,
            Segment::OptionalWildcard => {
                if self.child_matches(path) {
                    return true;
                }
                true
            }
            Segment::RepeatedWildcard => {
                if self.child_matches(path) || (!remaining.is_empty() && self.matches(remaining)) {
                    return true;
                }
                true
            }
        };
        if !matches {
            return false;
        }

        if remaining.is_empty() && self.terminating {
            return true;
        }
        self.child_matches(remaining)
    }

    fn child_matches(&self, path: &[u8]) -> bool {
        for item in self.children {
            if item.matches(path) {
                return true;
            }
        }
        false
    }

    pub fn fmt_children(
        items: &[Self],
        terminating: bool,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if !items.is_empty() {
            if terminating {
                write!(f, " (terminating)")?;
            }
            if !terminating && items.len() == 1 {
                items[0].fmt(f)?;
            } else if f.alternate() {
                writeln!(f, " {{")?;
                use fmt::Write;
                let mut writer = pad_adapter::PadAdapter::new(f);
                for item in items {
                    writeln!(writer, "{item:#},")?;
                }
                write!(f, "}}")?;
            } else {
                write!(f, " {{ ")?;
                let mut first = true;
                for item in items {
                    if !first {
                        write!(f, ", ")?;
                    }
                    item.fmt(f)?;
                    first = false;
                }
                write!(f, " }}")?;
            }
        }
        Ok(())
    }
}

impl Display for Item<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.segment {
            Segment::Static(x) => {
                write!(f, "/")?;
                unsafe { std::str::from_utf8_unchecked(x) }.fmt(f)
            }
            Segment::Wildcard => write!(f, "/*"),
            Segment::OptionalWildcard => write!(f, "(/*)"),
            Segment::RepeatedWildcard => write!(f, "/**"),
            Segment::ComplexWildcard(x) => {
                write!(f, "/")?;
                ComplexWildcard::fmt(x, f)
            }
        }?;

        Self::fmt_children(self.children, self.terminating, f)
    }
}

#[derive(Debug)]
pub struct SveltePathMatcher<'a> {
    pub children: &'a [Item<'a>],
    pub terminating: bool,
}

impl SveltePathMatcher<'_> {
    pub fn matches(&self, path: &str) -> bool {
        let mut path = path.as_bytes();
        if !path.is_empty() && path[0] == b'/' {
            path = &path[1..];
        }
        if self.terminating && path.is_empty() {
            return true;
        }
        for item in self.children {
            if item.matches(path) {
                return true;
            }
        }
        false
    }
}

impl Display for SveltePathMatcher<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/")?;
        Item::fmt_children(self.children, self.terminating, f)
    }
}
