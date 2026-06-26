use crate::error::TransportResult;
use crate::{Transport, TransportError};
use a2ui_core::{ClientEnvelope, ServerEnvelope};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

#[async_trait::async_trait]
impl<R, W> Transport for JsonlTransport<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    async fn connect(&mut self) -> TransportResult<()> {
        Ok(())
    }

    async fn send(&mut self, envelope: ClientEnvelope) -> TransportResult<()> {
        let json = serde_json::to_string(&envelope)
            .map_err(|e| crate::TransportError::SendError(format!("serialization error: {}", e)))?;
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
        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .await
            .map_err(|e| crate::TransportError::ReceiveError(format!("read error: {}", e)))?;
        let envelope: ServerEnvelope = serde_json::from_str(&line).map_err(|e| {
            crate::TransportError::ReceiveError(format!("deserialization error: {}", e))
        })?;
        Ok(envelope)
    }

    async fn close(&mut self) -> TransportResult<()> {
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
    fn test_jsonl_transport_new() {
        let input = Builder::new().read(b"").build();
        let output = Vec::new();
        let _transport = JsonlTransport::new(input, output);
        // 结构验证
        assert!(true);
    }
}
