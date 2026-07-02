//! 语言无关一致性向量（conformance vectors）的 **Rust 侧 runner**。
//!
//! 消费与 TypeScript 前端**同一批**共享测试向量
//! （仓库根 `tests/conformance/*.json`），把每个用例的 `messages`
//! 依次喂入用 a2ui-core / a2ui-renderer 公开 API 组装出的最小引擎，
//! 再断言解析结果与 `expect` 一致。TS 半边见
//! `clients/web-react/src/core/store-conformance.test.ts`。
//!
//! 这是「防 Rust / TS 两份逻辑走样」闸门的 Rust 半边：若某个向量在
//! Rust 侧无法通过，即暴露出两份实现之间的漂移。
//!
//! # 放置理由
//!
//! 建树、集合作用域、DynamicValue / formatString 解析等逻辑都在
//! `a2ui-renderer` 层（`ComponentForest` / `PathResolver` /
//! `FunctionDispatcher` / `FormatString`），因此 runner 放在
//! `a2ui-renderer` 的集成测试目录，从 crate 外部只用公开 API 组装引擎。
//! 数据模型的 upsert / 删除语义来自 `a2ui-core::DataModel`。

use a2ui_core::prelude::*;
use a2ui_renderer::component_forest::ComponentForest;
use a2ui_renderer::{DataBinding, FunctionDispatcher, PathResolver};
use serde_json::{Map, Value};
use std::error::Error;
use std::fs;
use std::path::PathBuf;

type BoxError = Box<dyn Error>;

// --------------------------------------------------------------------------
// 向量定位：从本 crate 的 CARGO_MANIFEST_DIR 上溯到 workspace 根。
// crates/a2ui-renderer -> crates -> <root>/tests/conformance
// --------------------------------------------------------------------------

fn conformance_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent() // crates/
        .and_then(|p| p.parent()) // <root>/
        .expect("crate manifest should have <root>/crates/<crate> layout")
        .join("tests")
        .join("conformance")
}

fn load_cases() -> Vec<(String, Value)> {
    let dir = conformance_dir();
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read conformance dir {}: {}", dir.display(), e))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    files.sort();

    files
        .into_iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let text = fs::read_to_string(&p).unwrap();
            let json: Value = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("invalid JSON in {}: {}", name, e));
            (name, json)
        })
        .collect()
}

// --------------------------------------------------------------------------
// 最小引擎：用公开 API 组装。
//   - ComponentForest 存组件 / 建树 / 展开模板（a2ui-renderer 公开 API）
//   - DataModel       存数据模型 / upsert / 删除（a2ui-core 公开 API）
// --------------------------------------------------------------------------

struct Engine {
    forest: ComponentForest,
    data_model: DataModel,
}

impl Engine {
    fn new() -> Self {
        Self {
            forest: ComponentForest::new(),
            data_model: DataModel::empty(),
        }
    }

