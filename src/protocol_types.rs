use async_std::io::{Read, ReadExt, Write, WriteExt};
use async_trait::async_trait;

const SEGMENT_BITS: u8 = 0x7f;
const CONTINUE_BIT: u8 = 0x80;


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
impl ReadProt for i32 {
    async fn read(stream: &mut (impl Read + Unpin + Send)) -> Result<Self, String> {
        let mut value: i32 = 0;
        let mut pos: u32 = 0;
        let mut current_byte: u8 = 0;
        loop {
            let mut buf = vec![0u8; 1];
            stream.read_exact(&mut buf).await.or_else(|x| Err(format!("IO error: {:?}", x)))?;
            current_byte = buf[0];
            value |= ((current_byte & SEGMENT_BITS) as i32) << pos;
            if current_byte & CONTINUE_BIT == 0 { return Ok(value) }
            pos += 7;
            if pos >= 32 {
                return Err("VarInt is too big".into())
            }
        }
    }
}

impl SizedProt for i32 {
    fn size(&self) -> usize {
        let mut x = *self as u32;
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
impl WriteProt for i32 {
    async fn write(&self, stream: &mut (impl Write + Unpin + Send)) -> Result<(), String> {
        let mut x = *self as u32;
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
impl ReadProt for i64 {
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
                break Ok(result);
            }
        }
    }
}

impl SizedProt for i64 {
    fn size(&self) -> usize {
        let mut x = *self as u64;
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
impl WriteProt for i64 {
    async fn write(&self, stream: &mut (impl Write + Unpin + Send)) -> Result<(), String> {
        let mut x = *self as u64;
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
        let len = i32::read(stream).await?;
        let len = len as u32;
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
        (self.len() as i32).write(stream).await?;
        stream.write_all(self.as_bytes()).await.unwrap();
        Ok(())
    }
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

#[cfg(test)]
mod test {
    use crate::protocol_types::WriteProt;
    #[async_std::test]
    async fn i32_0() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        0i32.write(&mut buf).await?;
        assert_eq!(buf[0], 0);
        Ok(())
    }

    #[async_std::test]
    async fn i32_1() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        1i32.write(&mut buf).await?;
        assert_eq!(buf[0], 1);
        Ok(())
    }

    #[async_std::test]
    async fn i32_2() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        2i32.write(&mut buf).await?;
        assert_eq!(buf[0], 2);
        Ok(())
    }

    #[async_std::test]
    async fn i32_127() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        127i32.write(&mut buf).await?;
        assert_eq!(buf[0], 127);
        Ok(())
    }

    #[async_std::test]
    async fn i32_128() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        128i32.write(&mut buf).await?;
        assert_eq!(buf[0], 128);
        assert_eq!(buf[1], 1);
        Ok(())
    }

    #[async_std::test]
    async fn i32_255() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        255i32.write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 1);
        Ok(())
    }

    #[async_std::test]
    async fn i32_25565() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        25565i32.write(&mut buf).await?;
        assert_eq!(buf[0], 221);
        assert_eq!(buf[1], 199);
        assert_eq!(buf[2], 1);
        Ok(())
    }

    #[async_std::test]
    async fn i32_2097151() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        2097151i32.write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 127);
        Ok(())
    }

    #[async_std::test]
    async fn i32_2147483647() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        2147483647i32.write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 255);
        assert_eq!(buf[3], 255);
        assert_eq!(buf[4], 7);
        Ok(())
    }

    #[async_std::test]
    async fn i32_n1() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (-1i32).write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 255);
        assert_eq!(buf[3], 255);
        assert_eq!(buf[4], 15);
        Ok(())
    }

    #[async_std::test]
    async fn i32_n2147483648() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (-2147483648i32).write(&mut buf).await?;
        assert_eq!(buf[0], 128);
        assert_eq!(buf[1], 128);
        assert_eq!(buf[2], 128);
        assert_eq!(buf[3], 128);
        assert_eq!(buf[4], 8);
        Ok(())
    }

    #[async_std::test]
    async fn i64_0() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (0i64).write(&mut buf).await?;
        assert_eq!(buf[0], 0);
        Ok(())
    }

    #[async_std::test]
    async fn i64_1() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (1i64).write(&mut buf).await?;
        assert_eq!(buf[0], 1);
        Ok(())
    }

    #[async_std::test]
    async fn i64_2() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (2i64).write(&mut buf).await?;
        assert_eq!(buf[0], 2);
        Ok(())
    }

    #[async_std::test]
    async fn i64_127() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (127i64).write(&mut buf).await?;
        assert_eq!(buf[0], 127);
        Ok(())
    }

    #[async_std::test]
    async fn i64_128() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (128i64).write(&mut buf).await?;
        assert_eq!(buf[0], 128);
        assert_eq!(buf[1], 1);
        Ok(())
    }

    #[async_std::test]
    async fn i64_255() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (255i64).write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 1);
        Ok(())
    }

    #[async_std::test]
    async fn i64_2147483647() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (2147483647i64).write(&mut buf).await?;
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 255);
        assert_eq!(buf[3], 255);
        assert_eq!(buf[4], 7);
        Ok(())
    }

    #[async_std::test]
    async fn i64_9223372036854775807() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (9223372036854775807i64).write(&mut buf).await?;
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
    async fn i64_n1() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (-1i64).write(&mut buf).await?;
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
    async fn i64_n2147483648() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (-2147483648i64).write(&mut buf).await?;
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
    async fn i64_n9223372036854775808() -> Result<(), String> {
        let mut buf: Vec<u8> = vec![];
        (-9223372036854775808i64).write(&mut buf).await?;
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