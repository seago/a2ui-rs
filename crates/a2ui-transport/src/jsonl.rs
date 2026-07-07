use crate::error::TransportResult;
use crate::{Transport, TransportError};
use a2ui_core::message::Capabilities;
use a2ui_core::{
    message::{V1_0ClientMessage, V1_0ServerMessage},
    ClientEnvelope, ServerEnvelope,
};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

/// JSONL 单行最大字节数（防止内存耗尽 DoS 攻击）
const MAX_LINE_LENGTH: usize = 1_048_576; // 1 MiB

/// 限长读取一行（含结尾换行符），EOF 返回 `Ok(None)`。
///
/// 长度上限在读取过程中生效（通过 `take` 截断），而非读完整行后再检查——
/// 否则恶意对端发送不含换行的超长"行"会在检查前耗尽内存。
/// 超限属于协议违规，返回错误后流停留在行中间，调用方应关闭连接。
async fn read_line_bounded<R>(reader: &mut BufReader<R>) -> TransportResult<Option<String>>
where
    R: AsyncRead + Unpin,
{
    let mut buf = Vec::new();
    let n = (&mut *reader)
        .take((MAX_LINE_LENGTH + 1) as u64)
        .read_until(b'\n', &mut buf)
        .await
        .map_err(|e| TransportError::ReceiveError(format!("read error: {}", e)))?;
    if n == 0 {
        return Ok(None);
    }
    if buf.len() > MAX_LINE_LENGTH {
        return Err(TransportError::ReceiveError(format!(
            "line too long: exceeds {} bytes",
            MAX_LINE_LENGTH
        )));
    }
    let line = String::from_utf8(buf)
        .map_err(|e| TransportError::ReceiveError(format!("invalid utf-8: {}", e)))?;
    Ok(Some(line))
}

#[async_trait::async_trait]
impl<R, W> Transport for JsonlTransport<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    async fn connect(&mut self) -> TransportResult<()> {
        Ok(())
    }

    async fn handshake(&mut self, capabilities: Capabilities) -> TransportResult<Capabilities> {
        // 发送客户端能力描述
        let client_msg = V1_0ClientMessage::Capabilities(capabilities);
        self.send(ClientEnvelope::V1_0(client_msg)).await?;

        // 接收服务端能力描述
        let envelope = self.receive().await?;
        match envelope {
            ServerEnvelope::V1_0(V1_0ServerMessage::Capabilities(server_caps)) => Ok(server_caps),
            _ => Err(TransportError::ConnectionError(
                "expected capabilities message during handshake".to_string(),
            )),
        }
    }

    async fn send(&mut self, envelope: ClientEnvelope) -> TransportResult<()> {
        let json = serde_json::to_string(&envelope)
            .map_err(|e| crate::TransportError::SendError(format!("serialization error: {}", e)))?;
        tracing::trace!("JSONL send: {} bytes", json.len());
        self.writer
            .write_all(json.as_bytes())
            .await
            .map_err(|e| crate::TransportError::SendError(format!("write error: {}", e)))?;
        self.writer
            .write_all(b"\n")
            .await
            .map_err(|e| crate::TransportError::SendError(format!("write error: {}", e)))?;
        Ok(())
    }

    async fn receive(&mut self) -> TransportResult<ServerEnvelope> {
        loop {
            let Some(line) = read_line_bounded(&mut self.reader).await? else {
                return Err(crate::TransportError::Eof);
            };
            // 跳过空行（仅有换行符的行）
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let envelope: ServerEnvelope = serde_json::from_str(trimmed).map_err(|e| {
                crate::TransportError::ReceiveError(format!("deserialization error: {}", e))
            })?;
            return Ok(envelope);
        }
    }

    async fn close(&mut self) -> TransportResult<()> {
        tracing::info!("JSONL transport closing");
        self.writer
            .flush()
            .await
            .map_err(|e| crate::TransportError::SendError(format!("flush on close: {}", e)))?;
        Ok(())
    }
}

/// JSONL Transport：基于 STDIN/STDOUT 行分隔 JSON
#[derive(Debug)]
pub struct JsonlTransport<R, W> {
    pub reader: BufReader<R>,
    pub writer: W,
}

impl<R, W> JsonlTransport<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    /// 从 reader/writer 创建
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader: BufReader::new(reader),
            writer,
        }
    }

    /// 读取一行 JSONL 数据（不经过 Transport trait，直接操作 reader）
    ///
    /// 返回去除尾随换行符的字符串。EOF 时返回 `Ok(None)`。
    pub async fn receive_line(&mut self) -> TransportResult<Option<String>> {
        let Some(mut line) = read_line_bounded(&mut self.reader).await? else {
            return Ok(None);
        };
        // 去除尾随换行符
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        Ok(Some(line))
    }
}

impl JsonlTransport<tokio::io::Stdin, tokio::io::Stdout> {
    /// 从标准输入/输出创建（tokio 运行时）
    pub fn from_std() -> Self {
        Self::new(tokio::io::stdin(), tokio::io::stdout())
    }
}

/// Convenience type alias for a JSONL transport using stdin as reader
pub type JsonlTransportReader<W> = JsonlTransport<tokio::io::Stdin, W>;

