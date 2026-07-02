/**
 * `formatString` 模板插值（Catalog 内置函数的最小实现）。
 *
 * 语法：模板中的 `{key}` 被 `values[key]` 替换（自动类型转换）；`{{`/`}}`
 * 转义为字面量花括号；未知 key 替换为空串。
 *
 * 与 Rust 渲染器一致，**返回原始文本、不做上下文转义**——渲染层（React/HTML）
 * 需按自身上下文自行转义。{@link htmlEscape} 提供 HTML 场景的转义工具。
 *
 * @example
 * ```ts
 * import { formatString } from "@/core";
 * formatString("你好，{name}！你有 {count} 条消息", { name: "张三", count: 3 });
 * // => "你好，张三！你有 3 条消息"
 * ```
 */
import type { Json } from "@/core/json-pointer";

/** 将 JSON 值转为显示字符串：字符串原样，数字/布尔转字面，null/undefined 为空串，复合类型 JSON 序列化。 */
export function valueToString(value: Json | undefined): string {
  if (value === null || value === undefined) return "";
  switch (typeof value) {
    case "string":
      return value;
    case "number":
    case "boolean":
      return String(value);
    default:
      return JSON.stringify(value);
  }
}

/**
 * 用 `values` 里的绑定插值模板中的 `{key}` 占位。
 * `{{` → `{`，`}}` → `}`；未知 key → 空串。
 */
export function formatString(
  template: string,
  values: Record<string, Json | undefined> = {},
): string {
  let out = "";
  for (let i = 0; i < template.length; i++) {
    const ch = template[i];
    if (ch === "{") {
      if (template[i + 1] === "{") {
        out += "{";
        i++;
        continue;
      }
      const end = template.indexOf("}", i + 1);
      if (end === -1) {
        // 未闭合，原样保留剩余文本
        out += template.slice(i);
        break;
      }
      const key = template.slice(i + 1, end).trim();
      out += valueToString(values[key]);
      i = end;
      continue;
    }
    if (ch === "}" && template[i + 1] === "}") {
      out += "}";
      i++;
      continue;
    }
    out += ch;
  }
  return out;
}

/**
 * HTML 上下文转义，防止 formatString 结果被当作标签注入。
 * `&` `<` `>` `"` `'` → 对应 HTML entity。
 */
export function htmlEscape(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}