    /// 依次消费一条 ServerEnvelope（以原始 JSON 形态传入，
    /// 因为 updateDataModel 的「省略 value = 删除」语义在
    /// a2ui-core 的 `Option<Value>` 结构上会丢失，需要读原始键）。
    fn ingest(&mut self, envelope: &Value) -> std::result::Result<(), BoxError> {
        // 通过 a2ui-core 公开 API 反序列化，验证 envelope 合法。
        let parsed = ServerEnvelope::from_json(&envelope.to_string())?;

        match parsed {
            ServerEnvelope::V1_0(V1_0ServerMessage::CreateSurface(cs)) => {
                let sid = cs.surface_id.clone();
                if let Some(dm) = cs.data_model.clone() {
                    self.data_model = DataModel::new(dm);
                }
                if let Some(components) = cs.components.clone() {
                    for c in components {
                        self.forest.upsert(&sid, c)?;
                    }
                }
                Ok(())
            }
            ServerEnvelope::V1_0(V1_0ServerMessage::UpdateComponents(uc)) => {
                for c in uc.components {
                    self.forest.upsert(&uc.surface_id, c)?;
                }
                Ok(())
            }
            ServerEnvelope::V1_0(V1_0ServerMessage::UpdateDataModel(udm)) => {
                let path = udm.path.clone().unwrap_or_else(|| "/".to_string());
                // A2UI v1.0：省略 value 与显式 null 均为删除。a2ui-core 已把 JSON
                // null 反序列化为 None，故直接用 udm.value（Some=upsert，None=删除）。
                self.data_model.apply_pointer(&path, udm.value.clone())?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn binding(&self) -> DataBinding {
        DataBinding::new(self.data_model.clone())
    }

    fn resolver(&self) -> PathResolver {
        PathResolver::new(self.data_model.clone())
    }
}

// --------------------------------------------------------------------------
// 解析：把一个组件的属性子集解析成具体 JSON 值。
// 只使用核心原语（PathResolver / FunctionDispatcher / FormatString），
// 不在 runner 里重写 TS 的插值逻辑——核心缺什么，向量就该在此暴露。
// --------------------------------------------------------------------------

/// 把单个动态 JSON 值解析为具体 Value（供 resolved / tree 断言）。
fn resolve_value(value: &Value, resolver: &PathResolver, dispatcher: &FunctionDispatcher) -> Value {
    match value {
        Value::Object(map) => {
            // DynamicValue::Path
            if let Some(Value::String(p)) = map.get("path") {
                return resolver.resolve(p).unwrap_or(Value::Null);
            }
            // DynamicValue::FunctionCall
            if let Some(Value::String(call)) = map.get("call") {
                if call == "@index" {
                    return resolver.resolve("@index").unwrap_or(Value::Null);
                }
                let args = map.get("args").cloned().unwrap_or(Value::Null);
                return resolve_function_call(call, &args, resolver, dispatcher);
            }
            // 普通对象：逐值递归
            let mut out = Map::new();
            for (k, v) in map {
                out.insert(k.clone(), resolve_value(v, resolver, dispatcher));
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(
            arr.iter()
                .map(|v| resolve_value(v, resolver, dispatcher))
                .collect(),
        ),
        _ => value.clone(),
    }
}

/// 通过核心 FunctionDispatcher / FormatString 求值一个函数调用。
///
/// formatString 在向量里的形态是 `{template, bindings}` + `{key}` 占位符；
/// 我们先解析 bindings 为具体值，再用核心能力求值。核心若不支持该形态，
/// 结果自然与期望不符（漂移会被断言暴露），runner 不自行重写逻辑。
fn resolve_function_call(
    call: &str,
    args: &Value,
    resolver: &PathResolver,
    dispatcher: &FunctionDispatcher,
) -> Value {
    if call == "formatString" {
        // 先把 bindings 内的动态值解析成具体值
        let template = args
            .get("template")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut resolved_bindings: Map<String, Value> = Map::new();
        if let Some(bindings) = args.get("bindings").and_then(|v| v.as_object()) {
            for (k, v) in bindings {
                resolved_bindings.insert(k.clone(), resolve_value(v, resolver, dispatcher));
            }
        }
        // 交给核心 FunctionDispatcher（若注册了 formatString handler）。
        let dispatch_args = serde_json::json!({
            "template": template,
            "bindings": Value::Object(resolved_bindings),
        });
        return dispatcher
            .dispatch(
                "formatString",
                dispatch_args,
                a2ui_renderer::CallableFrom::ClientOrRemote,
            )
            .unwrap_or(Value::Null);
    }

    // 其它函数：解析 args 后交给核心 dispatcher。
    let resolved_args = resolve_value(args, resolver, dispatcher);
    dispatcher
        .dispatch(
            call,
            resolved_args,
            a2ui_renderer::CallableFrom::ClientOrRemote,
        )
        .unwrap_or(Value::Null)
}

/// 解析一个组件的全部属性（跳过结构性字段）为具体值 map。
fn resolve_component_props(
    props: &Value,
    resolver: &PathResolver,
    dispatcher: &FunctionDispatcher,
) -> Map<String, Value> {
    let mut out = Map::new();
    if let Some(obj) = props.as_object() {
        for (k, v) in obj {
            // 结构性 key 不做值解析（与 TS resolve.ts 的 STRUCTURAL_KEYS 对齐），
            // 外加 component/id（Rust 侧 flatten 可能带入这两个键）。
            if is_structural_key(k) {
                continue;
            }
            out.insert(k.clone(), resolve_value(v, resolver, dispatcher));
        }
    }
    out
}

/// 与 TS `STRUCTURAL_KEYS` 对齐的结构性属性 key（不做值解析）。
fn is_structural_key(k: &str) -> bool {
    matches!(
        k,
        "component" | "id" | "children" | "child" | "content" | "trigger" | "tabs" | "action"
    )
}

// --------------------------------------------------------------------------
// 归一化渲染树：{ id, type, props, children }
// template 实例 id 形如 `row_tpl#0`（与向量约定一致）。
// --------------------------------------------------------------------------

/// 从给定组件 ID 出发，用 `ComponentForest::get`（公开 API）递归构建归一化树。
///
/// 不复用 `ComponentForest::build_tree`，因为后者硬编码以 id 为 `"root"`
/// 的组件为根；而向量（如 01）用 `root_card` 作根（TS 侧的约定名 root
/// 检测），故 runner 自行确定根并遍历——只读、不改核心语义。
fn build_normalized(
    forest: &ComponentForest,
    surface_id: &str,
    comp_id: &ComponentId,
    resolver: &PathResolver,
    dispatcher: &FunctionDispatcher,
) -> Value {
    let comp = forest
        .get(surface_id, comp_id)
        .unwrap_or_else(|| panic!("component `{}` not found while building tree", comp_id));

    // template 实例 id 形如 `row_tpl_0`（下划线，来自 expand_templates），
    // 向量约定为 `row_tpl#0`（井号）；归一化对齐。
    let id = normalize_instance_id(comp.id().as_str());
    let props = resolve_component_props(comp.properties(), resolver, dispatcher);

    let mut children = Vec::new();
    let p = comp.properties();
    // children 数组
    if let Some(ids) = p.get("children").and_then(|v| v.as_array()) {
        for id_val in ids {
            if let Some(s) = id_val.as_str() {
                if let Ok(cid) = ComponentId::new(s) {
                    children.push(build_normalized(
                        forest, surface_id, &cid, resolver, dispatcher,
                    ));
                }
            }
        }
    }
    // 单个 child（Button / Card）
    if let Some(child) = p.get("child").and_then(|v| v.as_str()) {
        if let Ok(cid) = ComponentId::new(child) {
            children.push(build_normalized(
                forest, surface_id, &cid, resolver, dispatcher,
            ));
        }
    }

    serde_json::json!({
        "id": id,
        "type": comp.component_type(),
        "props": Value::Object(props),
        "children": Value::Array(children),
    })
}

/// 确定 Surface 根组件 ID，与 TS store.ts 的 `rootId` 保持一致：
/// 约定名优先（`root` / `root_card`），否则取未被任何组件引用者。
fn detect_root(forest: &ComponentForest, surface_id: &str) -> Option<ComponentId> {
    let comps = forest.components_of(surface_id);
    if comps.is_empty() {
        return None;
    }
    // 约定名
    for name in ["root", "root_card"] {
        if let Ok(cid) = ComponentId::new(name) {
            if forest.get(surface_id, &cid).is_some() {
                return Some(cid);
            }
        }
    }
    // 收集被引用的 ID
    let mut referenced = std::collections::HashSet::new();
    for c in &comps {
        let p = c.properties();
        if let Some(ids) = p.get("children").and_then(|v| v.as_array()) {
            for id_val in ids {
                if let Some(s) = id_val.as_str() {
                    referenced.insert(s.to_string());
                }
            }
        }
        // 模板 children 对象
        if let Some(obj) = p.get("children").and_then(|v| v.as_object()) {
            if let Some(t) = obj.get("template").and_then(|v| v.as_str()) {
                referenced.insert(t.to_string());
            }
        }
        if let Some(child) = p.get("child").and_then(|v| v.as_str()) {
            referenced.insert(child.to_string());
        }
    }
    comps
        .iter()
        .find(|c| !referenced.contains(c.id().as_str()))
        .map(|c| c.id().clone())
}

/// 把 `tpl_0` 尾部的 `_<n>` 归一化为 `#<n>`（仅当以数字结尾时）。
fn normalize_instance_id(id: &str) -> String {
    if let Some(pos) = id.rfind('_') {
        let (head, tail) = id.split_at(pos);
        let digits = &tail[1..];
        if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
            return format!("{}#{}", head, digits);
        }
    }
    id.to_string()
}

// --------------------------------------------------------------------------
// 断言辅助：子集匹配 & 深度相等，失败信息带用例名与字段。
// --------------------------------------------------------------------------

/// 断言 `actual` 的每个键都与 `expected` 深度相等（子集匹配）。
fn assert_subset(case: &str, id: &str, expected: &Value, actual: &Map<String, Value>) {
    let exp_obj = expected
        .as_object()
        .expect("expected resolved must be object");
    for (k, ev) in exp_obj {
        match actual.get(k) {
            Some(av) => assert_eq!(
                av, ev,
                "[{}] component `{}` prop `{}`: 期望 {} 实际 {}",
                case, id, k, ev, av
            ),
            None => panic!(
                "[{}] component `{}` 缺少 prop `{}`（期望 {}）",
                case, id, k, ev
            ),
        }
    }
}

// --------------------------------------------------------------------------
// runner
// --------------------------------------------------------------------------

/// 运行单个用例，返回 Ok(()) 或包含漂移信息的 panic。
fn run_case(name: &str, case: &Value) {
    let mut engine = Engine::new();

    let mut first_surface: Option<String> = None;
    let messages = case
        .get("messages")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("[{}] missing messages array", name));

    for (i, env) in messages.iter().enumerate() {
        if let Some(cs) = env.get("createSurface") {
            if first_surface.is_none() {
                if let Some(sid) = cs.get("surfaceId").and_then(|v| v.as_str()) {
                    first_surface = Some(sid.to_string());
                }
            }
        }
        engine
            .ingest(env)
            .unwrap_or_else(|e| panic!("[{}] message #{} ingest 失败: {}", name, i, e));
    }

    let sid = case
        .get("surfaceId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or(first_surface)
        .unwrap_or_else(|| panic!("[{}] 无法确定 surfaceId", name));

    let expect = case.get("expect").expect("case must have expect");

    // 1) dataModel 深度相等
    if let Some(exp_dm) = expect.get("dataModel") {
        assert_eq!(
            engine.data_model.as_value(),
            exp_dm,
            "[{}] dataModel 不一致：期望 {} 实际 {}",
            name,
            exp_dm,
            engine.data_model.as_value()
        );
    }

    let dispatcher = FunctionDispatcher::new();

    // 2) resolved：每个组件的关键属性子集匹配（根作用域）
    if let Some(resolved) = expect.get("resolved").and_then(|v| v.as_object()) {
        let resolver = engine.resolver();
        for (id, exp_props) in resolved {
            let comp = engine
                .forest
                .get(&sid, &ComponentId::new(id).unwrap())
                .unwrap_or_else(|| panic!("[{}] 组件 `{}` 未找到", name, id));
            let actual = resolve_component_props(comp.properties(), &resolver, &dispatcher);
            assert_subset(name, id, exp_props, &actual);
        }
    }

    // 3) tree：展开模板后归一化，结构/类型/props/children 深度相等
    if let Some(exp_tree) = expect.get("tree") {
        // 先展开模板（会把 {template,path} 的 children 替换为具体实例 ID）
        let binding = engine.binding();
        let resolver = engine.resolver();
        engine
            .forest
            .expand_templates(&sid, &binding, &resolver, &dispatcher)
            .unwrap_or_else(|e| panic!("[{}] expand_templates 失败: {}", name, e));

        let root = detect_root(&engine.forest, &sid)
            .unwrap_or_else(|| panic!("[{}] 无法确定渲染树根组件", name));
        let normalized = build_normalized(&engine.forest, &sid, &root, &resolver, &dispatcher);
        assert_eq!(
            &normalized,
            exp_tree,
            "[{}] 渲染树不一致：\n期望 {}\n实际 {}",
            name,
            serde_json::to_string_pretty(exp_tree).unwrap(),
            serde_json::to_string_pretty(&normalized).unwrap()
        );
    }
}

// --------------------------------------------------------------------------
// 每个 JSON 用例一个 #[test]，失败信息能指出是哪个用例、哪个字段。
// --------------------------------------------------------------------------

#[test]
fn conformance_dir_has_vectors() {
    let cases = load_cases();
    assert!(
        cases.len() >= 7,
        "期望至少 7 个一致性向量，实际 {}",
        cases.len()
    );
}

#[test]
fn vector_01_create_surface_basic() {
    run_named("01_create_surface_basic.json");
}

#[test]
fn vector_02_update_data_model_upsert() {
    run_named("02_update_data_model_upsert.json");
}

#[test]
fn vector_03_update_data_model_delete() {
    run_named("03_update_data_model_delete.json");
}

#[test]
fn vector_04_dynamic_value_path() {
    run_named("04_dynamic_value_path.json");
}

/// formatString（`{key}` 占位符 + `bindings`）。此前 Rust 侧不兼容 TS 语义
/// （曾用 `${}` 且未注册内置函数）——现 `FunctionDispatcher::new()` 已注册
/// A2UI 兼容的 `formatString`，本向量转绿。
#[test]
fn vector_05_format_string() {
    run_named("05_format_string.json");
}

/// ChildList template + `@index` + 集合作用域相对路径，每行文本经 `formatString`
/// （`{i}. {label}` + bindings）。formatString 兼容后本向量转绿。
#[test]
fn vector_06_childlist_template_index() {
    run_named("06_childlist_template_index.json");
}

#[test]
fn vector_07_progressive_placeholder() {
    run_named("07_progressive_placeholder.json");
}

fn run_named(file: &str) {
    let cases = load_cases();
    let (name, case) = cases
        .iter()
        .find(|(n, _)| n == file)
        .unwrap_or_else(|| panic!("找不到向量文件 {}", file));
    run_case(name, case);
}
