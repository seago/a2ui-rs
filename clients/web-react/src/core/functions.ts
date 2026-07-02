/**
 * 函数调度器（最小集）与执行边界（`callableFrom`）校验。
 *
 * 对齐协议安全约束：`callFunction`（remote）调用 `clientOnly` 函数或调用未注册
 * 函数一律拒绝并抛 {@link FunctionError}（code `INVALID_FUNCTION_CALL`）。
 * 渲染期本地解析（client）可调用 `clientOnly` / `clientOrRemote`。
 *
 * 内置：`required`（非空校验，clientOnly）、`formatString`（模板插值，clientOrRemote）。
 */
import { formatString } from "@/core/format-string";
import type { Json } from "@/core/json-pointer";

/** 函数的执行边界。 */
export type CallableFrom = "clientOnly" | "remoteOnly" | "clientOrRemote";

/** 调用发起方：`client`（本地渲染解析）或 `remote`（服务端 callFunction）。 */
export type CallSite = "client" | "remote";

/** 已注册函数的处理器：接收（已解析的）参数对象，返回 JSON 值。 */
export type FunctionHandler = (args: Record<string, Json>) => Json;

/** 函数调度错误（未注册 / 边界不允许）。 */
export class FunctionError extends Error {
  code: string;
  constructor(message: string, code = "INVALID_FUNCTION_CALL") {
    super(message);
    this.name = "FunctionError";
    this.code = code;
  }
}

interface Entry {
  callableFrom: CallableFrom;
  handler: FunctionHandler;
}

function boundaryAllows(from: CallableFrom, site: CallSite): boolean {
  switch (from) {
    case "clientOnly":
      return site === "client";
    case "remoteOnly":
      return site === "remote";
    case "clientOrRemote":
      return true;
  }
}

export class FunctionDispatcher {
  private registry = new Map<string, Entry>();

  /** 创建调度器；`withBuiltins` 为真（默认）时注册内置函数。 */
  constructor(withBuiltins = true) {
    if (withBuiltins) this.registerBuiltins();
  }

  /** 注册（或覆盖）一个函数。 */
  register(
    name: string,
    callableFrom: CallableFrom,
    handler: FunctionHandler,
  ): void {
    this.registry.set(name, { callableFrom, handler });
  }

  /** 该函数是否已注册。 */
  has(name: string): boolean {
    return this.registry.has(name);
  }

  /** 在指定发起方下该函数是否可调用（未注册或边界不允许均为 false）。 */
  canCall(name: string, site: CallSite): boolean {
    const e = this.registry.get(name);
    return e !== undefined && boundaryAllows(e.callableFrom, site);
  }

  /**
   * 调度调用。
   * @throws {@link FunctionError} 未注册或执行边界不允许。
   */
  dispatch(
    name: string,
    args: Record<string, Json>,
    site: CallSite,
  ): Json {
    const e = this.registry.get(name);
    if (!e) {
      throw new FunctionError(`function not registered: ${name}`);
    }
    if (!boundaryAllows(e.callableFrom, site)) {
      throw new FunctionError(
        `function '${name}' not callable from ${site} (callableFrom=${e.callableFrom})`,
      );
    }
    return e.handler(args);
  }

  private registerBuiltins(): void {
    // required：非空校验。空串 / null / undefined / 空数组 视为不满足。
    this.register("required", "clientOnly", (args) => {
      const v = args.value;
      if (v === null || v === undefined) return false;
      if (typeof v === "string") return v.length > 0;
      if (Array.isArray(v)) return v.length > 0;
      return true;
    });

    // formatString：模板插值。args = { template, bindings? }，bindings 为已解析的值映射。
    this.register("formatString", "clientOrRemote", (args) => {
      const template = typeof args.template === "string" ? args.template : "";
      const bindings =
        typeof args.bindings === "object" &&
        args.bindings !== null &&
        !Array.isArray(args.bindings)
          ? (args.bindings as Record<string, Json>)
          : {};
      return formatString(template, bindings);
    });
  }
}
