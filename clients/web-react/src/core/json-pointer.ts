/**
 * RFC 6901 JSON Pointer 工具函数。
 *
 * 提供 token 转义/反转义、路径安全校验、以及在任意 JSON 值上按指针
 * 读取（{@link resolvePointer}）与 upsert/删除（{@link applyPointer}）。
 *
 * 这些函数是纯函数式的底座，{@link "@/core/data-model".DataModel} 在其上封装
 * 有状态的 Data Model。语义严格对齐 Rust `a2ui-core` 的
 * `datamodel::model::DataModel`（见仓库 `crates/a2ui-core/src/datamodel/model.rs`）。
 */

/** JSON 值类型（Data Model 内部使用）。 */
export type Json =
  | null
  | boolean
  | number
  | string
  | Json[]
  | { [key: string]: Json };

/** 非法 JSON Pointer（含 null 字节、空段、`..` 逃逸等）时抛出的错误。 */
export class PointerError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PointerError";
  }
}

/** 反转义单个 token：`~1` → `/`，`~0` → `~`（顺序敏感）。 */
export function unescapeToken(token: string): string {
  return token.replace(/~1/g, "/").replace(/~0/g, "~");
}

/** 转义单个 token：`~` → `~0`，`/` → `~1`（顺序敏感）。 */
export function escapeToken(token: string): string {
  return token.replace(/~/g, "~0").replace(/\//g, "~1");
}

/** 判断指针是否指向整个文档根（空串或 `/`，协议约定替换整个 model）。 */
export function isRootPointer(pointer: string): boolean {
  return pointer === "" || pointer === "/";
}

/**
 * 校验 JSON Pointer 的安全性；非法时抛出 {@link PointerError}。
 *
 * 拒绝：null 字节、非根却不以 `/` 开头、空路径段（`//`）、`..` 遍历段。
 * 根指针（`""` / `/`）视为合法。
 */
export function validatePointer(pointer: string): void {
  if (pointer.includes("\0")) {
    throw new PointerError(`path contains null byte: ${JSON.stringify(pointer)}`);
  }
  if (isRootPointer(pointer)) return;
  if (!pointer.startsWith("/")) {
    throw new PointerError(`pointer must start with '/': ${pointer}`);
  }
  for (const raw of pointer.slice(1).split("/")) {
    if (raw === "") {
      throw new PointerError(`empty path segment in: ${pointer}`);
    }
    if (raw === "..") {
      throw new PointerError(`path traversal detected: ${pointer}`);
    }
  }
}

/** 将 token 解析为数组索引；非法（空、前导零、非数字）返回 null。 */
export function parseArrayIndex(token: string): number | null {
  if (token.length === 0) return null;
  if (token.length > 1 && token.startsWith("0")) return null;
  if (!/^[0-9]+$/.test(token)) return null;
  const n = Number.parseInt(token, 10);
  return Number.isSafeInteger(n) ? n : null;
}

function isPlainObject(v: Json | undefined): v is { [key: string]: Json } {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/**
 * 读取指针指向的值；不存在或路径非法返回 `undefined`。
 *
 * 根指针返回整个 `root`。不会抛出——非法路径按「未命中」处理，便于渲染层
 * 静默降级。
 */
export function resolvePointer(root: Json, pointer: string): Json | undefined {
  if (isRootPointer(pointer)) return root;
  try {
    validatePointer(pointer);
  } catch {
    return undefined;
  }
  let cur: Json | undefined = root;
  for (const raw of pointer.slice(1).split("/")) {
    const token = unescapeToken(raw);
    if (Array.isArray(cur)) {
      const idx = parseArrayIndex(token);
      if (idx === null || idx >= cur.length) return undefined;
      cur = cur[idx];
    } else if (isPlainObject(cur)) {
      if (!Object.prototype.hasOwnProperty.call(cur, token)) return undefined;
      cur = cur[token];
    } else {
      return undefined;
    }
  }
  return cur;
}

/**
 * 在 `root` 上应用一次 upsert / 删除，返回新的根值。
 *
 * - 根指针（`""` / `/`）：`hasValue` 为真时用 `value` 替换整个文档，否则清空为 `{}`。
 * - `hasValue` 为真：存在则更新，不存在则创建（含中间节点，按下一段是否为数组索引
 *   决定创建对象还是数组）。
 * - `hasValue` 为假：删除该路径（对象删 key，数组按索引 splice）。
 *
 * 非法路径抛出 {@link PointerError}。除根替换外原地修改并返回同一引用。
 */
export function applyPointer(
  root: Json,
  pointer: string,
  hasValue: boolean,
  value?: Json,
): Json {
  if (isRootPointer(pointer)) {
    return hasValue ? (value as Json) : {};
  }
  validatePointer(pointer);
  const tokens = pointer.slice(1).split("/").map(unescapeToken);

  if (!hasValue) {
    deleteAt(root, tokens);
    return root;
  }
  createPath(root, tokens, value as Json);
  return root;
}

function deleteAt(root: Json, tokens: string[]): void {
  let cur: Json | undefined = root;
  for (let i = 0; i < tokens.length - 1; i++) {
    const token = tokens[i];
    if (Array.isArray(cur)) {
      const idx = parseArrayIndex(token);
      cur = idx === null ? undefined : cur[idx];
    } else if (isPlainObject(cur)) {
      cur = cur[token];
    } else {
      return;
    }
    if (cur === undefined) return;
  }
  const last = tokens[tokens.length - 1];
  if (Array.isArray(cur)) {
    const idx = parseArrayIndex(last);
    if (idx !== null && idx < cur.length) cur.splice(idx, 1);
  } else if (isPlainObject(cur)) {
    delete cur[last];
  }
}

function createPath(root: Json, tokens: string[], value: Json): void {
  let cur: Json = root;
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    const isLast = i === tokens.length - 1;
    if (isLast) {
      if (Array.isArray(cur)) {
        const idx = parseArrayIndex(token);
        if (idx !== null && idx <= cur.length) cur[idx] = value;
      } else if (isPlainObject(cur)) {
        cur[token] = value;
      }
      return;
    }
    // 中间段：确保存在
    if (Array.isArray(cur)) {
      const idx = parseArrayIndex(token);
      if (idx === null || idx >= cur.length) return;
      cur = cur[idx];
    } else if (isPlainObject(cur)) {
      if (!Object.prototype.hasOwnProperty.call(cur, token)) {
        const nextIsIndex = parseArrayIndex(tokens[i + 1]) !== null;
        cur[token] = nextIsIndex ? [] : {};
      }
      cur = cur[token];
    } else {
      return;
    }
  }
}
