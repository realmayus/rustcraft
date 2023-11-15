use std::fmt::{Debug, Display, Formatter};
use async_std::io::{Read, ReadExt, Write, WriteExt};
use async_trait::async_trait;

const SEGMENT_BITS: u8 = 0x7f;
const CONTINUE_BIT: u8 = 0x80;

#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub(crate) struct VarInt {
    pub(crate) value: i32
}

impl From<usize> for VarInt {
    fn from(value: usize) -> Self {
        Self { value: value as i32 }
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
    value: i64
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
pub(crate) trait ReadProt {
    async fn read(stream: &mut (impl Read + Unpin + Send)) -> Result<Self, String> where Self: Sized;
}

#[async_trait]
pub(crate) trait WriteProt {
    async fn write(&self, stream: &mut (impl Write + Unpin + Send)) -> Result<(), String>;
}

pub(crate) trait SizedProt {
    fn size(&self) -> usize;
}


#[async_trait]
impl ReadProt for VarInt {
    async fn read(stream: &mut (impl Read + Unpin + Send)) -> Result<Self, String> {
        let mut value: i32 = 0;
        let mut pos: u32 = 0;
        let mut current_byte: u8 = 0;
        loop {
            let mut buf = vec![0u8; 1];
            stream.read_exact(&mut buf).await.or_else(|x| Err(format!("IO error: {:?}", x)))?;
            current_byte = buf[0];
            value |= ((current_byte & SEGMENT_BITS) as i32) << pos;
            if current_byte & CONTINUE_BIT == 0 { return Ok(Self { value }) }
            pos += 7;
            if pos >= 32 {
                return Err("VarInt is too big".into())
            }
        }
    }
}

impl SizedProt for VarInt {
    fn size(&self) -> usize {
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
    async fn write(&self, stream: &mut (impl Write + Unpin + Send)) -> Result<(), String> {
        let mut x = self.value as u32;
        loop {
            let mut temp = (x & 0b0111_1111) as u8;
            x >>= 7;
            if x != 0 {
                temp |= 0b1000_0000;
            }

            stream.write_all(&[temp]).await.or_else(|x| Err(format!("IO error: {:?}", x)))?;

            if x == 0 {
                break;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ReadProt for VarLong {
    async fn read(stream: &mut (impl Read + Unpin + Send)) -> Result<Self, String> {
        let mut result = 0;
        let mut num_read = 0;
        loop {
            let mut buf = vec![0u8; 1];
            stream.read_exact(&mut buf).await.or_else(|x| Err(format!("IO error: {:?}", x)))?;
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
                break Ok(Self {value: result});
            }
        }
    }
}

impl SizedProt for VarLong {
    fn size(&self) -> usize {
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
    async fn write(&self, stream: &mut (impl Write + Unpin + Send)) -> Result<(), String> {
        let mut x = self.value as u64;
        loop {
            let mut temp = (x & 0b0111_1111) as u8;
            x >>= 7;
            if x != 0 {
                temp |= 0b1000_0000;
            }

            stream.write_all(&[temp]).await.or_else(|x| Err(format!("IO error: {:?}", x)))?;

            if x == 0 {
                break;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ReadProt for String {
    async fn read(stream: &mut (impl Read + Unpin + Send)) -> Result<Self, String> where Self: Sized {
        let len = VarInt::read(stream).await?;
        let len = len.value as u32;
        if len > 32767*4 + 3 { return Err(format!("String too long: {} B", len)) }

        let mut data = stream.take(len as u64);
        let mut buf = vec![];
        data.read_to_end(&mut buf).await.or_else(|x| Err(format!("IO error: {:?}", x)))?;
        let value = String::from_utf8(buf).or_else(|x| Err(format!("UTF8 error: {:?}", x)))?;
        Ok(value)
    }
}

#[async_trait]
impl WriteProt for String {
    async fn write(&self, stream: &mut (impl Write + Unpin + Send)) -> Result<(), String> {
        (VarInt { value: self.len() as i32 }).write(stream).await?;
        stream.write_all(self.as_bytes()).await.unwrap();
        Ok(())
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
    [
        (v >> 24) as u8,
        (v >> 16) as u8,
        (v >> 8) as u8,
        v as u8,
    ]
}

#[inline]
fn u16tou8abe(v: u16) -> [u8; 2] {
    [
        (v >> 8) as u8,
        v as u8,
    ]
}

#[async_trait]
impl ReadProt for u16 {
    async fn read(stream: &mut (impl Read + Unpin + Send)) -> Result<Self, String> where Self: Sized {
        let mut buffer = [0; 2];
        stream.read_exact(&mut buffer).await.or_else(|x| Err(format!("IO error: {:?}", x)))?;

        let value = ((buffer[0] as u16) << 8) | buffer[1] as u16;
        Ok(value)
    }
}

#[async_trait]
impl WriteProt for u16 {
    async fn write(&self, stream: &mut (impl Write + Unpin + Send)) -> Result<(), String> {
        let data = u16tou8abe(*self);
        stream.write_all(&data).await.or_else(|x| Err(format!("IO error: {:?}", x)))?;
        Ok(())
    }
}

impl SizedProt for u16 {
    fn size(&self) -> usize {
        2
    }
}

#[async_trait]
impl ReadProt for i64 {
    async fn read(stream: &mut (impl Read + Unpin + Send)) -> Result<Self, String> where Self: Sized {
        let mut buffer = [0; 8];
        stream.read_exact(&mut buffer).await.or_else(|x| Err(format!("IO error: {:?}", x)))?;
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
    async fn write(&self, stream: &mut (impl Write + Unpin + Send)) -> Result<(), String> {
        let data = u64tou8abe(*self as u64);
        stream.write_all(&data).await.or_else(|x| Err(format!("IO error: {:?}", x)))?;
        Ok(())
    }
}

impl SizedProt for i64 {
    fn size(&self) -> usize {
        8
    }
}


#[cfg(test)]
mod test {
    use crate::protocol_types::{VarInt, VarLong, WriteProt};
    #[async_std::test]
    async fn varint_0() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt{ value: 0 }.write(&mut buf).await?;
        assert_eq!(buf[0], 0);
        Ok(())
    }

    #[async_std::test]
    async fn varint_1() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt{ value: 1 }.write(&mut buf).await?;
        assert_eq!(buf[0], 1);
        Ok(())
    }

    #[async_std::test]
    async fn varint_2() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt{ value: 2 }.write(&mut buf).await?;
        assert_eq!(buf[0], 2);
        Ok(())
    }

    #[async_std::test]
    async fn varint_127() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt{ value: 127 }.write(&mut buf).await?;
        assert_eq!(buf[0], 127);
        Ok(())
    }

    #[async_std::test]
    async fn varint_128() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt{ value: 128 }.write(&mut buf).await?;
        assert_eq!(buf[0], 128);
        assert_eq!(buf[1], 1);
        Ok(())
    }

    #[async_std::test]
    async fn varint_255() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt{ value: 255 }.write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 1);
        Ok(())
    }

    #[async_std::test]
    async fn varint_25565() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt{ value: 25565 }.write(&mut buf).await?;
        assert_eq!(buf[0], 221);
        assert_eq!(buf[1], 199);
        assert_eq!(buf[2], 1);
        Ok(())
    }

    #[async_std::test]
    async fn varint_2097151() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt{ value: 2097151 }.write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 127);
        Ok(())
    }

    #[async_std::test]
    async fn varint_2147483647() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt{ value: 2147483647 }.write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 255);
        assert_eq!(buf[3], 255);
        assert_eq!(buf[4], 7);
        Ok(())
    }

    #[async_std::test]
    async fn varint_n1() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt{ value: -1 }.write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 255);
        assert_eq!(buf[3], 255);
        assert_eq!(buf[4], 15);
        Ok(())
    }

    #[async_std::test]
    async fn varint_n2147483648() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        VarInt{ value: -2147483648 }.write(&mut buf).await?;
        assert_eq!(buf[0], 128);
        assert_eq!(buf[1], 128);
        assert_eq!(buf[2], 128);
        assert_eq!(buf[3], 128);
        assert_eq!(buf[4], 8);
        Ok(())
    }

    #[async_std::test]
    async fn varlong_0() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong{ value: 0 }).write(&mut buf).await?;
        assert_eq!(buf[0], 0);
        Ok(())
    }

    #[async_std::test]
    async fn varlong_1() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong{ value: 1 }).write(&mut buf).await?;
        assert_eq!(buf[0], 1);
        Ok(())
    }

    #[async_std::test]
    async fn varlong_2() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong{ value: 2 }).write(&mut buf).await?;
        assert_eq!(buf[0], 2);
        Ok(())
    }

    #[async_std::test]
    async fn varlong_127() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong{ value: 127 }).write(&mut buf).await?;
        assert_eq!(buf[0], 127);
        Ok(())
    }

    #[async_std::test]
    async fn varlong_128() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong{ value: 128 }).write(&mut buf).await?;
        assert_eq!(buf[0], 128);
        assert_eq!(buf[1], 1);
        Ok(())
    }

    #[async_std::test]
    async fn varlong_255() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong{ value: 255 }).write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 1);
        Ok(())
    }

    #[async_std::test]
    async fn varlong_2147483647() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong{ value: 2147483647 }).write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 255);
        assert_eq!(buf[3], 255);
        assert_eq!(buf[4], 7);
        Ok(())
    }

    #[async_std::test]
    async fn varlong_9223372036854775807() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong{ value: 9223372036854775807 }).write(&mut buf).await?;
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

    #[async_std::test]
    async fn varlong_n1() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong{ value: -1 }).write(&mut buf).await?;
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

    #[async_std::test]
    async fn varlong_n2147483648() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong{ value: -2147483648 }).write(&mut buf).await?;
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

    #[async_std::test]
    async fn varlong_n9223372036854775808() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (VarLong{ value: -9223372036854775808 }).write(&mut buf).await?;
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