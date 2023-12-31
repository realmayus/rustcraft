// Taken from https://github.com/feather-rs/feather/blob/main/feather/base/src/chunk/packed_array.rs (Apache 2.0 license)

use crate::protocol_types::primitives::VarInt;
use crate::protocol_types::traits::WriteProt;
use async_trait::async_trait;
use std::fmt::{Debug, Formatter};
use tokio::io::AsyncWrite;

/// A packed array of integers where each integer consumes
/// `n` bits. Used to store block data in chunks.
#[derive(Clone)]
pub struct PackedArray {
    length: usize,
    bits_per_value: usize,
    bits: Vec<u64>,
}

impl Debug for PackedArray {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let data = self
            .iter()
            .map(|x| format!("{:0width$b}", x, width = self.bits_per_value))
            .collect::<Vec<_>>()
            .join(", ");
        f.debug_struct("PackedArray")
            .field("length", &self.length)
            .field("bits_per_value", &self.bits_per_value)
            .field("bits", &data)
            .finish()
    }
}

impl PackedArray {
    /// Creates a new `PackedArray` with the given length
    /// and number of bits per value. Values are initialized
    /// to zero.
    ///
    /// # Panics
    /// Panics if `bits_per_value > 64`.
    pub fn new(length: usize, bits_per_value: usize) -> Self {
        let mut this = Self {
            length,
            bits_per_value,
            bits: Vec::new(),
        };
        let needed_u64s = this.needed_u64s();
        this.bits = vec![0u64; needed_u64s];

        this
    }

    /// Creates a `PackedArray` from raw `u64` data
    /// and a length.
    pub fn from_u64_vec(bits: Vec<u64>, length: usize) -> Self {
        let bits_per_value = bits.len() * 64 / length;
        Self {
            length,
            bits_per_value,
            bits,
        }
    }

    /// Gets the value at the given index.
    #[inline]
    pub fn get(&self, index: usize) -> Option<u64> {
        if index >= self.len() {
            return None;
        }

        let (u64_index, bit_index) = self.indexes(index);

        let u64 = self.bits[u64_index];
        Some((u64 >> bit_index) & self.mask())
    }

    /// Sets the value at the given index.
    ///
    /// # Panics
    /// Panics if `index >= self.length()` or `value > self.max_value()`.
    #[inline]
    pub fn set(&mut self, index: usize, value: u64) {
        assert!(
            index < self.len(),
            "index out of bounds: index is {}; length is {}",
            index,
            self.len()
        );

        let mask = self.mask();
        assert!(value <= mask);

        let (u64_index, bit_index) = self.indexes(index);

        let u64 = &mut self.bits[u64_index];
        *u64 &= !(mask << bit_index);
        *u64 |= value << bit_index;
    }

    /// Sets all values is the packed array to `value`.
    ///
    /// # Panics
    /// Panics if `value > self.max_value()`.
    pub fn fill(&mut self, value: u64) {
        assert!(value <= self.max_value());
        let mut x = 0;
        for i in 0..self.values_per_u64() {
            x |= value << (i * self.bits_per_value);
        }

        self.bits.fill(x);
    }

    /// Returns an iterator over values in this array.
    pub fn iter(&self) -> impl Iterator<Item = u64> + '_ {
        let values_per_u64 = self.values_per_u64();
        let bits_per_value = self.bits_per_value() as u64;
        let mask = self.mask();
        let length = self.len();

        self.bits
            .iter()
            .flat_map(move |&u64| {
                (0..values_per_u64).map(move |i| (u64 >> (i as u64 * bits_per_value)) & mask)
            })
            .take(length)
    }

    /// Resizes this packed array to a new bits per value.
    pub fn resized(&mut self, new_bits_per_value: usize) -> PackedArray {
        Self::from_iter(self.iter(), new_bits_per_value)
    }

    /// Collects an iterator into a `PackedArray`.
    pub fn from_iter(iter: impl IntoIterator<Item = u64>, bits_per_value: usize) -> Self {
        assert!(bits_per_value <= 64);
        let iter = iter.into_iter();
        let mut bits = Vec::with_capacity(iter.size_hint().0);

        let mut current_u64 = 0u64;
        let mut current_offset = 0;
        let mut length = 0;

        for value in iter {
            debug_assert!(value < 1 << bits_per_value);
            current_u64 |= value << current_offset;

            current_offset += bits_per_value;
            if current_offset > 64 - bits_per_value {
                bits.push(current_u64);
                current_offset = 0;
                current_u64 = 0;
            }

            length += 1;
        }

        if current_offset != 0 {
            bits.push(current_u64);
        }

        Self {
            length,
            bits_per_value,
            bits,
        }
    }

    /// Returns the maximum value of an integer in this packed array.
    #[inline]
    pub fn max_value(&self) -> u64 {
        self.mask()
    }

    /// Returns the length of this packed array.
    #[inline]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Determines whether the length of this array is 0.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of bits used to represent each value.
    #[inline]
    pub fn bits_per_value(&self) -> usize {
        self.bits_per_value
    }

    pub fn set_bits_per_value(&mut self, new_value: usize) {
        self.bits_per_value = new_value;
    }

    /// Gets the raw `u64` data.
    pub fn as_u64_slice(&self) -> &[u64] {
        &self.bits
    }

    pub fn as_u64_mut_vec(&mut self) -> &mut Vec<u64> {
        &mut self.bits
    }

    fn mask(&self) -> u64 {
        (1 << self.bits_per_value) - 1
    }

    fn needed_u64s(&self) -> usize {
        (self.length + self.values_per_u64() - 1) / self.values_per_u64()
    }

    fn values_per_u64(&self) -> usize {
        64 / self.bits_per_value
    }

    fn indexes(&self, index: usize) -> (usize, usize) {
        let u64_index = index / self.values_per_u64();
        let bit_index = (index % self.values_per_u64()) * self.bits_per_value;

        (u64_index, bit_index)
    }
}

#[async_trait]
impl WriteProt for PackedArray {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        VarInt::from(self.needed_u64s()).write(stream).await?;
        for i in 0..self.bits.len() {
            self.bits[i].write(stream).await?;
        }
        Ok(())
    }
}
