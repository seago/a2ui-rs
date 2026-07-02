/**
 * A2uiEngine：多 Surface 的协议核心入口（纯 TS，无 React）。
 *
 * 把收到的服务端信封 {@link ingest} 解析并派发到对应 {@link Surface}，管理 Surface
 * 生命周期、跨消息的 action 响应回写（`responsePath`）、以及客户端函数注册表
 * （`callFunction` 的执行边界校验在此闭环）。
 *
 * @example
 * ```ts
 * import { A2uiEngine } from "@/core";
 * const engine = new A2uiEngine();
 * engine.ingest({ version: "v1.0", createSurface: {
 *   surfaceId: "s1", catalogId: "basic",
 *   components: [{ id: "root", component: "Text", text: "hi" }],
 *   dataModel: {},
 * }});
 * engine.getSurface("s1")?.getRenderTree();
 * ```
 */
import { parseServerEnvelope } from "@/core/messages";
import { FunctionDispatcher } from "@/core/functions";
import { Surface, type ActionDispatch } from "@/core/surface";
import type { ClientEnvelope, Json } from "@/core/types";

/** ingest 的结果。`reply` 为需要回传服务端的客户端信封（functionResponse / error）。 */
export interface IngestResult {
  ok: boolean;
  error?: string;
  surfaceId?: string;
  reply?: ClientEnvelope;
}

interface PendingAction {
  surfaceId: string;
  responsePath?: string;
}

export class A2uiEngine {
  private readonly surfaces = new Map<string, Surface>();
  private readonly dispatcher: FunctionDispatcher;
  private readonly pending = new Map<string, PendingAction>();

  /** 传入自定义函数调度器以扩展客户端函数注册表；默认含内置函数。 */
  constructor(dispatcher: FunctionDispatcher = new FunctionDispatcher()) {
    this.dispatcher = dispatcher;
  }

  /** 客户端函数注册表（供上层注册自定义函数，供 callFunction 校验）。 */
  get functions(): FunctionDispatcher {
    return this.dispatcher;
  }

  /** 取某 Surface（含已 deleted 的，便于查询状态）。 */
  getSurface(surfaceId: string): Surface | undefined {
    return this.surfaces.get(surfaceId);
  }

  /** 当前所有 Surface id。 */
  surfaceIds(): string[] {
    return [...this.surfaces.keys()];
  }

  /**
   * 喂入一条服务端信封（JSON 字符串或对象），解析并派发。
   * 解析失败或语义错误时返回 `{ ok: false, error }`，不抛异常。
   */
  ingest(envelope: string | unknown): IngestResult {
    const parsed = parseServerEnvelope(envelope);
    if (!parsed.ok) return { ok: false, error: parsed.error };
    const msg = parsed.message;

    switch (msg.kind) {
      case "createSurface": {
        const m = msg.message;
        const surface = new Surface(m.surfaceId, m.catalogId, {
          sendDataModel: m.sendDataModel,
          dataModel: m.dataModel ?? {},
          dispatcher: this.dispatcher,
        });
        if (m.components) surface.upsertComponents(m.components);
        surface.activate();
        this.surfaces.set(m.surfaceId, surface);
        return { ok: true, surfaceId: m.surfaceId };
      }
      case "updateComponents": {
        const s = this.surfaces.get(msg.message.surfaceId);
        if (!s) return this.noSurface(msg.message.surfaceId);
        s.upsertComponents(msg.message.components);
        return { ok: true, surfaceId: msg.message.surfaceId };
      }
      case "updateDataModel": {
        const m = msg.message;
        const s = this.surfaces.get(m.surfaceId);
        if (!s) return this.noSurface(m.surfaceId);
        s.applyDataModel(m.path, m.hasValue, m.value);
        return { ok: true, surfaceId: m.surfaceId };
      }
      case "deleteSurface": {
        const s = this.surfaces.get(msg.message.surfaceId);
        if (!s) return this.noSurface(msg.message.surfaceId);
        s.markDeleted();
        return { ok: true, surfaceId: msg.message.surfaceId };
      }
      case "actionResponse": {
        const m = msg.message;
        const pending = this.pending.get(m.actionId);
        this.pending.delete(m.actionId);
        if (pending && pending.responsePath !== undefined && m.error === undefined) {
          const s = this.surfaces.get(pending.surfaceId);
          if (s) s.setDataValue(pending.responsePath, m.value ?? null);
        }
        return { ok: true, surfaceId: pending?.surfaceId };
      }
      case "callFunction": {
        return this.handleCallFunction(
          msg.message.functionCallId,
          msg.message.call,
          msg.message.wantResponse,
          msg.message.args,
        );
      }
    }
  }

  /**
   * 为某组件的 event action 生成回传信封，并登记 actionId → responsePath 以便
   * 后续 actionResponse 写回。返回 undefined 表示该组件无可回传的 event action。
   */
  buildActionMessage(
    surfaceId: string,
    componentId: string,
  ): ActionDispatch | undefined {
    const s = this.surfaces.get(surfaceId);
    if (!s) return undefined;
    const dispatch = s.buildActionMessage(componentId);
    if (dispatch && "action" in dispatch.envelope) {
      const a = dispatch.envelope.action;
      if (a.wantResponse && a.actionId) {
        this.pending.set(a.actionId, {
          surfaceId,
          responsePath: a.responsePath,
        });
      }
    }
    return dispatch;
  }

  private handleCallFunction(
    functionCallId: string,
    call: string,
    wantResponse: boolean,
    args: Json | undefined,
  ): IngestResult {
    // 未注册或边界不允许（remote 调 clientOnly）→ 拒绝并回传 error。
    if (!this.dispatcher.canCall(call, "remote")) {
      const err = {
        code: "INVALID_FUNCTION_CALL",
        message: `function not callable from remote: ${call}`,
        functionCallId,
      };
      const reply: ClientEnvelope = { version: "v1.0", error: err };
      return { ok: false, error: err.message, reply };
    }
    const argObj =
      typeof args === "object" && args !== null && !Array.isArray(args)
        ? (args as Record<string, Json>)
        : {};
    const value = this.dispatcher.dispatch(call, argObj, "remote");
    if (!wantResponse) return { ok: true };
    return {
      ok: true,
      reply: {
        version: "v1.0",
        functionResponse: { functionCallId, call, value },
      },
    };
  }

  private noSurface(surfaceId: string): IngestResult {
    return { ok: false, error: `surface not found: ${surfaceId}`, surfaceId };
  }
}
