# a2ui-rs

A2UI (Agent to UI) Protocol v1.0 的 Rust 实现。

## 快速开始

### 安装

```bash
git clone https://github.com/yufeng108/a2ui-rs.git
cd a2ui-rs
```

### 构建

```bash
cargo build --workspace
```

### 运行测试

```bash
cargo test --workspace
```

### 使用 CLI

从 STDIN 读取 JSONL 流并渲染到终端：

```bash
echo '{"version":"v1.0","createSurface":{"surfaceId":"s1","catalogId":"basic","sendDataModel":false}}' | cargo run --bin a2ui -- render
```

## Workspace Crate 说明

| Crate | 职责 |
|-------|------|
| `a2ui-core` | 协议类型定义、消息枚举、JSON Schema 解析、状态机 |
| `a2ui-transport` | 传输层抽象 trait + JSONL/WebSocket 绑定 |
| `a2ui-renderer` | `Renderer` trait、组件树管理、路径解析、函数调度 |
| `a2ui-renderer-tui` | TUI 渲染器实现（ratatui） |
| `a2ui-renderer-gui` | GUI 渲染器（预留） |
| `a2ui-renderer-web` | Web 渲染器（预留） |
| `a2ui-cli` | 命令行入口 |

## 协议版本

当前实现：A2UI Protocol v1.0

详见 [ARCHITECTURE.md](ARCHITECTURE.md)。
