/**
 * Data Model：组件绑定的纯 JSON 数据，是 Surface 的单一数据源。
 *
 * 对齐 Rust `a2ui-core` 的 `DataModel`：基于 RFC 6901 JSON Pointer 的
 * upsert / 删除 / 读取，含路径边界防护。渲染层通过
 * {@link "@/core/path-resolver".PathResolver} 在其上做作用域解析。
 *
 * @example
 * ```ts
 * import { DataModel } from "@/core";
 * const dm = new DataModel({ form: { name: "" } });
 * dm.applyPointer("/form/name", "张三");
 * dm.resolvePointer("/form/name"); // => "张三"
 * ```
 */
import {
  applyPointer,
  isRootPointer,
  resolvePointer,
  validatePointer,
  type Json,
} from "@/core/json-pointer";

/** 一次 Data Model 变更的描述，供依赖图反查受影响组件。 */
export interface DataModelChange {
  /** 发生变更的绝对 JSON Pointer（根替换为 `"/"`）。 */
  path: string;
  /** 是否为删除操作（`value` 省略/undefined）。 */
  deleted: boolean;
}

export class DataModel {
  private root: Json;

  /** 用初始 JSON 值创建（默认空对象）。传入值不做深拷贝。 */
  constructor(initial: Json = {}) {
    this.root = initial;
  }

  /** 获取整个 Data Model 的 JSON 值（只读语义，请勿直接改写）。 */
  get value(): Json {
    return this.root;
  }

  /**
   * 读取 JSON Pointer 路径的值；不存在或路径非法返回 `undefined`。
   * 根指针（`""` / `/`）返回整个 model。
   */
  resolvePointer(pointer: string): Json | undefined {
    return resolvePointer(this.root, pointer);
  }

  /**
   * upsert 或删除某路径。
   *
   * - 传入 `value`（含显式 `null`）→ 设置该值（存在则更新，不存在则创建）。
   * - 省略 `value`（`arguments.length < 2`）→ 删除该路径。
   * - 根指针 → 有值时替换整个 model，无值时清空为 `{}`。
   *
   * @returns 本次变更描述，供响应性反查。
   * @throws {@link "@/core/json-pointer".PointerError} 路径非法（含 null 字节 / 空段 / `..`）。
   */
  applyPointer(pointer: string, value?: Json): DataModelChange {
    const hasValue = arguments.length >= 2 && value !== undefined;
    this.root = applyPointer(this.root, pointer, hasValue, value);
    return {
      path: isRootPointer(pointer) ? "/" : pointer,
      deleted: !hasValue,
    };
  }

  /** 删除某路径（等价于省略 value 的 {@link applyPointer}）。 */
  deletePointer(pointer: string): DataModelChange {
    this.root = applyPointer(this.root, pointer, false);
    return { path: isRootPointer(pointer) ? "/" : pointer, deleted: true };
  }

  /** 校验路径是否合法（非法抛出）。用于在写入前显式防护。 */
  validate(pointer: string): void {
    validatePointer(pointer);
  }
}
