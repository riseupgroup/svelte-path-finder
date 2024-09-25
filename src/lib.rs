use std::{
    borrow::Cow,
    fmt::{self, Display},
};

pub use macros::build_from_filesystem;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ComplexWildcard<'a> {
    Static(&'a str),
    Wildcard,
}

impl<'a> ComplexWildcard<'a> {
    pub fn matches(items: &[Self], mut segment: &str) -> bool {
        if items.is_empty() {
            return segment.is_empty();
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
                Self::Static(x) => x.fmt(f)?,
                Self::Wildcard => write!(f, "*")?,
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Segment<'a> {
    Static(&'a str),
    Wildcard,
    OptionalWildcard,
    RepeatedWildcard,
    ComplexWildcard(&'a [ComplexWildcard<'a>]),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item<'a> {
    pub segment: Segment<'a>,
    pub children: &'a [Item<'a>],
    pub terminating: bool,
    pub requires_login: bool,
}

impl Item<'_> {
    /// # Returns
    ///
    /// `requires_login` if the path was found
    pub fn find(&self, path: &str) -> Option<bool> {
        let (segment, remaining) = match path.as_bytes().iter().position(|c| *c == b'/') {
            Some(pos) => (&path[..pos], &path[pos + 1..]),
            None => (path, ""),
        };

        let matches = match &self.segment {
            Segment::Static(_) | Segment::ComplexWildcard(_) => {
                let decoded = urlencoding::decode_binary(segment.as_bytes());
                let decoded = match &decoded {
                    Cow::Borrowed(_) => segment,
                    Cow::Owned(segment) => unsafe { std::str::from_utf8_unchecked(segment) },
                };
                match &self.segment {
                    Segment::Static(x) => **x == *decoded,
                    Segment::ComplexWildcard(items) => ComplexWildcard::matches(items, decoded),
                    _ => unreachable!(),
                }
            }
            Segment::Wildcard => true,
            Segment::OptionalWildcard => {
                if let Some(login) = self.find_child(path) {
                    return Some(login);
                }
                true
            }
            Segment::RepeatedWildcard => {
                let mut res = self.find_child(path);
                if res.is_none() && !remaining.is_empty() {
                    res = self.find(remaining);
                }
                if res.is_some() {
                    return res;
                }
                true
            }
        };

        match matches {
            true => {
                if remaining.is_empty() && self.terminating {
                    return Some(self.requires_login);
                }

                self.find_child(remaining)
            }
            false => None,
        }
    }

    fn find_child(&self, path: &str) -> Option<bool> {
        for item in self.children {
            if let Some(login) = item.find(path) {
                return Some(login);
            }
        }
        None
    }

    pub fn fmt_children(
        items: &[Self],
        terminating: bool,
        requires_login: bool,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if !items.is_empty() {
            if terminating {
                write!(f, " (terminating")?;
                if requires_login {
                    write!(f, ", login")?;
                }
                write!(f, ")")?;
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
        } else if requires_login {
            write!(f, " (login)")?;
        }
        Ok(())
    }
}

impl Display for Item<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.segment {
            Segment::Static(x) => {
                write!(f, "/")?;
                x.fmt(f)
            }
            Segment::Wildcard => write!(f, "/*"),
            Segment::OptionalWildcard => write!(f, "(/*)"),
            Segment::RepeatedWildcard => write!(f, "/**"),
            Segment::ComplexWildcard(x) => {
                write!(f, "/")?;
                ComplexWildcard::fmt(x, f)
            }
        }?;

        Self::fmt_children(self.children, self.terminating, self.requires_login, f)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SveltePathFinder<'a> {
    pub children: &'a [Item<'a>],
    pub terminating: bool,
    pub requires_login: bool,
}

impl SveltePathFinder<'_> {
    /// # Returns
    ///
    /// `requires_login` if the path was found
    pub fn find(&self, mut path: &str) -> Option<bool> {
        if !path.is_empty() && path.as_bytes()[0] == b'/' {
            path = &path[1..];
        }
        if self.terminating && path.is_empty() {
            return Some(self.requires_login);
        }
        for item in self.children {
            if let Some(login) = item.find(path) {
                return Some(login);
            }
        }
        None
    }
}

impl Display for SveltePathFinder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/")?;
        Item::fmt_children(self.children, self.terminating, self.requires_login, f)
    }
}
