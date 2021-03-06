use std::fmt;

use lazy_static::lazy_static;
use regex::Regex;

#[cfg(feature = "serde")]
use serde::{
    ser::{SerializeStruct, Serializer},
    Serialize,
};

lazy_static! {
    static ref RELEASE_REGEX: Regex = Regex::new(r#"^(@?[^@]+)@(.*?)$"#).unwrap();
    static ref VERSION_REGEX: Regex = Regex::new(
        r#"(?x)
        ^
            (?P<major>0|[1-9][0-9]*)
            (?:\.(?P<minor>0|[1-9][0-9]*))?
            (?:\.(?P<patch>0|[1-9][0-9]*))?
            (?:
                (?P<prerelease>
                    (?:-|[a-z])
                    (?:0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*)?
                    (?:\.(?:0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*))*)
                )?
            (?:\+(?P<build_code>[0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?
        $
    "#
    )
    .unwrap();
    static ref HEX_REGEX: Regex = Regex::new(r#"^[a-fA-F0-9]+$"#).unwrap();
    static ref VALID_RELEASE_REGEX: Regex = Regex::new(r"^[^/\r\n]*\z").unwrap();
}

/// An error indicating invalid versions.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct InvalidVersion;

impl std::error::Error for InvalidVersion {}

impl fmt::Display for InvalidVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid version")
    }
}

/// An error indicating invalid releases.
#[derive(Debug, Clone, PartialEq)]
pub enum InvalidRelease {
    /// The release name was too long
    TooLong,
    /// Release name is restricted
    RestrictedName,
    /// The release contained invalid characters
    BadCharacters,
}

impl std::error::Error for InvalidRelease {}

impl fmt::Display for InvalidRelease {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "invalid release: {}",
            match *self {
                InvalidRelease::BadCharacters => "bad characters in release name",
                InvalidRelease::RestrictedName => "restricted release name",
                InvalidRelease::TooLong => "release name too long",
            }
        )
    }
}

/// Represents a parsed version.
#[derive(Debug, Clone, PartialEq)]
pub struct Version<'a> {
    raw: &'a str,
    major: u64,
    minor: u64,
    patch: u64,
    pre: &'a str,
    build_code: &'a str,
    components: u8,
}

#[cfg(feature = "serde")]
impl<'a> Serialize for Version<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Version", 5)?;
        state.serialize_field("major", &self.major())?;
        state.serialize_field("minor", &self.minor())?;
        state.serialize_field("patch", &self.patch())?;
        state.serialize_field("pre", &self.pre())?;
        state.serialize_field("build_code", &self.build_code())?;
        state.serialize_field("components", &self.components())?;
        state.end()
    }
}

fn is_build_hash(s: &str) -> bool {
    match s.len() {
        12 | 16 | 20 | 32 | 40 | 64 => HEX_REGEX.is_match(s),
        _ => false,
    }
}

impl<'a> Version<'a> {
    /// Parses a version from a string.
    pub fn parse(version: &'a str) -> Result<Version<'a>, InvalidVersion> {
        let caps = if let Some(caps) = VERSION_REGEX.captures(version) {
            caps
        } else {
            return Err(InvalidVersion);
        };

        let components = 1 + caps.get(2).map_or(0, |_| 1) + caps.get(3).map_or(0, |_| 1);
        Ok(Version {
            raw: version,
            major: caps[1].parse().unwrap_or(0),
            minor: caps
                .get(2)
                .and_then(|x| x.as_str().parse().ok())
                .unwrap_or(0),
            patch: caps
                .get(3)
                .and_then(|x| x.as_str().parse().ok())
                .unwrap_or(0),
            pre: caps
                .get(4)
                .map(|x| {
                    let mut pre = x.as_str();
                    if pre.starts_with('-') {
                        pre = &pre[1..]
                    }
                    pre
                })
                .unwrap_or(""),
            build_code: caps.get(5).map(|x| x.as_str()).unwrap_or(""),
            components,
        })
    }

    /// Converts the version into a semver.
    ///
    /// Requires the `semver` feature.
    #[cfg(feature = "semver")]
    pub fn as_semver(&self) -> semver::Version {
        fn split(s: &str) -> Vec<semver::Identifier> {
            s.split('.')
                .map(|item| {
                    if let Ok(val) = item.parse::<u64>() {
                        semver::Identifier::Numeric(val)
                    } else {
                        semver::Identifier::AlphaNumeric(item.into())
                    }
                })
                .collect()
        }

        semver::Version {
            major: self.major,
            minor: self.minor,
            patch: self.patch,
            pre: split(self.pre),
            build: split(self.build_code),
        }
    }

    /// Returns the major version component.
    pub fn major(&self) -> u64 {
        self.major
    }

    /// Returns the minor version component.
    pub fn minor(&self) -> u64 {
        self.minor
    }

    /// Returns the patch level version component.
    pub fn patch(&self) -> u64 {
        self.patch
    }

    /// If a pre-release identifier is included returns that.
    pub fn pre(&self) -> Option<&str> {
        if self.pre.is_empty() {
            None
        } else {
            Some(self.pre)
        }
    }

    /// If a build code is included returns that.
    pub fn build_code(&self) -> Option<&str> {
        if self.build_code.is_empty() {
            None
        } else {
            Some(self.build_code)
        }
    }

    /// Returns the number of components.
    pub fn components(&self) -> u8 {
        self.components
    }

    /// Returns the raw version as string.
    ///
    /// It's generally better to use `to_string` which normalizes.
    pub fn raw(&self) -> &str {
        self.raw
    }

    /// Returns the version triple (major, minor, patch)
    pub fn triple(&self) -> (u64, u64, u64) {
        (self.major, self.minor, self.patch)
    }

    /// Returns the version triple with an added pre-release marker.
    pub fn quad(&self) -> (u64, u64, u64, Option<&str>) {
        (self.major, self.minor, self.patch, self.pre())
    }
}

impl<'a> fmt::Display for Version<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&VersionDescription(self), f)?;
        if let Some(pre) = self.pre() {
            write!(f, "-{}", pre)?;
        }
        if let Some(build_code) = self.build_code() {
            write!(f, "+{}", build_code)?;
        }
        Ok(())
    }
}

