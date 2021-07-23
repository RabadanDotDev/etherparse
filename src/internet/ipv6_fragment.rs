use super::super::*;

extern crate byteorder;
use self::byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::slice::from_raw_parts;

/// IPv6 fragment header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ipv6FragmentHeader {
    /// IP protocol number specifying the next header or transport layer protocol.
    ///
    /// See [IpNumber] or [ip_number] for a definition of the known values.
    pub next_header: u8,
    /// Offset in 8 octets
    ///
    /// Note: In the header only 13 bits are used, so the allowed range
    /// of the value is between 0 and 0x1FFF (inclusive).
    pub fragment_offset: u16,
    /// True if more fragment packets will follow. False if this is the last packet.
    pub more_fragments: bool,
    /// Identifcation value generated by the source.
    pub identification: u32
}

impl Ipv6FragmentHeader {
    /// Create a new fragmentation header with the given parameters.
    ///
    /// Note that the `fragment_offset` can only support values between 0 and 0x1fff (inclusive).
    pub fn new(next_header: u8, fragment_offset: u16, more_fragments: bool, identification: u32) -> Ipv6FragmentHeader {
        Ipv6FragmentHeader{
            next_header,
            fragment_offset,
            more_fragments,
            identification
        }
    }

    /// Read an Ipv6FragmentHeader from a slice and return the header & unused parts of the slice.
    pub fn read_from_slice(slice: &[u8]) -> Result<(Ipv6FragmentHeader, &[u8]), ReadError> {
        let s = Ipv6FragmentHeaderSlice::from_slice(slice)?;
        let rest = &slice[8..];
        let header = s.to_header();
        Ok((
            header, 
            rest
        ))
    }

    /// Read an fragment header from the current reader position.
    pub fn read<T: io::Read + io::Seek + Sized>(reader: &mut T) -> Result<Ipv6FragmentHeader, ReadError> {
        let next_header = reader.read_u8()?;
        // reserved can be skipped
        reader.read_u8()?;

        let (fragment_offset, more_fragments) = {
            let mut buf: [u8;2] = [0;2];
            reader.read_exact(&mut buf[..])?;
            (
                // fragment offset
                u16::from_be_bytes(
                    [
                        (buf[0] >> 3) & 0b0001_1111u8,
                        ((buf[0] << 5) & 0b1110_0000u8) |
                        (buf[1] & 0b0001_1111u8)
                    ]
                ),
                // more fragments 
                0 != buf[1] & 0b1000_0000u8
            )
        };
        Ok(Ipv6FragmentHeader {
            next_header,
            fragment_offset,
            more_fragments,
            identification: reader.read_u32::<BigEndian>()?
        })
    }

    /// Writes a given IPv6 fragment header to the current position.
    pub fn write<T: io::Write + Sized>(&self, writer: &mut T) -> Result<(), WriteError> {
        use ErrorField::*;

        max_check_u16(
            self.fragment_offset,
            0b0001_1111_1111_1111u16,
            Ipv6FragmentOffset
        )?;

        writer.write_u8(self.next_header)?;
        writer.write_u8(0)?;
        // offset (13bit big endian) & more fragments
        {
            let buf: [u8;2] = self.fragment_offset.to_be_bytes();
            
            writer.write_u8(
                ((buf[0] << 3) & 0b1111_1000u8) |
                ((buf[1] >> 5) & 0b0000_0111u8)
            )?;

            writer.write_u8(
                (buf[1] & 0b0001_1111u8) |
                if self.more_fragments {
                    0b1000_0000u8
                } else {
                    0
                }
            )?;
            
        }
        writer.write_u32::<BigEndian>(self.identification)?;
        Ok(())
    }

    /// Length of the header in bytes.
    #[inline]
    pub fn header_len(&self) -> usize {
        8
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
    ///     let header = Ipv6FragmentHeader::new(UDP, 0, false, 123);
    ///     assert!(false == header.is_fragmenting_payload());
    /// }
    ///
    /// // offset 0 & but more fragments will come -> fragmented
    /// {
    ///     let header = Ipv6FragmentHeader::new(UDP, 0, true, 123);
    ///     assert!(header.is_fragmenting_payload());
    /// }
    ///
    /// // offset non zero & no more fragments will come -> fragmented
    /// {
    ///     let header = Ipv6FragmentHeader::new(UDP, 1, false, 123);
    ///     assert!(header.is_fragmenting_payload());
    /// }
    /// ```
    #[inline]
    pub fn is_fragmenting_payload(&self) -> bool {
        self.more_fragments ||
        (0 != self.fragment_offset)
    }
}

/// Slice containing an IPv6 fragment header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ipv6FragmentHeaderSlice<'a> {
    /// Slice containing the packet data.
    slice: &'a [u8]
}

impl<'a> Ipv6FragmentHeaderSlice<'a> {

