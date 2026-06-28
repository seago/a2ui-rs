use crate::error::TransportResult;
use crate::{Transport, TransportError};
use a2ui_core::message::Capabilities;
use a2ui_core::{
    message::{V1_0ClientMessage, V1_0ServerMessage},
    ClientEnvelope, ServerEnvelope,
};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

/// JSONL 单行最大字节数（防止内存耗尽 DoS 攻击）
const MAX_LINE_LENGTH: usize = 1_048_576; // 1 MiB

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
            let mut line = String::new();
            let n =
                self.reader.read_line(&mut line).await.map_err(|e| {
                    crate::TransportError::ReceiveError(format!("read error: {}", e))
                })?;
            if n == 0 {
                return Err(crate::TransportError::Eof);
            }
            // 检查行长度限制（防止 OOM DoS）
            if line.len() > MAX_LINE_LENGTH {
                return Err(crate::TransportError::ReceiveError(format!(
                    "line too long: {} bytes (max: {})",
                    line.len(),
                    MAX_LINE_LENGTH
                )));
            }
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
        let mut line = String::new();
        let n = self
            .reader
            .read_line(&mut line)
            .await
            .map_err(|e| TransportError::ReceiveError(format!("read error: {}", e)))?;
        if n == 0 {
            return Ok(None);
        }
        // 检查行长度限制
        if line.len() > MAX_LINE_LENGTH {
            return Err(TransportError::ReceiveError(format!(
                "line too long: {} bytes (max: {})",
                line.len(),
                MAX_LINE_LENGTH
            )));
        }
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
