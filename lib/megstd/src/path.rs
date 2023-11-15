// impl Path for MEG-OS
// Most of them are clones of Rust's original definition.

use crate::sys::path::MAIN_SEP_STR;
use crate::*;
use core::{
    cmp, fmt,
    hash::{Hash, Hasher},
    iter::FusedIterator,
    ops, str,
};

#[repr(transparent)]
pub struct Path {
    inner: OsStr,
}

impl Path {
    pub fn new<S: AsRef<OsStr> + ?Sized>(s: &S) -> &Path {
        unsafe { &*(s.as_ref() as *const OsStr as *const Path) }
    }

    #[inline]
    pub fn as_os_str(&self) -> &OsStr {
        &self.inner
    }

    #[inline]
    pub fn to_str(&self) -> Option<&str> {
        self.inner.to_str()
    }

    #[inline]
    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf::from(self.inner.to_os_string())
    }

    #[inline]
    pub fn is_absolute(&self) -> bool {
        todo!()
    }

    #[inline]
    pub fn is_relative(&self) -> bool {
        !self.is_absolute()
    }

    #[inline]
    pub fn has_root(&self) -> bool {
        todo!()
        // self.components().has_root()
    }

    pub fn parent(&self) -> Option<&Path> {
        todo!()
    }

    #[inline]
    pub fn ancestors(&self) -> Ancestors<'_> {
        Ancestors { next: Some(&self) }
    }

    #[inline]
    #[track_caller]
    pub fn file_name(&self) -> Option<&OsStr> {
        todo!()
    }

    // pub fn strip_prefix<P>(&self, base: P) -> Result<&Path, StripPrefixError> {
    //     todo!()
    // }

    #[inline]
    #[track_caller]
    pub fn starts_with<P: AsRef<Path>>(&self, _base: P) -> bool {
        todo!()
    }

    #[inline]
    #[track_caller]
    pub fn ends_with<P: AsRef<Path>>(&self, _child: P) -> bool {
        todo!()
    }

    #[inline]
    #[track_caller]
    pub fn file_stem(&self) -> Option<&OsStr> {
        todo!()
    }

    #[inline]
    #[track_caller]
    pub fn extension(&self) -> Option<&OsStr> {
        todo!()
    }

    #[must_use]
    pub fn join<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        let mut buf = self.to_path_buf();
        buf.push(path.as_ref());
        buf
    }

    pub fn with_file_name<S: AsRef<OsStr>>(&self, file_name: S) -> PathBuf {
        let mut buf = self.to_path_buf();
        buf.set_file_name(file_name.as_ref());
        buf
    }

    pub fn with_extension<S: AsRef<OsStr>>(&self, extension: S) -> PathBuf {
        let mut buf = self.to_path_buf();
        buf.set_extension(extension.as_ref());
        buf
    }

    pub fn components(&self) -> Components<'_> {
        Components::new(self)
    }

    // #[inline]
    // pub fn canonicalize(&self) -> io::Result<PathBuf> {
    //     // fs::canonicalize(self)
    // }
}

impl fmt::Debug for Path {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, formatter)
    }
}

impl AsRef<OsStr> for Path {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        &self.inner
    }
}

impl AsRef<Path> for Path {
    #[inline]
    fn as_ref(&self) -> &Path {
        self
    }
}

impl AsRef<Path> for OsStr {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

#[repr(transparent)]
#[derive(Clone)]
pub struct PathBuf {
    inner: OsString,
}

impl PathBuf {
    #[inline]
    pub fn new() -> PathBuf {
        Self {
            inner: OsString::new(),
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> PathBuf {
        Self {
            inner: OsString::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn as_path(&self) -> &Path {
        self
    }

    #[inline]
    pub fn push<P: AsRef<Path>>(&mut self, _path: P) {
        todo!()
    }

    #[inline]
    pub fn pop(&mut self) -> bool {
        todo!()
    }

    #[inline]
    pub fn set_file_name<S: AsRef<OsStr>>(&mut self, _file_name: S) {
        todo!()
    }

    #[inline]
    pub fn set_extension<S: AsRef<OsStr>>(&mut self, _extension: S) -> bool {
        todo!()
    }

    #[inline]
    pub fn into_os_string(self) -> OsString {
        self.inner
    }

    #[inline]
    pub fn into_boxed_path(self) -> Box<Path> {
        let rw = Box::into_raw(self.inner.into_boxed_os_str()) as *mut Path;
        unsafe { Box::from_raw(rw) }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional)
    }

    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.inner.reserve_exact(additional)
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit()
    }

    // pub fn shrink_to(&mut self, min_capacity: usize)
}

impl AsRef<OsStr> for PathBuf {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        &self.inner[..]
    }
}

impl AsRef<Path> for PathBuf {
    #[inline]
    fn as_ref(&self) -> &Path {
        self
    }
}

impl ops::Deref for PathBuf {
    type Target = Path;
    #[inline]
    fn deref(&self) -> &Path {
        Path::new(&self.inner)
    }
}

impl From<OsString> for PathBuf {
    #[inline]
    fn from(s: OsString) -> PathBuf {
        PathBuf { inner: s }
    }
}

impl From<PathBuf> for OsString {
    #[inline]
    fn from(path_buf: PathBuf) -> OsString {
        path_buf.inner
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Ancestors<'a> {
    next: Option<&'a Path>,
}

impl<'a> Iterator for Ancestors<'a> {
    type Item = &'a Path;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.next;
        self.next = next.and_then(Path::parent);
        next
    }
}

impl FusedIterator for Ancestors<'_> {}

#[allow(dead_code)]
pub struct Components<'a> {
    path: &'a [u8],
    prefix: Option<Prefix<'a>>,
    cursor: usize,
}

impl<'a> Components<'a> {
    fn new(path: &'a Path) -> Self {
        Self {
            path: path.inner.bytes(),
            prefix: None,
            cursor: 0,
        }
    }

