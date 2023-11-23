use std::fmt::{Debug, Display, Formatter};

use crate::protocol_types::traits::{ReadProt, SizedProt, WriteProt};
use async_trait::async_trait;
use openssl::symm::Crypter;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const SEGMENT_BITS: u8 = 0x7f;
const CONTINUE_BIT: u8 = 0x80;

#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Copy)]
pub(crate) struct VarInt {
    pub(crate) value: i32,
}

impl VarInt {
    async fn get_byte(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<u8, String> {
        let mut buf = vec![0u8; 1];
        stream
            .read_exact(&mut buf)
            .await
            .or_else(|x| Err(format!("Trying to read byte: {:?}", x)))?;
        Ok(buf[0])
    }

    async fn get_byte_decrypt(
        stream: &mut (impl AsyncRead + Unpin + Send),
        crypter: &mut Crypter,
    ) -> Result<u8, String> {
        let mut temp = vec![0u8; 1];
        let byte = Self::get_byte(stream).await?;
        crypter
            .update(&[byte], &mut temp)
            .or_else(|x| Err(format!("Crypter error: {:?}", x)))?;
        Ok(temp[0])
    }

    pub(crate) async fn read_decrypt(
        stream: &mut (impl AsyncRead + Unpin + Send),
        crypter: &mut Crypter,
    ) -> Result<Self, String> {
        let mut value: i32 = 0;
        let mut pos: u32 = 0;
        let mut current_byte: u8;
        loop {
            current_byte = Self::get_byte_decrypt(stream, crypter).await?;
            value |= ((current_byte & SEGMENT_BITS) as i32) << pos;
            if current_byte & CONTINUE_BIT == 0 {
                return Ok(Self { value });
            }
            pos += 7;
            if pos >= 32 {
                return Err("VarInt is too big".into());
            }
        }
    }
}

impl From<usize> for VarInt {
    fn from(value: usize) -> Self {
        Self {
            value: value as i32,
        }
    }
}

impl From<i32> for VarInt {
    fn from(value: i32) -> Self {
        Self { value }
    }
}

impl Display for VarInt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl Debug for VarInt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub(crate) struct VarLong {
    value: i64,
}

impl From<i64> for VarLong {
    fn from(value: i64) -> Self {
        Self { value }
    }
}

impl Display for VarLong {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl Debug for VarLong {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[async_trait]
impl ReadProt for VarInt {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String> {
        let mut value: i32 = 0;
        let mut pos: u32 = 0;
        let mut current_byte: u8;
        loop {
            let mut buf = vec![0u8; 1];
            stream
                .read_exact(&mut buf)
                .await
                .or_else(|x| Err(format!("IO error: {:?}", x)))?;
            current_byte = buf[0];
            value |= ((current_byte & SEGMENT_BITS) as i32) << pos;
            if current_byte & CONTINUE_BIT == 0 {
                return Ok(Self { value });
            }
            pos += 7;
            if pos >= 32 {
                return Err("VarInt is too big".into());
            }
        }
    }
}

impl SizedProt for VarInt {
    fn prot_size(&self) -> usize {
        let mut x = self.value as u32;
        let mut count = 0;
        loop {
            x >>= 7;
            count += 1;

            if x == 0 {
                break count;
            }
        }
    }
}

#[async_trait]
impl WriteProt for VarInt {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        let mut x = self.value as u32;
        loop {
            let mut temp = (x & 0b0111_1111) as u8;
            x >>= 7;
            if x != 0 {
                temp |= 0b1000_0000;
            }

            stream
                .write_all(&[temp])
                .await
                .or_else(|x| Err(format!("IO error: {:?}", x)))?;

            if x == 0 {
                break;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ReadProt for VarLong {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String> {
        let mut result = 0;
        let mut num_read = 0;
        loop {
            let mut buf = vec![0u8; 1];
            stream
                .read_exact(&mut buf)
                .await
                .or_else(|x| Err(format!("IO error: {:?}", x)))?;
            let read = buf[0];
            let value = i64::from(read & 0b0111_1111);
            result |= value.overflowing_shl(7 * num_read).0;

            num_read += 1;

            if num_read > 10 {
                break Err(format!(
                    "VarInt too long (max length: 5, value read so far: {})",
                    result
                ));
            }
            if read & 0b1000_0000 == 0 {
                break Ok(Self { value: result });
            }
        }
    }
}

impl SizedProt for VarLong {
    fn prot_size(&self) -> usize {
        let mut x = self.value as u64;
        let mut count = 0;
        loop {
            x >>= 7;
            count += 1;

            if x == 0 {
                break count;
            }
        }
    }
}

#[async_trait]
impl WriteProt for VarLong {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        let mut x = self.value as u64;
        loop {
            let mut temp = (x & 0b0111_1111) as u8;
            x >>= 7;
            if x != 0 {
                temp |= 0b1000_0000;
            }

            stream
                .write_all(&[temp])
                .await
                .or_else(|x| Err(format!("IO error: {:?}", x)))?;

            if x == 0 {
                break;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ReadProt for String {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        let len = VarInt::read(stream).await?;
        let len = len.value as u32;
        if len > 32767 * 4 + 3 {
            return Err(format!("String too long: {} B", len));
        }

        let mut data = stream.take(len as u64);
        let mut buf = vec![];
        data.read_to_end(&mut buf)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))?;
        let value = String::from_utf8(buf).or_else(|x| Err(format!("UTF8 error: {:?}", x)))?;
        Ok(value)
    }
}

#[async_trait]
impl WriteProt for String {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        (VarInt {
            value: self.len() as i32,
        })
        .write(stream)
        .await?;
        stream.write_all(self.as_bytes()).await.unwrap();
        Ok(())
    }
}

impl SizedProt for String {
    fn prot_size(&self) -> usize {
        VarInt::from(self.len()).prot_size() + self.len()
    }
}

#[inline]
fn u64tou8abe(v: u64) -> [u8; 8] {
    [
        (v >> 56) as u8,
        (v >> 48) as u8,
        (v >> 40) as u8,
        (v >> 32) as u8,
        (v >> 24) as u8,
        (v >> 16) as u8,
        (v >> 8) as u8,
        v as u8,
    ]
}

#[inline]
fn u32tou8abe(v: u32) -> [u8; 4] {
    [(v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8]
}

#[inline]
fn u16tou8abe(v: u16) -> [u8; 2] {
    [(v >> 8) as u8, v as u8]
}

#[async_trait]
impl ReadProt for i32 {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        let mut buffer = [0; 4];
        stream
            .read_exact(&mut buffer)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))?;
        let mut value: u32 = buffer[0] as u32;
        value <<= 8;
        value |= buffer[1] as u32;
        value <<= 8;
        value |= buffer[2] as u32;
        value <<= 8;
        value |= buffer[3] as u32;

        Ok(value as i32)
    }
}

#[async_trait]
impl WriteProt for i32 {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        let data = u32tou8abe(*self as u32);
        stream
            .write_all(&data)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))?;
        Ok(())
    }
}

