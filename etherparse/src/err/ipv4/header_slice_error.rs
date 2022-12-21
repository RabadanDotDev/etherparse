use crate::err::UnexpectedEndOfSliceError;
use super::HeaderError;

/// Error when decoding the IPv4 part of a message.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum HeaderSliceError {
    /// Error when an unexpected end of a slice is reached even though more data was expected to be present.
    UnexpectedEndOfSlice(UnexpectedEndOfSliceError),

    /// Error caused by the contents of the header.
    Content(HeaderError),
}

impl core::fmt::Display for HeaderSliceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use HeaderSliceError::*;
        match self {
            UnexpectedEndOfSlice(err) => write!(f, "IPv4 Header: Length of the slice ({} bytes/octets) is too small to contain an IPv4 header. The slice must at least contain {} bytes/octets.", err.actual, err.expected_min),
            Content(value) => value.fmt(f),
        }
    }
}

impl std::error::Error for HeaderSliceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HeaderSliceError::UnexpectedEndOfSlice(_) => None,
            HeaderSliceError::Content(err) => Some(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, hash::{Hash, Hasher}, collections::hash_map::DefaultHasher};
    use crate::err::Layer;
    use super::{*, HeaderSliceError::*};
    
    #[test]
    fn debug() {
        let err = HeaderError::UnexpectedVersion { version_number: 6 };
        assert_eq!(
            format!("Content({:?})", err.clone()),
            format!("{:?}", Content(err))
        );
    }

    #[test]
    fn clone_eq_hash() {
        let err = Content(HeaderError::UnexpectedVersion { version_number: 6 });
        assert_eq!(err, err.clone());
        let hash_a = {
            let mut hasher = DefaultHasher::new();
            err.hash(&mut hasher);
            hasher.finish()
        };
        let hash_b = {
            let mut hasher = DefaultHasher::new();
            err.clone().hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(hash_a, hash_b);
    }

    #[test]
    fn fmt() {
        assert_eq!(
            "IPv4 Header: Length of the slice (1 bytes/octets) is too small to contain an IPv4 header. The slice must at least contain 2 bytes/octets.",
            format!(
                "{}",
                UnexpectedEndOfSlice(
                    UnexpectedEndOfSliceError{ expected_min: 2, actual: 1, layer: Layer::Ipv4Header }
                )
            )
        );
        {
            let err = HeaderError::UnexpectedVersion { version_number: 6 };
            assert_eq!(
                format!("{}", &err),
                format!("{}", Content(err.clone()))
            );
        }
    }

    #[test]
    fn source() {
        assert!(UnexpectedEndOfSlice(
            UnexpectedEndOfSliceError{ expected_min: 0, actual: 0, layer: Layer::Ipv4Header }
        ).source().is_none());
        assert!(Content(HeaderError::UnexpectedVersion { version_number: 6 }).source().is_some());
    }
}
