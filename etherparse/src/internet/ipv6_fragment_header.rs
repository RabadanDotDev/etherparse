use super::super::*;

/// IPv6 fragment header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ipv6FragmentHeader {
    /// IP protocol number specifying the next header or transport layer protocol.
    ///
    /// See [IpNumber] or [ip_number] for a definition of the known values.
    pub next_header: IpNumber,
    /// Offset of the current IP payload relative to the start of the fragmented
    /// packet payload.
    pub fragment_offset: IpFragOffset,
    /// True if more fragment packets will follow. False if this is the last packet.
    pub more_fragments: bool,
    /// Identifcation value generated by the source.
    pub identification: u32,
}

impl Ipv6FragmentHeader {
    /// Length of the serialized header.
    pub const LEN: usize = 8;

    /// Create a new fragmentation header with the given parameters.
    ///
    /// Note that the `fragment_offset` can only support values between 0 and 0x1fff (inclusive).
    pub const fn new(
        next_header: IpNumber,
        fragment_offset: IpFragOffset,
        more_fragments: bool,
        identification: u32,
    ) -> Ipv6FragmentHeader {
        Ipv6FragmentHeader {
            next_header,
            fragment_offset,
            more_fragments,
            identification,
        }
    }

    /// Read an Ipv6FragmentHeader from a slice and return the header & unused parts of the slice.
    pub fn from_slice(slice: &[u8]) -> Result<(Ipv6FragmentHeader, &[u8]), err::LenError> {
        let s = Ipv6FragmentHeaderSlice::from_slice(slice)?;
        let rest = &slice[8..];
        let header = s.to_header();
        Ok((header, rest))
    }

    /// Read an fragment header from the current reader position.
    #[cfg(feature = "std")]
    pub fn read<T: std::io::Read + std::io::Seek + Sized>(
        reader: &mut T,
    ) -> Result<Ipv6FragmentHeader, std::io::Error> {
        let buffer = {
            let mut buffer: [u8; 8] = [0; 8];
            reader.read_exact(&mut buffer)?;
            buffer
        };

        Ok(Ipv6FragmentHeader {
            next_header: IpNumber(buffer[0]),
            fragment_offset: unsafe {
                // SAFE as the resulting number is guaranteed to have at most
                // 13 bits.
                IpFragOffset::new_unchecked(u16::from_be_bytes([
                    (buffer[2] >> 3) & 0b0001_1111u8,
                    ((buffer[2] << 5) & 0b1110_0000u8) | (buffer[3] & 0b0001_1111u8),
                ]))
            },
            more_fragments: 0 != buffer[3] & 0b1000_0000u8,
            identification: u32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]),
        })
    }

    /// Read an fragment header from the current reader position.
    #[cfg(feature = "std")]
    pub fn read_limited<T: std::io::Read + std::io::Seek + Sized>(
        reader: &mut crate::io::LimitedReader<T>,
    ) -> Result<Ipv6FragmentHeader, crate::err::io::LimitedReadError> {
        use err::Layer;

        // set layer so errors contain the correct layer & offset
        reader.start_layer(Layer::Ipv6FragHeader);

        let buffer = {
            let mut buffer: [u8; 8] = [0; 8];
            reader.read_exact(&mut buffer)?;
            buffer
        };

        Ok(Ipv6FragmentHeader {
            next_header: IpNumber(buffer[0]),
            fragment_offset: unsafe {
                // SAFE as the resulting number is guaranteed to have at most
                // 13 bits.
                IpFragOffset::new_unchecked(u16::from_be_bytes([
                    (buffer[2] >> 3) & 0b0001_1111u8,
                    ((buffer[2] << 5) & 0b1110_0000u8) | (buffer[3] & 0b0001_1111u8),
                ]))
            },
            more_fragments: 0 != buffer[3] & 0b1000_0000u8,
            identification: u32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]),
        })
    }

    /// Writes a given IPv6 fragment header to the current position.
    #[cfg(feature = "std")]
    pub fn write<T: std::io::Write + Sized>(&self, writer: &mut T) -> Result<(), std::io::Error> {
        writer.write_all(&self.to_bytes())
    }

    /// Length of the header in bytes.
    #[inline]
    pub fn header_len(&self) -> usize {
        Ipv6FragmentHeader::LEN
    }

    /// Checks if the fragment header actually fragments the packet.
    ///
    /// Returns false if the fragment offset is 0 and the more flag
    /// is not set. Otherwise returns true.
    ///
    /// [RFC8200](https://datatracker.ietf.org/doc/html/rfc8200) explicitly
    /// states that fragment headers that don't fragment the packet payload are
    /// allowed. See the following quote from
    /// RFC8200 page 32:
    ///
    /// > Revised the text to handle the case of fragments that are whole
    /// > datagrams (i.e., both the Fragment Offset field and the M flag
    /// > are zero).  If received, they should be processed as a
    /// > reassembled packet.  Any other fragments that match should be
    /// > processed independently.  The Fragment creation process was
    /// > modified to not create whole datagram fragments (Fragment
    /// > Offset field and the M flag are zero).  See
    /// > [RFC6946](https://datatracker.ietf.org/doc/html/6946) and
    /// > [RFC8021](https://datatracker.ietf.org/doc/html/rfc8021) for more
    /// > information."
    ///
    /// ```
    /// use etherparse::{Ipv6FragmentHeader, ip_number::UDP};
    ///
    /// // offset 0 & no more fragments result in an unfragmented payload
    /// {
    ///     let header = Ipv6FragmentHeader::new(UDP, 0.try_into().unwrap(), false, 123);
    ///     assert!(false == header.is_fragmenting_payload());
    /// }
    ///
    /// // offset 0 & but more fragments will come -> fragmented
    /// {
    ///     let header = Ipv6FragmentHeader::new(UDP, 0.try_into().unwrap(), true, 123);
    ///     assert!(header.is_fragmenting_payload());
    /// }
    ///
    /// // offset non zero & no more fragments will come -> fragmented
    /// {
    ///     let header = Ipv6FragmentHeader::new(UDP, 1.try_into().unwrap(), false, 123);
    ///     assert!(header.is_fragmenting_payload());
    /// }
    /// ```
    #[inline]
    pub fn is_fragmenting_payload(&self) -> bool {
        self.more_fragments || (0 != self.fragment_offset.value())
    }

    /// Returns the serialized form of the header as a statically
    /// sized byte array.
    #[inline]
    pub fn to_bytes(&self) -> [u8; 8] {
        let fo_be: [u8; 2] = self.fragment_offset.value().to_be_bytes();
        let id_be = self.identification.to_be_bytes();
        [
            self.next_header.0,
            0,
            (((fo_be[0] << 3) & 0b1111_1000u8) | ((fo_be[1] >> 5) & 0b0000_0111u8)),
            ((fo_be[1] & 0b0001_1111u8)
                | if self.more_fragments {
                    0b1000_0000u8
                } else {
                    0
                }),
            id_be[0],
            id_be[1],
            id_be[2],
            id_be[3],
        ]
    }
}