impl SizedProt for i32 {
    fn prot_size(&self) -> usize {
        4
    }
}

#[async_trait]
impl ReadProt for i16 {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String> where Self: Sized {
        stream.read_i16().await.or_else(|x| Err(format!("IO error: {:?}", x)))
    }
}

#[async_trait]
impl WriteProt for i16 {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        stream.write_i16(*self).await.or_else(|x| Err(format!("IO error: {:?}", x)))
    }
}

#[async_trait]
impl SizedProt for i16 {
    fn prot_size(&self) -> usize {
        2
    }
}

#[async_trait]
impl ReadProt for u8 {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        let mut buffer = [0; 1];
        stream
            .read_exact(&mut buffer)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))?;

        let value = buffer[0];
        Ok(value)
    }
}

#[async_trait]
impl WriteProt for u8 {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        stream
            .write_all(&[*self])
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))?;
        Ok(())
    }
}

impl SizedProt for u8 {
    fn prot_size(&self) -> usize {
        1
    }
}

#[async_trait]
impl ReadProt for bool {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(u8::read(stream).await? == 0x01)
    }
}

#[async_trait]
impl WriteProt for bool {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        u8::write(&if *self { 0x01 } else { 0x00 }, stream)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))?; // 0x01 = true, 0x00 = false
        Ok(())
    }
}

