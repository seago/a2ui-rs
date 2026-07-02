// 共享契约桶文件。C / V / K 三条轨道统一从 `@/contracts` 引入接口与类型。
// 本目录仅含接口/类型（+ 少量常量），无业务逻辑；各轨道实现只依赖这里，互不直接耦合。

export * from "./protocol";
export * from "./store";
export * from "./kit";