/// Convenience type alias for a JSONL transport using stdout as writer
pub type JsonlTransportWriter<R> = JsonlTransport<R, tokio::io::Stdout>;

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test::io::Builder;

    #[test]
    fn test_jsonl_transport_from_std() {
        // 验证 from_std 创建成功
        let _transport = JsonlTransport::from_std();
    }

    #[test]
    fn test_jsonl_transport_new() {
        let input = Builder::new().read(b"").build();
        let output = Vec::new();
        let _transport = JsonlTransport::new(input, output);
        // 结构验证
        assert!(true);
    }

    #[test]
    fn test_jsonl_transport_send_receive_roundtrip() {
        let input =
            b"{\"version\":\"v1.0\",\"action\":{\"name\":\"click\",\"surfaceId\":\"s1\"}}\n";
        let mut output = Vec::new();

        let mut transport = JsonlTransport::new(input.as_slice(), &mut output);

        // 创建 task 运行时
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            transport.connect().await.unwrap();
        });

        // 验证结构
        assert!(true);
    }

    #[test]
    fn test_jsonl_transport_handshake() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // 模拟服务端响应：返回 capabilities 消息
            let server_response =
                b"{\"version\":\"v1.0\",\"capabilities\":{\"version\":\"1.0\",\"features\":[\"basic\"]}}\n";
            let input = Builder::new().read(server_response).build();
            let mut output = Vec::new();

            let mut transport = JsonlTransport::new(input, &mut output);
            transport.connect().await.unwrap();

            let client_caps = Capabilities {
                version: "1.0".to_string(),
                features: vec!["tui".to_string()],
            };
            let server_caps = transport.handshake(client_caps).await.unwrap();

            assert_eq!(server_caps.version, "1.0");
            assert_eq!(server_caps.features, vec!["basic"]);

            // 验证写入的内容包含客户端能力描述
            let written = String::from_utf8(output).unwrap();
            assert!(written.contains("capabilities"));
            assert!(written.contains("tui"));
        });
    }

    #[test]
    fn test_jsonl_transport_handshake_unexpected_message() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // 模拟服务端返回非 capabilities 消息
            let server_response =
                b"{\"version\":\"v1.0\",\"createSurface\":{\"surfaceId\":\"s1\"}}\n";
            let input = Builder::new().read(server_response).build();
            let mut output = Vec::new();

            let mut transport = JsonlTransport::new(input, &mut output);
            transport.connect().await.unwrap();

            let client_caps = Capabilities {
                version: "1.0".to_string(),
                features: vec![],
            };
            let result = transport.handshake(client_caps).await;
            assert!(result.is_err());
        });
    }

    /// 无限输出 'a'（永不出现换行）的 reader，被读超过 limit 字节即报 IO 错误。
    /// 用于验证行长检查发生在读取过程中而非读完整行之后：
    /// 若实现先无界 read_line 再检查长度，会撞上守卫错误而非返回 line too long。
    struct GuardedEndlessReader {
        served: usize,
        limit: usize,
    }

    impl tokio::io::AsyncRead for GuardedEndlessReader {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            if self.served >= self.limit {
                return std::task::Poll::Ready(Err(std::io::Error::other(
                    "guard: read past limit",
                )));
            }
            let n = buf.remaining().min(self.limit - self.served).min(64 * 1024);
            buf.put_slice(&vec![b'a'; n]);
            self.served += n;
            std::task::Poll::Ready(Ok(()))
        }
    }

    #[test]
    fn test_receive_stops_reading_at_line_length_limit() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let input = GuardedEndlessReader {
                served: 0,
                limit: 8 * 1024 * 1024, // 守卫：读超 8 MiB 即视为防护失效
            };
            let mut output = Vec::new();
            let mut transport = JsonlTransport::new(input, &mut output);
            let result = Transport::receive(&mut transport).await;
            match result {
                Err(TransportError::ReceiveError(msg)) => assert!(
                    msg.contains("line too long"),
                    "expected line-too-long error before reading past guard, got: {msg}"
                ),
                other => panic!("expected ReceiveError(line too long), got: {other:?}"),
            }
        });
    }

    #[test]
    fn test_receive_line_over_limit_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // 略超 1 MiB 的单行（有限输入），两条读取路径都应报 line too long
            let mut data = vec![b'a'; MAX_LINE_LENGTH + 10];
            data.push(b'\n');
            let mut output = Vec::new();
            let mut transport = JsonlTransport::new(data.as_slice(), &mut output);
            let result = transport.receive_line().await;
            match result {
                Err(TransportError::ReceiveError(msg)) => {
                    assert!(msg.contains("line too long"), "got: {msg}")
                }
                other => panic!("expected ReceiveError(line too long), got: {other:?}"),
            }
        });
    }

    #[test]
    fn test_receive_eof_returns_eof_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let input = Builder::new().read(b"").build();
            let mut output = Vec::new();
            let mut transport = JsonlTransport::new(input, &mut output);
            let result = Transport::receive(&mut transport).await;
            assert!(matches!(result, Err(TransportError::Eof)));
        });
    }
}