impl SizedProt for bool {
    fn prot_size(&self) -> usize {
        1
    }
}

#[async_trait]
impl ReadProt for u16 {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        let mut buffer = [0; 2];
        stream
            .read_exact(&mut buffer)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))?;

        let value = ((buffer[0] as u16) << 8) | buffer[1] as u16;
        Ok(value)
    }
}

#[async_trait]
impl WriteProt for u16 {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        let data = u16tou8abe(*self);
        stream
            .write_all(&data)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))?;
        Ok(())
    }
}

impl SizedProt for u16 {
    fn prot_size(&self) -> usize {
        2
    }
}

#[async_trait]
impl ReadProt for i64 {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        let mut buffer = [0; 8];
        stream
            .read_exact(&mut buffer)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))?;
        let mut value: u64 = buffer[0] as u64;
        value <<= 8;
        value |= buffer[1] as u64;
        value <<= 8;
        value |= buffer[2] as u64;
        value <<= 8;
        value |= buffer[3] as u64;
        value <<= 8;
        value |= buffer[4] as u64;
        value <<= 8;
        value |= buffer[5] as u64;
        value <<= 8;
        value |= buffer[6] as u64;
        value <<= 8;
        value |= buffer[7] as u64;

        Ok(value as i64)
    }
}

#[async_trait]
impl WriteProt for i64 {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        let data = u64tou8abe(*self as u64);
        stream
            .write_all(&data)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))?;
        Ok(())
    }
}

impl SizedProt for i64 {
    fn prot_size(&self) -> usize {
        8
    }
}

#[async_trait]
impl ReadProt for u64 {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        let mut buffer = [0; 8];
        stream
            .read_exact(&mut buffer)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))?;
        let mut value: u64 = buffer[0] as u64;
        value <<= 8;
        value |= buffer[1] as u64;
        value <<= 8;
        value |= buffer[2] as u64;
        value <<= 8;
        value |= buffer[3] as u64;
        value <<= 8;
        value |= buffer[4] as u64;
        value <<= 8;
        value |= buffer[5] as u64;
        value <<= 8;
        value |= buffer[6] as u64;
        value <<= 8;
        value |= buffer[7] as u64;

        Ok(value as u64)
    }
}

#[async_trait]
impl WriteProt for u64 {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        let data = u64tou8abe(*self as u64);
        stream
            .write_all(&data)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))?;
        Ok(())
    }
}

impl SizedProt for u64 {
    fn prot_size(&self) -> usize {
        8
    }
}

#[async_trait]
impl WriteProt for f32 {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        stream
            .write_f32(*self)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))
    }
}

#[async_trait]
impl ReadProt for f32 {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        stream
            .read_f32()
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))
    }
}

impl SizedProt for f32 {
    fn prot_size(&self) -> usize {
        4
    }
}

#[async_trait]
impl WriteProt for f64 {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        stream
            .write_f64(*self)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))
    }
}

#[async_trait]
impl ReadProt for f64 {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        stream
            .read_f64()
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))
    }
}

impl SizedProt for f64 {
    fn prot_size(&self) -> usize {
        8
    }
}

#[derive(Debug)]
pub(crate) struct SizedVec<T>
where
    T: Send + Sync,
{
    pub(crate) vec: Vec<T>,
}

