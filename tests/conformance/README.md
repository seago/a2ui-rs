# A2UI 一致性测试向量（conformance vectors）

语言无关的协议一致性用例。每个 `*.json` 描述一串 `messages`（ServerEnvelope）依次
喂入协议核心后，最终的 `expect`（Data Model 与关键组件解析结果）。目的是让 Rust 与
TypeScript 两份实现消费**同一批**用例，防止双份逻辑走样。

## 用例格式

```jsonc
{
  "name": "人类可读的用例名",
  "surfaceId": "s1",              // 断言针对的 Surface（可省略，默认取首个 createSurface）
  "messages": [ /* ServerEnvelope[]：{version:"v1.0", <msgKey>:{...}} */ ],
  "expect": {
    "dataModel": { /* 断言最终整个 Data Model 深度相等 */ },
    "resolved": {                  // 断言各组件在【根作用域】解析后的 props 深度相等
      "<componentId>": { /* 期望 props */ }
    },
    "tree": { /* 断言展开后的渲染树（含 template + @index）归一化后深度相等 */ }
  }
}
```

归一化渲染树节点形如 `{ id, type, props, children[] }`，缺失引用附加 `placeholder: true`。
`updateDataModel` 的删除语义用**省略 `value` 字段**表达（显式 `null` 表示置空而非删除）。

TypeScript runner：`clients/web-react/src/core/conformance.test.ts`。