#[cfg(test)]
mod test {
    use crate::{test_gens::*, *};
    use alloc::{format, vec::Vec};
    use proptest::prelude::*;
    use std::io::Cursor;

    proptest! {
        #[test]
        fn debug(input in ipv6_fragment_any()) {
            assert_eq!(
                &format!(
                    "Ipv6FragmentHeader {{ next_header: {:?}, fragment_offset: {:?}, more_fragments: {}, identification: {} }}",
                    input.next_header,
                    input.fragment_offset,
                    input.more_fragments,
                    input.identification
                ),
                &format!("{:?}", input)
            );
        }
    }

    proptest! {
        #[test]
        fn clone_eq(input in ipv6_fragment_any()) {
            assert_eq!(input, input.clone());
        }
    }

    proptest! {
        #[test]
        fn new(
            next_header in ip_number_any(),
            fragment_offset in 0..IpFragOffset::MAX_U16,
            more_fragments in any::<bool>(),
            identification in any::<u32>(),
        ) {
            let a = Ipv6FragmentHeader::new(
                next_header,
                fragment_offset.try_into().unwrap(),
                more_fragments,
                identification
            );
            assert_eq!(next_header, a.next_header);
            assert_eq!(fragment_offset, a.fragment_offset.value());
            assert_eq!(more_fragments, a.more_fragments);
            assert_eq!(identification, a.identification);
        }
    }

    proptest! {
        #[test]
        fn from_slice(
            input in ipv6_fragment_any(),
            dummy_data in proptest::collection::vec(any::<u8>(), 0..20)
        ) {
            // serialize
            let mut buffer: Vec<u8> = Vec::with_capacity(8 + dummy_data.len());
            input.write(&mut buffer).unwrap();
            buffer.extend(&dummy_data[..]);

            // calls with a valid result
            {
                let (result, rest) = Ipv6FragmentHeader::from_slice(&buffer[..]).unwrap();
                assert_eq!(input, result);
                assert_eq!(&buffer[8..], rest);
            }
            // call with not enough data in the slice
            for len in 0..Ipv6FragmentHeader::LEN {
                assert_eq!(
                    Ipv6FragmentHeader::from_slice(&buffer[0..len]).unwrap_err(),
                    err::LenError{
                        required_len: Ipv6FragmentHeader::LEN,
                        len: len,
                        len_source: err::LenSource::Slice,
                        layer: err::Layer::Ipv6FragHeader,
                        layer_start_offset: 0,
                    }
                );
            }
        }
    }