impl<T> From<Vec<T>> for SizedVec<T>
where
    T: Send + Sync,
{
    fn from(value: Vec<T>) -> Self {
        Self { vec: value }
    }
}

#[async_trait]
impl<T> WriteProt for SizedVec<T>
where
    T: WriteProt + Sync + Send,
{
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        VarInt::from(self.vec.len()).write(stream).await?;
        for item in &self.vec {
            item.write(stream).await?;
        }
        Ok(())
    }
}

// Reading a Vec<u8> assumes the length of the vec is announced as a VarInt in the stream just before the bytearray.
#[async_trait]
impl<T> ReadProt for SizedVec<T>
where
    T: ReadProt + Sync + SizedProt + Send,
{
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        let len = VarInt::read(stream).await?;
        let len = len.value as usize;
        let mut bytes_so_far = 0;
        let mut buf = vec![];
        loop {
            if bytes_so_far == len {
                break;
            }
            buf.push(T::read(stream).await?);
            bytes_so_far += buf.last().unwrap().prot_size();
        }
        Ok(Self { vec: buf })
    }
}

impl<T> SizedProt for SizedVec<T>
where
    T: SizedProt + Send + Sync,
{
    fn prot_size(&self) -> usize {
        VarInt::from(self.vec.len()).prot_size()
            + self.vec.iter().map(|x| x.prot_size()).sum::<usize>()
    }
}

#[async_trait]
impl<const N: usize> WriteProt for [u8; N] {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        stream
            .write_all(self)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))?;
        Ok(())
    }
}

#[async_trait]
impl<const N: usize> ReadProt for [u8; N] {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized,
    {
        let mut buf = [0u8; N];
        stream
            .read_exact(&mut buf)
            .await
            .or_else(|x| Err(format!("IO error: {:?}", x)))?;
        Ok(buf)
    }
}

impl<const N: usize> SizedProt for [u8; N] {
    fn prot_size(&self) -> usize {
        N
    }
}

impl<T> SizedProt for Option<T>
where
    T: SizedProt,
{
    fn prot_size(&self) -> usize {
        match self {
            Some(x) => x.prot_size(),
            None => 0,
        }
    }
}

