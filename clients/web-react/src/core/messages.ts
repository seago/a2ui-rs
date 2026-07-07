/**
 * 服务端信封的解析与分发（Server → Client）。
 *
 * 把收到的 JSON（字符串或已解析对象）转成类型化的 {@link ServerMessage}，
 * 对未知消息键、未知版本、缺字段做稳健处理——返回带错误信息的结果而非抛异常，
 * 便于渲染层跳过坏消息而不中断消息流。
 *
 * @example
 * ```ts
 * import { parseServerEnvelope } from "@/core";
 * const r = parseServerEnvelope('{"version":"v1.0","createSurface":{"surfaceId":"s1","catalogId":"basic"}}');
 * if (r.ok && r.message.kind === "createSurface") {
 *   r.message.message.surfaceId; // "s1"
 * }
 * ```
 */
import { PROTOCOL_VERSION } from "@/core/types";
import type {
  Component,
  Json,
  ServerMessage,
  AccessibilityAttributes,
} from "@/core/types";

/** 解析结果：成功携带类型化消息，失败携带错误说明。 */
export type ParseResult =
  | { ok: true; message: ServerMessage }
  | { ok: false; error: string };

const SERVER_KEYS = [
  "createSurface",
  "updateComponents",
  "updateDataModel",
  "deleteSurface",
  "actionResponse",
  "callFunction",
] as const;

function isObject(v: unknown): v is Record<string, Json> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function asString(v: unknown): string | undefined {
  return typeof v === "string" ? v : undefined;
}

/** 解析单个组件（邻接表节点）；缺 `id`/`component` 返回 `null`。 */
export function parseComponent(raw: unknown): Component | null {
  if (!isObject(raw)) return null;
  const id = asString(raw.id);
  const component = asString(raw.component);
  if (id === undefined || component === undefined) return null;

  const properties: Record<string, Json> = {};
  let accessibility: AccessibilityAttributes | undefined;
  let weight: number | undefined;
  for (const [key, value] of Object.entries(raw)) {
    if (key === "id" || key === "component") continue;
    if (key === "accessibility") {
      if (isObject(value)) {
        accessibility = {
          label: asString(value.label),
          description: asString(value.description),
        };
      }
      continue;
    }
    if (key === "weight") {
      if (typeof value === "number") weight = value;
      continue;
    }
    properties[key] = value as Json;
  }
  return { id, component, accessibility, weight, properties };
}

function parseComponents(raw: unknown): Component[] {
  if (!Array.isArray(raw)) return [];
  const out: Component[] = [];
  for (const item of raw) {
    const c = parseComponent(item);
    if (c) out.push(c);
  }
  return out;
}

/** 解析服务端信封。接受 JSON 字符串或已解析对象。 */
export function parseServerEnvelope(input: string | unknown): ParseResult {
  let envelope: unknown = input;
  if (typeof input === "string") {
    try {
      envelope = JSON.parse(input);
    } catch (e) {
      return { ok: false, error: `invalid JSON: ${(e as Error).message}` };
    }
  }
  if (!isObject(envelope)) {
    return { ok: false, error: "envelope must be a JSON object" };
  }
  if (envelope.version !== PROTOCOL_VERSION) {
    return { ok: false, error: `unsupported version: ${String(envelope.version)}` };
  }
  const present = SERVER_KEYS.filter((k) =>
    Object.prototype.hasOwnProperty.call(envelope, k),
  );
  if (present.length === 0) {
    return { ok: false, error: "no known server message key present" };
  }
  if (present.length > 1) {
    return { ok: false, error: `multiple message keys: ${present.join(", ")}` };
  }
  const key = present[0];
  const payload = (envelope as Record<string, unknown>)[key];
  if (!isObject(payload)) {
    return { ok: false, error: `${key} payload must be an object` };
  }
  return dispatch(key, payload, envelope);
}

function dispatch(
  key: (typeof SERVER_KEYS)[number],
  p: Record<string, Json>,
  envelope: Record<string, Json>,
): ParseResult {
  switch (key) {
    case "createSurface": {
      const surfaceId = asString(p.surfaceId);
      const catalogId = asString(p.catalogId);
      if (surfaceId === undefined || catalogId === undefined) {
        return { ok: false, error: "createSurface missing surfaceId/catalogId" };
      }
      return {
        ok: true,
        message: {
          kind: "createSurface",
          message: {
            surfaceId,
            catalogId,
            surfaceProperties: p.surfaceProperties,
            sendDataModel: p.sendDataModel === true,
            components: p.components ? parseComponents(p.components) : undefined,
            dataModel: p.dataModel,
          },
        },
      };
    }
    case "updateComponents": {
      const surfaceId = asString(p.surfaceId);
      if (surfaceId === undefined) {
        return { ok: false, error: "updateComponents missing surfaceId" };
      }
      return {
        ok: true,
        message: {
          kind: "updateComponents",
          message: { surfaceId, components: parseComponents(p.components) },
        },
      };
    }
    case "updateDataModel": {
      const surfaceId = asString(p.surfaceId);
      if (surfaceId === undefined) {
        return { ok: false, error: "updateDataModel missing surfaceId" };
      }
      const hasValue = Object.prototype.hasOwnProperty.call(p, "value");
      return {
        ok: true,
        message: {
          kind: "updateDataModel",
          message: {
            surfaceId,
            path: asString(p.path),
            value: hasValue ? p.value : undefined,
            hasValue,
          },
        },
      };
    }
    case "deleteSurface": {
      const surfaceId = asString(p.surfaceId);
      if (surfaceId === undefined) {
        return { ok: false, error: "deleteSurface missing surfaceId" };
      }
      return {
        ok: true,
        message: { kind: "deleteSurface", message: { surfaceId } },
      };
    }
    case "actionResponse": {
      // 规范：actionId 在信封层（与 actionResponse 键平级），payload 内只有
      // value（成功）或 error{code,message}（失败）恰含其一。
      const actionId = asString(envelope.actionId);
      if (actionId === undefined) {
        return {
          ok: false,
          error: "actionResponse missing envelope-level actionId",
        };
      }
      const hasValue = Object.prototype.hasOwnProperty.call(p, "value");
      const hasError = Object.prototype.hasOwnProperty.call(p, "error");
      if (hasValue === hasError) {
        return {
          ok: false,
          error: "actionResponse must contain exactly one of value/error",
        };
      }
      const errObj = isObject(p.error) ? p.error : undefined;
      if (hasError && !errObj) {
        return { ok: false, error: "actionResponse error must be an object" };
      }
      return {
        ok: true,
        message: {
          kind: "actionResponse",
          message: {
            actionId,
            value: errObj ? undefined : p.value,
            error: errObj
              ? {
                  code: asString(errObj.code) ?? "",
                  message: asString(errObj.message) ?? "",
                }
              : undefined,
          },
        },
      };
    }
    case "callFunction": {
      const functionCallId = asString(p.functionCallId);
      const call = asString(p.call);
      if (functionCallId === undefined || call === undefined) {
        return { ok: false, error: "callFunction missing functionCallId/call" };
      }
      return {
        ok: true,
        message: {
          kind: "callFunction",
          message: {
            functionCallId,
            wantResponse: p.wantResponse === true,
            call,
            args: p.args,
          },
        },
      };
    }
  }
}
