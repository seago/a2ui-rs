# CLAUDE.md

本文件为 Claude Code（claude.ai/code）提供本仓库的协作指引。

## 用户偏好

- 全程使用中文进行对话
- 文档内容使用中文语言

## 项目

`a2ui-rs` 是 **A2UI (Agent to UI) Protocol v1.0** 的 Rust 实现。协议规范见 [a2ui.org](https://a2ui.org/specification/v1.0-a2ui/)，架构设计见 [ARCHITECTURE.md](ARCHITECTURE.md)。采用 Cargo workspace 组织多个 crate。

## 常用命令

```bash
# 编译所有成员
cargo build --workspace

# 编译指定成员
cargo build -p <package_name>

# 运行所有成员的测试
cargo test --workspace

# 运行指定成员的测试
cargo test -p <package_name>

# 运行单个测试
cargo test -p <package_name> <test_name>

# 格式化与静态检查
cargo fmt && cargo clippy --workspace

# 运行示例
cargo run --example <name>
```

## 强制要求

- **必须采用 TDD（测试驱动开发）模式**，严格遵守「红 → 绿 → 重构」循环：
  1. **红**：先写一个失败的测试，明确描述期望行为。
  2. **绿**：用最少的代码让测试通过（不追求完美，只求通过）。
  3. **重构**：在测试始终通过的前提下优化代码结构和设计。
- 任何新功能或 bug 修复，**不允许先写实现代码再补测试**。
- 每个测试应当独立、可重复，不依赖执行顺序。
- 提交前确认 `cargo test --workspace` 全部通过。

### Rust 测试分层

- **单元测试**（`#[cfg(test)]`）：放在 `src/` 源码文件末尾，测试单个函数或模块的内部逻辑。覆盖率目标：核心逻辑 100%。
- **集成测试**（`tests/` 目录）：从 crate 外部调用公开 API，验证模块间的交互。每个文件对应一个集成测试套件。
- **文档测试**（`///` 中的 ` ```rust ` 代码块）：确保文档中的示例始终可编译运行，`cargo test` 会自动执行。

### TDD 开发流程

1. 确定本次迭代要完成的最小功能点。
2. 在对应的测试文件（单元/集成）中编写一个测试，描述预期行为，此时运行会失败。
3. 切换到实现文件，编写最少代码使该测试通过。
4. 运行 `cargo test -p <package_name> <test_name>` 确认单个测试通过。
5. 运行 `cargo test --workspace` 确认没有回归。
6. 重构：优化实现代码，保持测试全部通过。
7. 提交。

## 架构约束

详见 [ARCHITECTURE.md](ARCHITECTURE.md)。以下为实施时必须遵守的约束：

- `a2ui-transport` 只负责消息收发和会话管理，不包含任何渲染逻辑。
- `a2ui-renderer` 定义 `Renderer` trait，具体渲染 API 由各平台 crate 实现，不向上暴露。
- `a2ui-core` 是唯一依赖 `serde_json` 的 crate，下游只依赖 `a2ui-core` 的 Rust 类型，不直接处理 JSON。
- Surface 生命周期由状态机管理：`createSurface` → 活跃 → `deleteSurface`，状态转换有严格顺序约束。

## 代码偏好

- 错误处理用 `thiserror` 定义 crate 级错误类型，禁止裸 `panic!` 或 `unwrap()` 在业务逻辑中。
- 异步接口统一用 `async fn`，即使当前没有 await。
- 序列化/反序列化统一用 `serde` + `serde_json`。
- 所有公共 API 必须有文档注释（`///`），且包含至少一个可运行的示例。

## 禁止事项

- 永远不要修改 `a2ui-core` 中的消息类型定义而不更新所有下游渲染器的测试。
- 不要删除任何函数或类型，即使你认为没有被调用——可能有动态引用或插件加载。
- 不要在 `a2ui-core` 中引入与 A2UI 协议规范冲突的类型定义。
- 不要自动添加新的依赖，先确认需要哪个库、为什么需要。
- 不要混入功能改动和 `cargo fmt` 格式化改动到同一个 commit。
- 不要在测试中使用 `#[ignore]` 跳过失败测试而不说明原因。

## 工作方式

- 任何会影响超过 3 个文件的改动，先列计划确认再执行。
- 遇到不确定的需求，停下来问，不要自己猜。
- 每完成一个有意义的改动，自动运行 `cargo test --workspace` 验证没有引入回归。
- 发现潜在的 bug 或改进点，可以提出来，但不要自行修改。
