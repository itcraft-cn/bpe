use crate::consts::U8_DATA_MAX_SIZE;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug)]
pub struct U8Bytes {
    id: u16,
    len: usize,
    bytes: [u8; U8_DATA_MAX_SIZE],
}
impl U8Bytes {
    pub fn new(id: u16, len: usize, bytes: [u8; U8_DATA_MAX_SIZE]) -> U8Bytes {
        U8Bytes { id, len, bytes }
    }
    pub fn new_from_vec(id: u16, len: usize, vec: Vec<u8>) -> U8Bytes {
        let mut bytes = [0u8; U8_DATA_MAX_SIZE];
        let slice = vec.as_slice();
        match slice.len().cmp(&U8_DATA_MAX_SIZE) {
            Ordering::Less => copy(slice.len(), &mut bytes, slice),
            Ordering::Equal | Ordering::Greater => copy(U8_DATA_MAX_SIZE, &mut bytes, slice),
        }
        U8Bytes { id, len, bytes }
    }
    pub fn id(&self) -> u16 {
        self.id
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

fn copy(len: usize, bytes: &mut [u8; U8_DATA_MAX_SIZE], slice: &[u8]) {
    for i in 0..len {
        bytes[i] = slice[i];
    }
}
