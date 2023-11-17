use std::ascii::escape_default;

// Visualize u8 slice in hex
pub(crate) fn show(bs: &[u8]) -> String {
    let mut visible = String::new();
    for &b in bs {
        let part: Vec<u8> = escape_default(b).collect();
        visible.push_str(std::str::from_utf8(&part).unwrap());
    }
    visible
}


trait Packet {
    fn id() -> u8;
}

#[macro_export]
macro_rules! packet_base {
    ($packet_name:ident $id:literal {
        $( $field:ident : $field_type:ty ),* $(,)*
    }) => {
        #[derive(Debug)]
        pub(crate) struct $packet_name {
            $(
                $field : $field_type,
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
}

#[macro_export]
macro_rules! packet {
    // handler provided: server-bound packet
    ($packet_name:ident $id:literal {
        $( $field:ident : $field_type:ty ),* $(,)*
    }, handler |$this:ident, $stream:ident, $conn:ident, $assets:ident| $closure:expr) => {
        packet_base!($packet_name $id {
            $( $field : $field_type ),*
        });
        #[async_trait]
        impl ServerPacket for $packet_name {
            async fn handle(&self, $stream: &mut TcpStream, $conn: &mut Connection,$assets: Arc<Assets>) -> Result<(), String> {
                let $this = self;
                $closure
            }
            fn id() -> u8 {
                $id
            }
        }

        #[async_trait]
        impl ReadProt for $packet_name {
            async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String> where Self: Sized {
                Ok($packet_name {
                    $(
                        $field: <$field_type>::read(stream).await?,
                    )*
                })
            }
        }
    };
    // no handler provided: client-bound packet
    ($packet_name:ident $id:literal {
        $( $field:ident : $field_type:ty ),* $(,)*
    }) => {
        packet_base!($packet_name $id {
            $( $field : $field_type ),*
        });
        #[async_trait]
        impl ClientPacket for $packet_name {
            fn id() -> u8 {
                $id
            }
        }

        impl $packet_name {
            pub(crate) fn new($($field : $field_type,)*) -> Self {
                Self {
                    $(
                        $field,
                    )*
                }
            }
        }

        #[async_trait]
        impl WriteProt for $packet_name {
            async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
                debug!("Outbound packet: {self:?}");
                VarInt::from(self.prot_size()).write(stream).await?;
                VarInt::from(Self::id() as usize).write(stream).await?;
                $(
                    self.$field.write(stream).await?;
                )*
                Ok(())
            }
        }
    };
}