/// Represents a parsed release.
#[derive(Debug, Clone, PartialEq)]
pub struct Release<'a> {
    raw: &'a str,
    package: &'a str,
    version_raw: &'a str,
    version: Option<Version<'a>>,
}

#[cfg(feature = "serde")]
impl<'a> Serialize for Release<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Release", 6)?;
        state.serialize_field("package", &self.package())?;
        state.serialize_field("version_raw", &self.version_raw())?;
        state.serialize_field("version_parsed", &self.version())?;
        state.serialize_field("build_hash", &self.build_hash())?;
        state.serialize_field("description", &self.describe().to_string())?;
        state.end()
    }
}

impl<'a> Release<'a> {
    /// Parses a release from a string.
    pub fn parse(release: &'a str) -> Result<Release<'a>, InvalidRelease> {
        let release = release.trim();
        if release.len() > 250 {
            return Err(InvalidRelease::TooLong);
        } else if release == "." || release == ".." || release == "latest" {
            return Err(InvalidRelease::RestrictedName);
        } else if !VALID_RELEASE_REGEX.is_match(release) {
            return Err(InvalidRelease::BadCharacters);
        }
        if let Some(caps) = RELEASE_REGEX.captures(release) {
            let package = caps.get(1).unwrap().as_str();
            let version_raw = caps.get(2).unwrap().as_str();
            if !is_build_hash(version_raw) {
                let version = Version::parse(version_raw).ok();
                return Ok(Release {
                    raw: release,
                    package,
                    version_raw,
                    version,
                });
            } else {
                return Ok(Release {
                    raw: release,
                    package,
                    version_raw,
                    version: None,
                });
            }
        }
        Ok(Release {
            raw: release,
            package: "",
            version_raw: release,
            version: None,
        })
    }

    /// Returns the raw version.
    ///
    /// It's generally better to use `to_string` which normalizes.
    pub fn raw(&self) -> &str {
        self.raw
    }

    /// Returns the contained package information.
    pub fn package(&self) -> Option<&str> {
        if self.package.is_empty() {
            None
        } else {
            Some(self.package)
        }
    }

    /// The raw version part of the release.
    ///
    /// This is set even if the version part is not a valid version
    /// (for instance because it's a hash).
    pub fn version_raw(&self) -> &str {
        self.version_raw
    }

    /// If a parsed version if available returns it.
    pub fn version(&self) -> Option<&Version<'a>> {
        self.version.as_ref()
    }

    /// Returns the build hash if available.
    pub fn build_hash(&self) -> Option<&str> {
        self.version
            .as_ref()
            .and_then(|x| x.build_code())
            .filter(|x| is_build_hash(x))
            .or_else(|| {
                if is_build_hash(self.version_raw()) {
                    Some(self.version_raw())
                } else {
                    None
                }
            })
    }

    /// Returns a short description.
    ///
    /// This returns a human readable format that includes an abbreviated
    /// name of the release.  Typically it will remove the package and it
    /// will try to abbreviate build hashes etc.
    pub fn describe(&self) -> ReleaseDescription<'_> {
        ReleaseDescription(self)
    }
}

#[derive(Debug)]
struct VersionDescription<'a>(&'a Version<'a>);

impl<'a> fmt::Display for VersionDescription<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0.components() {
            3 => {
                write!(
                    f,
                    "{}.{}.{}",
                    self.0.major(),
                    self.0.minor(),
                    self.0.patch()
                )?;
            }
            2 => {
                write!(f, "{}.{}", self.0.major(), self.0.minor())?;
            }
            1 => {
                write!(f, "{}", self.0.major())?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}

/// Helper object to format a release into a description.
#[derive(Debug)]
pub struct ReleaseDescription<'a>(&'a Release<'a>);

impl<'a> fmt::Display for ReleaseDescription<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let short_hash = if let Some(hash) = self.0.build_hash() {
            Some(hash.get(..12).unwrap_or(hash))
        } else {
            None
        };

        if let Some(ver) = self.0.version() {
            fmt::Display::fmt(&VersionDescription(ver), f)?;
            if let Some(pre) = ver.pre() {
                write!(f, "-{}", pre)?;
            }
            if let Some(short_hash) = short_hash {
                write!(f, " ({})", short_hash)?;
            } else if let Some(build_code) = ver.build_code() {
                write!(f, " ({})", build_code)?;
            }
        } else if let Some(short_hash) = short_hash {
            write!(f, "{}", short_hash)?;
        } else {
            write!(f, "{}", self.0)?;
        }
        Ok(())
    }
}

impl<'a> fmt::Display for Release<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut have_package = false;
        if let Some(package) = self.package() {
            write!(f, "{}", package)?;
            have_package = true;
        }
        if let Some(version) = self.version() {
            if have_package {
                write!(f, "@")?;
            }
            write!(f, "{}", version)?;
        } else {
            if have_package {
                write!(f, "@")?;
            }
            write!(f, "{}", self.version_raw)?;
        }
        Ok(())
    }
}
