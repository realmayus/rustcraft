use md5::{Digest, Md5};
use std::ascii::escape_default;
use tokio::io::{AsyncRead, AsyncReadExt};
use uuid::Uuid;

// Visualize u8 slice in hex
pub(crate) fn show(bs: &[u8]) -> String {
    let mut visible = String::new();
    for &b in bs {
        // visible += format!("{:02x} ", b).as_str();
        let part: Vec<u8> = escape_default(b).collect();
        visible.push_str(std::str::from_utf8(&part).unwrap());
    }
    visible
}

pub(crate) fn name_uuid(name: String) -> Uuid {
    // MD5 digest of name
    let mut hasher = Md5::new();
    hasher.update(name.as_bytes());
    let mut digest = hasher.finalize();
    digest[6] &= 0x0f;
    digest[6] |= 0x30;
    digest[8] &= 0x3f;
    digest[8] |= 0x80;
    Uuid::from_bytes(digest.into())
}

pub(crate) async fn skip(
    stream: &mut (impl AsyncRead + Unpin + Send),
    n: u64,
) -> Result<(), String> {
    // skip n bytes in the given stream
    let mut took = stream.take(n);
    let mut buf = Vec::with_capacity(n as usize);
    took.read_to_end(&mut buf)
        .await
        .or_else(|err| Err(format!("{err}")))?;
    Ok(())
}

#[macro_export]
macro_rules! packet_base {
    ($packet_name:ident $id:literal {
        $( $field:ident, $field_type:ty $(;; $cond:expr)? ),* $(,)*
    }) => {
        #[derive(Debug)]
        pub(crate) struct $packet_name {
            $(
                    $field: packet_base!(@field $field_type, $($cond)?), // Option<$field_type>
            )*
        }

        impl SizedProt for $packet_name {
            fn prot_size(&self) -> usize {
                VarInt::from(Self::id() as usize).prot_size() $(+ self.$field.prot_size())*
            }
        }

        impl Display for $packet_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", stringify!($packet_name))?;
                Ok(())
            }
        }
    };
    (@field $fty:ty, $cond:expr) => {
        Option<$fty>
    };
    (@field $fty:ty $(,)?) => {
        $fty
    };
    (@read $stream: ident, $fty:ty, $cond:expr) => {
        if $cond {
            Some(<$fty>::read($stream).await?)
        } else {
            None
        }
    };
    (@read $stream: ident, $fty:ty $(,)?) => {
        <$fty>::read($stream).await?
    };
}

#[macro_export]
macro_rules! packet {
    // handler provided: server-bound packet
    ($packet_name:ident $id:literal {
        $( $field:ident : $({$cond:expr} && )? $field_type:ty ),* $(,)*
    }, handler |$this:ident, $stream:ident, $conn:ident, $assets:ident| $closure:expr) => {
        packet_base!($packet_name $id {
            $( $field , $field_type  $(;; $cond)? ),*
        });
        #[async_trait]
        impl ServerPacket for $packet_name {
            #[allow(unused)]
            async fn handle(&self, $stream: &mut TcpStream, $conn: &mut Connection,$assets: Arc<Assets>) -> Result<Vec<ClientPackets>, ProtError> {
                let $this = self;
                $closure
            }
            fn id() -> u8 {
                $id
            }
        }

        #[async_trait]
        impl ReadProtPacket for $packet_name {
            #[allow(unused)]
            async fn read(stream: &mut (impl AsyncRead + Unpin + Send), connection: &mut Connection) -> Result<Self, String> where Self: Sized {
                Ok($packet_name {
                    $(
                        $field: packet_base!(@read stream, $field_type, $($cond)?),
                    )*
                })
            }
        }
    };
    // no handler provided: client-bound packet
    ($packet_name:ident $id:literal {
        $( $field:ident : $({$cond:expr} && )? $field_type:ty ),* $(,)*
    }) => {
        packet_base!($packet_name $id {
            $( $field, $field_type $(;; $cond)? ),*
        });
        #[async_trait]
        impl ClientPacket for $packet_name {
            fn id() -> u8 {
                $id
            }
        }

        impl $packet_name {
            pub(crate) fn new($($field: packet_base!(@field $field_type, $($cond)?),)*) -> Self {
                Self {
                    $(
                        $field,
                    )*
                }
            }
        }

        #[async_trait]
        impl WriteProtPacket for $packet_name {
            #[allow(unused)]
            async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send), connection: &mut Connection) -> Result<(), String> {
                debug!("Outbound packet: {self:?} (len {})", self.prot_size() + VarInt::from(self.prot_size()).prot_size());
                let mut buf: Vec<u8> = Vec::with_capacity(self.prot_size() + VarInt::from(self.prot_size()).prot_size());
                VarInt::from(self.prot_size()).write(&mut buf).await?;
                VarInt::from(Self::id() as usize).write(&mut buf).await?;
                $(
                    self.$field.write(&mut buf).await?;
                )*
                // encrypt `buf` with AES/CFB8 using `shared_secret` as the key.
                if let Some(encrypter) = &mut connection.encrypter {
                    let mut encrypted_buf = vec![0u8; buf.len()];
                    encrypter.update(buf.as_slice(), &mut encrypted_buf).unwrap();
                    stream.write_all(&encrypted_buf).await.or_else(|err| Err(format!("{err}")))?;
                } else {
                    stream.write_all(&buf).await.or_else(|err| Err(format!("{err}")))?;
                }
                Ok(())
            }
        }
    };
}
