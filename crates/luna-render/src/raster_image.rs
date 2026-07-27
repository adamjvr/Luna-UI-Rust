// SPDX-License-Identifier: MPL-2.0

use luna_core::{CodedError, ErrorCode, SizeI};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

const BYTES_PER_PIXEL: usize = 4;

/// Immutable tightly packed BGRA8 image embedded in a display-list snapshot.
///
/// Text shaping adapters rasterize glyphs into this backend-neutral representation. CPU and GPU
/// renderers can then consume the same immutable command without retaining a font-system borrow or
/// exposing rasterizer-specific cache objects to widgets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RasterImage {
    size: SizeI,
    bytes: Arc<[u8]>,
}

impl RasterImage {
    /// Creates an image after validating the exact BGRA8 byte length.
    pub fn new(size: SizeI, bytes: Vec<u8>) -> Result<Self, RasterImageError> {
        let expected = expected_byte_count(size)?;
        if bytes.len() != expected {
            return Err(RasterImageError::IncorrectByteLength {
                expected,
                actual: bytes.len(),
            });
        }
        Ok(Self {
            size,
            bytes: bytes.into(),
        })
    }

    /// Creates a transparent image.
    pub fn transparent(size: SizeI) -> Result<Self, RasterImageError> {
        let byte_count = expected_byte_count(size)?;
        Ok(Self {
            size,
            bytes: vec![0; byte_count].into(),
        })
    }

    /// Returns image dimensions.
    #[must_use]
    pub const fn size(&self) -> SizeI {
        self.size
    }

    /// Returns immutable BGRA8 bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

fn expected_byte_count(size: SizeI) -> Result<usize, RasterImageError> {
    let width = usize::try_from(size.width).map_err(|_| RasterImageError::SizeOverflow)?;
    let height = usize::try_from(size.height).map_err(|_| RasterImageError::SizeOverflow)?;
    width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL))
        .ok_or(RasterImageError::SizeOverflow)
}

/// Validation failures for immutable raster images.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RasterImageError {
    /// Dimensions overflowed a platform allocation size.
    SizeOverflow,
    /// The supplied byte vector did not contain exactly four bytes per pixel.
    IncorrectByteLength {
        /// Required number of bytes.
        expected: usize,
        /// Supplied number of bytes.
        actual: usize,
    },
}

impl Display for RasterImageError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::SizeOverflow => formatter.write_str("raster image dimensions overflowed usize"),
            Self::IncorrectByteLength { expected, actual } => write!(
                formatter,
                "raster image requires {expected} BGRA8 bytes but received {actual}"
            ),
        }
    }
}

impl Error for RasterImageError {}

impl CodedError for RasterImageError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self {
            Self::SizeOverflow => "render.image.size_overflow",
            Self::IncorrectByteLength { .. } => "render.image.incorrect_byte_length",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{RasterImage, RasterImageError};
    use luna_core::SizeI;

    #[test]
    fn exact_bgra_length_is_required() {
        let error = RasterImage::new(SizeI::new(2, 2), vec![0; 15]);
        assert_eq!(
            error,
            Err(RasterImageError::IncorrectByteLength {
                expected: 16,
                actual: 15
            })
        );
    }
}