    /// Creates a hop by hop header slice from a slice.
    pub fn from_slice(slice: &'a[u8]) -> Result<Ipv6FragmentHeaderSlice<'a>, ReadError> {
        // the fragmentation header has the exact size of 8 bytes
        use crate::ReadError::*;
        if slice.len() < 8 {
            Err(UnexpectedEndOfSlice(8))
        } else {
            Ok(Ipv6FragmentHeaderSlice {
                // SAFETY:
                // Safe as slice length is checked to be at least 8 before this
                // code can be reached.
                slice: unsafe {
                    from_raw_parts(
                        slice.as_ptr(),
                        8
                    )
                }
            })
        }
    }

    /// Creates a hop by hop header slice from a slice (assumes slice size & content was validated before).
    ///
    /// # Safety
    ///
    /// This function assumes that the passed slice has at least the length
    /// of 8. If a slice with length less then 8 is passed to this function
    /// the behavior will be undefined.
    pub unsafe fn from_slice_unchecked(slice: &'a[u8]) -> Ipv6FragmentHeaderSlice<'a> {
        // the fragmentation header has the exact size of 8 bytes
        Ipv6FragmentHeaderSlice {
            slice: from_raw_parts(
                slice.as_ptr(),
                8
            )
        }
    }

    /// Returns the slice containing the ipv6 fragment header.
    #[inline]
    pub fn slice(&self) -> &'a[u8] {
        self.slice
    }

    /// Returns the IP protocol number of the next header.
    ///
    /// See [IpNumber] or [ip_number] for a definition of the known values.
    #[inline]
    pub fn next_header(&self) -> u8 {
        // SAFETY:
        // Slice size checked to be at least 8 bytes in constructor.
        unsafe {
            *self.slice.get_unchecked(0)
        }
    }

    /// Fragment offset in 8 octets.
    ///
    /// Note: In the header only 13 bits are used, so the allowed range
    /// of the value is between 0 and 0x1FFF (inclusive).
    #[inline]
    pub fn fragment_offset(&self) -> u16 {
        u16::from_be_bytes(
            // SAFETY:
            // Slice size checked to be at least 8 bytes in constructor.
            unsafe {
                [
                    (*self.slice.get_unchecked(2) >> 3) & 0b0001_1111u8,
                    ((*self.slice.get_unchecked(2) << 5) & 0b1110_0000u8) |
                    (*self.slice.get_unchecked(3) & 0b0001_1111u8)
                ]
            }
        )
    }

    /// True if more fragment packets will follow. False if this is the last packet.
    #[inline]
    pub fn more_fragments(&self) -> bool {
        // SAFETY:
        // Slice size checked to be at least 8 bytes in constructor.
        unsafe {
            0 != *self.slice.get_unchecked(3) & 0b1000_0000u8
        }
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
    /// use etherparse::Ipv6FragmentHeaderSlice;
    ///
    /// {
    ///     let slice = Ipv6FragmentHeaderSlice::from_slice(&[
    ///         0, 0, 0, 0, // offset 0 & more_fragments not set
    ///         1, 2, 3, 4,
    ///     ]).unwrap();
    ///     assert!(false == slice.is_fragmenting_payload());
    /// }
    ///
    /// {
    ///     let slice = Ipv6FragmentHeaderSlice::from_slice(&[
    ///         0, 0, 0, 0b1000_0000u8, // more_fragments set
    ///         1, 2, 3, 4,
    ///     ]).unwrap();
    ///     assert!(slice.is_fragmenting_payload());
    /// }
    ///
    /// {
    ///     let slice = Ipv6FragmentHeaderSlice::from_slice(&[
    ///         0, 0, 1, 0, // non zero offset
    ///         1, 2, 3, 4,
    ///     ]).unwrap();
    ///     assert!(slice.is_fragmenting_payload());
    /// }
    /// ```
    #[inline]
    pub fn is_fragmenting_payload(&self) -> bool {
        // SAFETY:
        // Slice size checked to be at least 8 bytes in constructor.
        unsafe {
            0 != *self.slice.get_unchecked(2) ||
            0 != (*self.slice.get_unchecked(3) & 0b1001_1111u8) // exclude the reserved bytes
        }
    }

    /// Identifcation value generated by the source 
    pub fn identification(&self) -> u32 {
        // SAFETY:
        // Slice size checked to be at least 8 bytes in constructor.
        unsafe {
            get_unchecked_be_u32(self.slice.as_ptr().add(4))
        }
    }

    /// Decode some of the fields and copy the results to a 
    /// Ipv6FragmentHeader struct.
    pub fn to_header(&self) -> Ipv6FragmentHeader {
        Ipv6FragmentHeader{
            next_header: self.next_header(),
            fragment_offset: self.fragment_offset(),
            more_fragments: self.more_fragments(),
            identification: self.identification()
        }
    }
}