#[async_trait]
impl<T> WriteProt for Option<T>
where
    T: WriteProt + Sync,
{
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        match self {
            Some(x) => {
                x.write(stream).await?;
            }
            None => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::{VarInt, VarLong, WriteProt};

    #[tokio::test]
    async fn varint_0() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt { value: 0 }.write(&mut buf).await?;
        assert_eq!(buf[0], 0);
        Ok(())
    }

    #[tokio::test]
    async fn varint_1() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt { value: 1 }.write(&mut buf).await?;
        assert_eq!(buf[0], 1);
        Ok(())
    }

    #[tokio::test]
    async fn varint_2() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt { value: 2 }.write(&mut buf).await?;
        assert_eq!(buf[0], 2);
        Ok(())
    }

    #[tokio::test]
    async fn varint_127() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt { value: 127 }.write(&mut buf).await?;
        assert_eq!(buf[0], 127);
        Ok(())
    }

    #[tokio::test]
    async fn varint_128() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt { value: 128 }.write(&mut buf).await?;
        assert_eq!(buf[0], 128);
        assert_eq!(buf[1], 1);
        Ok(())
    }

    #[tokio::test]
    async fn varint_255() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt { value: 255 }.write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 1);
        Ok(())
    }

    #[tokio::test]
    async fn varint_25565() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt { value: 25565 }.write(&mut buf).await?;
        assert_eq!(buf[0], 221);
        assert_eq!(buf[1], 199);
        assert_eq!(buf[2], 1);
        Ok(())
    }

    #[tokio::test]
    async fn varint_2097151() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt { value: 2097151 }.write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 127);
        Ok(())
    }

    #[tokio::test]
    async fn varint_2147483647() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt { value: 2147483647 }.write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 255);
        assert_eq!(buf[3], 255);
        assert_eq!(buf[4], 7);
        Ok(())
    }

    #[tokio::test]
    async fn varint_n1() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt { value: -1 }.write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 255);
        assert_eq!(buf[3], 255);
        assert_eq!(buf[4], 15);
        Ok(())
    }

    #[tokio::test]
    async fn varint_n2147483648() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt { value: -2147483648 }.write(&mut buf).await?;
        assert_eq!(buf[0], 128);
        assert_eq!(buf[1], 128);
        assert_eq!(buf[2], 128);
        assert_eq!(buf[3], 128);
        assert_eq!(buf[4], 8);
        Ok(())
    }

    #[tokio::test]
    async fn varlong_0() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong { value: 0 }).write(&mut buf).await?;
        assert_eq!(buf[0], 0);
        Ok(())
    }

    #[tokio::test]
    async fn varlong_1() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong { value: 1 }).write(&mut buf).await?;
        assert_eq!(buf[0], 1);
        Ok(())
    }

    #[tokio::test]
    async fn varlong_2() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong { value: 2 }).write(&mut buf).await?;
        assert_eq!(buf[0], 2);
        Ok(())
    }

    #[tokio::test]
    async fn varlong_127() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong { value: 127 }).write(&mut buf).await?;
        assert_eq!(buf[0], 127);
        Ok(())
    }

    #[tokio::test]
    async fn varlong_128() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong { value: 128 }).write(&mut buf).await?;
        assert_eq!(buf[0], 128);
        assert_eq!(buf[1], 1);
        Ok(())
    }

    #[tokio::test]
    async fn varlong_255() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong { value: 255 }).write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 1);
        Ok(())
    }

    #[tokio::test]
    async fn varlong_2147483647() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong { value: 2147483647 }).write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 255);
        assert_eq!(buf[3], 255);
        assert_eq!(buf[4], 7);
        Ok(())
    }

    #[tokio::test]
    async fn varlong_9223372036854775807() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong {
            value: 9223372036854775807,
        })
        .write(&mut buf)
        .await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 255);
        assert_eq!(buf[3], 255);
        assert_eq!(buf[4], 255);
        assert_eq!(buf[5], 255);
        assert_eq!(buf[6], 255);
        assert_eq!(buf[7], 255);
        assert_eq!(buf[8], 127);
        Ok(())
    }

    #[tokio::test]
    async fn varlong_n1() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong { value: -1 }).write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 255);
        assert_eq!(buf[3], 255);
        assert_eq!(buf[4], 255);
        assert_eq!(buf[5], 255);
        assert_eq!(buf[6], 255);
        assert_eq!(buf[7], 255);
        assert_eq!(buf[8], 255);
        assert_eq!(buf[9], 1);
        Ok(())
    }

    #[tokio::test]
    async fn varlong_n2147483648() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong { value: -2147483648 }).write(&mut buf).await?;
        assert_eq!(buf[0], 128);
        assert_eq!(buf[1], 128);
        assert_eq!(buf[2], 128);
        assert_eq!(buf[3], 128);
        assert_eq!(buf[4], 248);
        assert_eq!(buf[5], 255);
        assert_eq!(buf[6], 255);
        assert_eq!(buf[7], 255);
        assert_eq!(buf[8], 255);
        assert_eq!(buf[9], 1);
        Ok(())
    }

    #[tokio::test]
    async fn varlong_n9223372036854775808() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong {
            value: -9223372036854775808,
        })
        .write(&mut buf)
        .await?;
        assert_eq!(buf[0], 128);
        assert_eq!(buf[1], 128);
        assert_eq!(buf[2], 128);
        assert_eq!(buf[3], 128);
        assert_eq!(buf[4], 128);
        assert_eq!(buf[5], 128);
        assert_eq!(buf[6], 128);
        assert_eq!(buf[7], 128);
        assert_eq!(buf[8], 128);
        assert_eq!(buf[9], 1);
        Ok(())
    }
}
