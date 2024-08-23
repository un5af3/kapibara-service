use std::io::{self, Read};

use bytes::BufMut;

const MAX_VARINT_LEN64: u32 = 10;

pub fn read_varint<R: Read>(reader: &mut R) -> io::Result<u64> {
    let mut x = 0u64;
    let mut s = 0u32;

    for i in 0..MAX_VARINT_LEN64 {
        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf)?;
        let b = buf[0];
        if b < 0x80 {
            if i == MAX_VARINT_LEN64 - 1 && b > 1 {
                break;
            }
            return Ok(x | ((b as u64) << s));
        }
        x |= ((b & 0x7f) as u64) << s;
        s += 7;
    }

    Err(io::Error::new(io::ErrorKind::InvalidData, "overflow"))
}

pub fn write_varint<B: BufMut>(buf: &mut B, mut x: u64) {
    while x >= 0x80 {
        buf.put_u8((x as u8) | 0x80);
        x >>= 7;
    }
    buf.put_u8(x as u8);
}

pub fn variant_len(x: u64) -> usize {
    if x < 1 << (7 * 1) {
        1
    } else if x < 1 << (7 * 2) {
        2
    } else if x < 1 << (7 * 2) {
        2
    } else if x < 1 << (7 * 3) {
        3
    } else if x < 1 << (7 * 4) {
        4
    } else if x < 1 << (7 * 5) {
        5
    } else if x < 1 << (7 * 6) {
        6
    } else if x < 1 << (7 * 7) {
        7
    } else if x < 1 << (7 * 8) {
        8
    } else if x < 1 << (7 * 9) {
        9
    } else {
        10
    }
}