    pub fn as_path(&self) -> &'a Path {
        todo!()
    }
}

impl<'a> Iterator for Components<'a> {
    type Item = Component<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

impl AsRef<OsStr> for Components<'_> {
    fn as_ref(&self) -> &OsStr {
        todo!()
    }
}

impl AsRef<Path> for Components<'_> {
    fn as_ref(&self) -> &Path {
        todo!()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Component<'a> {
    Prefix(PrefixComponent<'a>),
    RootDir,
    CurDir,
    ParentDir,
    Normal(&'a OsStr),
}

impl<'a> Component<'a> {
    pub fn as_os_str(self) -> &'a OsStr {
        match self {
            Component::Prefix(p) => p.as_os_str(),
            Component::RootDir => OsStr::new(MAIN_SEP_STR),
            Component::CurDir => OsStr::new("."),
            Component::ParentDir => OsStr::new(".."),
            Component::Normal(path) => path,
        }
    }
}

#[derive(Copy, Clone, Eq, Debug)]
pub struct PrefixComponent<'a> {
    /// The prefix as an unparsed `OsStr` slice.
    raw: &'a OsStr,

    /// The parsed prefix data.
    parsed: Prefix<'a>,
}

impl<'a> PrefixComponent<'a> {
    /// Returns the parsed prefix data.
    ///
    /// See [`Prefix`]'s documentation for more information on the different
    /// kinds of prefixes.
    #[inline]
    pub fn kind(&self) -> Prefix<'a> {
        self.parsed
    }

    /// Returns the raw [`OsStr`] slice for this prefix.
    #[inline]
    pub fn as_os_str(&self) -> &'a OsStr {
        self.raw
    }
}

impl<'a> cmp::PartialEq for PrefixComponent<'a> {
    #[inline]
    fn eq(&self, other: &PrefixComponent<'a>) -> bool {
        cmp::PartialEq::eq(&self.parsed, &other.parsed)
    }
}

impl<'a> cmp::PartialOrd for PrefixComponent<'a> {
    #[inline]
    fn partial_cmp(&self, other: &PrefixComponent<'a>) -> Option<cmp::Ordering> {
        cmp::PartialOrd::partial_cmp(&self.parsed, &other.parsed)
    }
}

impl cmp::Ord for PrefixComponent<'_> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        cmp::Ord::cmp(&self.parsed, &other.parsed)
    }
}

impl Hash for PrefixComponent<'_> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.parsed.hash(h);
    }
}

/// Unusable in meg-os
#[derive(Copy, Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub enum Prefix<'a> {
    /// Verbatim prefix, e.g., `\\?\cat_pics`.
    Verbatim(&'a OsStr),
    /// Verbatim prefix using Windows' _**U**niform **N**aming **C**onvention_,
    /// e.g., `\\?\UNC\server\share`.
    VerbatimUNC(&'a OsStr, &'a OsStr),
    /// Verbatim disk prefix, e.g., `\\?\C:`.
    VerbatimDisk(u8),
    /// Device namespace prefix, e.g., `\\.\COM42`.
    DeviceNS(&'a OsStr),
    /// Prefix using Windows' _**U**niform **N**aming **C**onvention_, e.g.
    /// `\\server\share`.
    UNC(&'a OsStr, &'a OsStr),
    /// Prefix `C:` for the given disk drive.
    Disk(u8),
}

impl Prefix<'_> {
    pub fn is_verbatim(&self) -> bool {
        use self::Prefix::*;
        matches!(*self, Verbatim(_) | VerbatimDisk(_) | VerbatimUNC(..))
    }

    #[allow(dead_code)]
    #[inline]
    fn is_drive(&self) -> bool {
        matches!(*self, Prefix::Disk(_))
    }

    #[allow(dead_code)]
    #[inline]
    fn has_implicit_root(&self) -> bool {
        !self.is_drive()
    }
}

#[cfg(test)]
mod tests {
    // use super::*;

    // #[test]
    // fn path_file_name() {
    //     assert_eq!(Some(OsStr::new("bin")), Path::new("/usr/bin/").file_name());
    //     assert_eq!(
    //         Some(OsStr::new("foo.txt")),
    //         Path::new("tmp/foo.txt").file_name()
    //     );
    //     assert_eq!(
    //         Some(OsStr::new("foo.txt")),
    //         Path::new("foo.txt/.").file_name()
    //     );
    //     assert_eq!(
    //         Some(OsStr::new("foo.txt")),
    //         Path::new("foo.txt/.//").file_name()
    //     );
    //     assert_eq!(None, Path::new("foo.txt/..").file_name());
    //     assert_eq!(None, Path::new("/").file_name());
    // }

    // #[test]
    // fn components() {
    //     let path = Path::new("/tmp/foo/bar.txt");
    //     let components = path.components().collect::<Vec<_>>();
    //     assert_eq!(
    //         &components,
    //         &[
    //             Component::RootDir,
    //             Component::Normal("tmp".as_ref()),
    //             Component::Normal("foo".as_ref()),
    //             Component::Normal("bar.txt".as_ref()),
    //         ]
    //     );
    // }
}
