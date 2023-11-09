use crate::consts::U8_DATA_MAX_SIZE;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug)]
pub struct U8Bytes {
    id: u16,
    data_len: usize,
    bytes: [u8; U8_DATA_MAX_SIZE],
}
impl U8Bytes {
    pub fn new(id: u16, data_len: usize, bytes: [u8; U8_DATA_MAX_SIZE]) -> U8Bytes {
        U8Bytes {
            id,
            data_len,
            bytes,
        }
    }
    pub fn new_from_vec(id: u16, data_len: usize, vec: Vec<u8>) -> U8Bytes {
        let mut bytes = [0u8; U8_DATA_MAX_SIZE];
        let slice = vec.as_slice();
        match slice.len().cmp(&U8_DATA_MAX_SIZE) {
            Ordering::Less => copy(slice.len(), &mut bytes, slice),
            Ordering::Equal | Ordering::Greater => copy(U8_DATA_MAX_SIZE, &mut bytes, slice),
        }
        U8Bytes {
            id,
            data_len,
            bytes,
        }
    }
    pub fn new_from_slice(id: u16, data_len: usize, slice: &[u8]) -> U8Bytes {
        let mut bytes = [0u8; U8_DATA_MAX_SIZE];
        match slice.len().cmp(&U8_DATA_MAX_SIZE) {
            Ordering::Less => copy(slice.len(), &mut bytes, slice),
            Ordering::Equal | Ordering::Greater => copy(U8_DATA_MAX_SIZE, &mut bytes, slice),
        }
        U8Bytes {
            id,
            data_len,
            bytes,
        }
    }
    pub fn id(&self) -> u16 {
        self.id
    }
    pub fn data_len(&self) -> usize {
        self.data_len
    }
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

fn copy(len: usize, bytes: &mut [u8; U8_DATA_MAX_SIZE], slice: &[u8]) {
    bytes.as_mut_slice()[0..len].copy_from_slice(&slice[0..len]);
}
