# Task 1 Report: 修复 TUI render() 调用链

## 实现内容

1. **在 `run_render()` 中创建 ratatui Terminal**：使用 `CrosstermBackend::new(std::io::stderr())` 创建终端后端，再创建 `ratatui::Terminal`。
2. **修改 `process_server_envelope()` 签名**：新增 `terminal: &mut ratatui::Terminal<impl ratatui::backend::Backend>` 参数。
3. **替换渲染调用**：将 `renderer.render().await?`（stub，返回 `Ok(())`）替换为 `renderer.render_frame(terminal).await?`，实际执行帧绘制。
4. **更新调用处**：消息循环中传入 `&mut terminal`。
5. **添加 ratatui 依赖**：在 `a2ui-cli/Cargo.toml` 中添加 `ratatui = "0.26"` 以支持 `TestBackend` 测试。
6. **编写集成测试**：`test_process_server_envelope_calls_render_frame` 使用 `TestBackend` 验证 `render_frame` 被实际调用。

## TDD 证据

### RED 阶段（测试失败）

```bash
$ cargo test -p a2ui-cli test_process_server_envelope_calls_render_frame 2>&1
error[E0425]: cannot find value `terminal` in this scope
error[E0061]: this function takes 2 arguments but 3 arguments were supplied
error: could not compile `a2ui-cli` (bin "a2ui" test) due to 2 previous errors
```

`process_server_envelope` 不接受 `terminal` 参数，且函数体内 `terminal` 未定义。

### GREEN 阶段（测试通过）

```bash
$ cargo test -p a2ui-cli test_process_server_envelope_calls_render_frame 2>&1
running 1 test
test tests::test_process_server_envelope_calls_render_frame ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured
```

## 变更文件

- `crates/a2ui-cli/Cargo.toml` — 添加 `ratatui = "0.26"` 依赖
- `crates/a2ui-cli/src/main.rs` — 创建 Terminal、修改函数签名、替换 render 调用、添加测试

## 测试总结

```
cargo test --workspace 结果：
- a2ui-cli:      1 passed (新增测试)
- a2ui-core:    102 单元 + 6 集成 = 108 passed
- a2ui-renderer: 75 passed
- a2ui-renderer-gui: 6 单元 + 3 e2e = 9 passed
- a2ui-renderer-tui: 50 单元 + 3 e2e = 53 passed
- a2ui-transport: 5 passed
- 文档测试:      1 passed

总计: 252 passed, 0 failed
输出 pristine（仅预存在的 warning，无新 warning/error）
```

## 自查发现

- `a2ui-renderer-tui/src/widget_builder.rs:48` 存在已有的 `binding` 字段未使用 warning（非本次引入）。
- `renderer.render()` trait 方法仍为 stub（返回 `Ok(())`），后续可考虑标记为 deprecated 或移除。

## 关注事项

无。所有测试通过，行为符合任务 brief 要求。

## Fix Report

### 问题

Reviewer 发现测试被放置在 `main.rs` 的 `#[cfg(test)] mod tests` 中作为单元测试，而 brief 明确要求作为集成测试放在 `crates/a2ui-cli/tests/` 目录下。

### 修复

1. **提取共享函数到 lib**：由于 `a2ui-cli` 原本是 binary-only crate，集成测试无法通过 `crate::` 访问 `main.rs` 中的函数。修复方案为添加 `src/lib.rs` 将 `process_server_envelope` 导出为 public，`main.rs` 通过 `use a2ui_cli::process_server_envelope` 调用。

2. **移除单元测试块**：从 `main.rs` 末尾删除 `#[cfg(test)] mod tests` 块（原 lines 223-263）。

3. **创建集成测试**：`crates/a2ui-cli/tests/render_chain.rs`，使用 `TestBackend` 验证 `render_frame` 被调用。

### 测试结果

```bash
$ cargo test -p a2ui-cli test_process_server_envelope_calls_render_frame 2>&1
running 1 test
test test_process_server_envelope_calls_render_frame ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured
```

```bash
$ cargo test --workspace 2>&1
test result: ok. 246 passed; 0 failed; 0 ignored; 0 measured
```

### 变更文件

- `crates/a2ui-cli/src/lib.rs` — 新增，导出 `process_server_envelope`
- `crates/a2ui-cli/src/main.rs` — 移除函数定义和单元测试块，改为从 lib 导入
- `crates/a2ui-cli/tests/render_chain.rs` — 新增集成测试