    proptest! {
        #[test]
        fn read(
            input in ipv6_fragment_any(),
            dummy_data in proptest::collection::vec(any::<u8>(), 0..20)
        ) {
            use std::io::ErrorKind;

            // serialize
            let mut buffer: Vec<u8> = Vec::with_capacity(8 + dummy_data.len());
            input.write(&mut buffer).unwrap();
            buffer.extend(&dummy_data[..]);

            // calls with a valid result
            {
                let mut cursor = Cursor::new(&buffer);
                let result = Ipv6FragmentHeader::read(&mut cursor).unwrap();
                assert_eq!(input, result);
                assert_eq!(cursor.position(), 8);
            }

            // call with not enough data in the slice
            for len in 0..Ipv6FragmentHeader::LEN {
                let mut cursor = Cursor::new(&buffer[0..len]);
                assert_eq!(
                    Ipv6FragmentHeader::read(&mut cursor)
                    .unwrap_err()
                    .kind(),
                    ErrorKind::UnexpectedEof
                );
            }
        }
    }

    proptest! {
        #[test]
        fn write(input in ipv6_fragment_any()) {

            // normal write
            {
                let mut buffer = Vec::with_capacity(8);
                input.write(&mut buffer).unwrap();
                assert_eq!(
                    &buffer,
                    &input.to_bytes()
                );
            }

            // not enough memory for write
            for len in 0..Ipv6FragmentHeader::LEN {
                let mut buffer = [0u8;Ipv6FragmentHeader::LEN];
                let mut cursor = Cursor::new(&mut buffer[..len]);
                assert!(
                    input.write(&mut cursor).is_err()
                );
            }
        }
    }

    proptest! {
        #[test]
        fn header_len(input in ipv6_fragment_any()) {
            assert_eq!(8, input.header_len());
        }
    }

    proptest! {
        #[test]
        fn is_fragmenting_payload(
            non_zero_offset in 1u16..0b0001_1111_1111_1111u16,
            identification in any::<u32>(),
            next_header in ip_number_any(),

        ) {
            // negative case
            {
                let header = Ipv6FragmentHeader {
                    next_header,
                    fragment_offset: 0.try_into().unwrap(),
                    more_fragments: false,
                    identification
                };
                assert!(false == header.is_fragmenting_payload());
            }
            // positive case (non zero offset)
            {
                let header = Ipv6FragmentHeader {
                    next_header,
                    fragment_offset: non_zero_offset.try_into().unwrap(),
                    more_fragments: false,
                    identification
                };
                assert!(header.is_fragmenting_payload());
            }

            // positive case (more fragments)
            {
                let header = Ipv6FragmentHeader {
                    next_header,
                    fragment_offset: 0.try_into().unwrap(),
                    more_fragments: true,
                    identification
                };
                assert!(header.is_fragmenting_payload());
            }

            // positive case (non zero offset & more fragments)
            {
                let header = Ipv6FragmentHeader {
                    next_header,
                    fragment_offset: non_zero_offset.try_into().unwrap(),
                    more_fragments: true,
                    identification
                };
                assert!(header.is_fragmenting_payload());
            }
        }
    }

    proptest! {
        #[test]
        fn to_bytes(input in ipv6_fragment_any()) {

            // normal write
            {
                let fragment_offset_be = input.fragment_offset.value().to_be_bytes();
                let id_be = input.identification.to_be_bytes();
                assert_eq!(
                    &input.to_bytes(),
                    &[
                        input.next_header.0,
                        0,
                        (
                            (fragment_offset_be[0] << 3 & 0b1111_1000u8) |
                            (fragment_offset_be[1] >> 5 & 0b0000_0111u8)
                        ),
                        (
                            (fragment_offset_be[1] & 0b0001_1111u8) |
                            if input.more_fragments {
                                0b1000_0000u8
                            } else {
                                0u8
                            }
                        ),
                        id_be[0],
                        id_be[1],
                        id_be[2],
                        id_be[3],
                    ]
                );
            }
        }
    }
}
