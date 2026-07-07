use crate::data_binding::DataBinding;
use crate::error::{RenderResult, RendererError};
use crate::function_dispatcher::FunctionDispatcher;
use crate::path_resolver::PathResolver;
use a2ui_core::prelude::*;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// 收集组件**直接**引用的子组件 id：`children`（数组或 `{template,path}`）、
/// `child`、`content`、`trigger`、`tabs[].child`。用于根检测的「未被引用」判定。
fn collect_child_refs(component: &Component, out: &mut HashSet<ComponentId>) {
    let props = component.properties();

    match props.get("children") {
        Some(Value::Array(arr)) => {
            for v in arr {
                if let Some(s) = v.as_str() {
                    push_ref(out, s);
                }
            }
        }
        Some(Value::Object(obj)) => {
            if let Some(s) = obj.get("template").and_then(|v| v.as_str()) {
                push_ref(out, s);
            }
        }
        _ => {}
    }

    for key in ["child", "content", "trigger"] {
        if let Some(s) = props.get(key).and_then(|v| v.as_str()) {
            push_ref(out, s);
        }
    }

    if let Some(arr) = props.get("tabs").and_then(|v| v.as_array()) {
        for tab in arr {
            if let Some(s) = tab.get("child").and_then(|v| v.as_str()) {
                push_ref(out, s);
            }
        }
    }
}

/// 把合法的 ComponentId 字符串加入引用集合（非法 id 静默跳过）。
fn push_ref(out: &mut HashSet<ComponentId>, s: &str) {
    if let Ok(id) = ComponentId::new(s) {
        out.insert(id);
    }
}

/// 模板克隆递归中保持不变的上下文（实例后缀 + 作用域解析器 + 函数分发器）
struct TemplateCloneCtx<'a> {
    suffix: &'a str,
    scope_resolver: &'a PathResolver,
    dispatcher: &'a FunctionDispatcher,
}

/// 组件树节点
#[derive(Debug, Clone)]
pub struct ComponentTreeNode {
    pub component: Component,
    pub children: Vec<ComponentTreeNode>,
}

impl ComponentTreeNode {
    pub fn new(component: Component) -> Self {
        Self {
            component,
            children: Vec::new(),
        }
    }

    pub fn with_children(mut self, children: Vec<ComponentTreeNode>) -> Self {
        self.children = children;
        self
    }
}

/// 组件森林：管理所有 Surface 的组件树
/// 使用邻接表模型：flat list + ID map，延迟构建树
#[derive(Debug, Clone)]
pub struct ComponentForest {
    surfaces: HashMap<String, ComponentSurface>,
    /// 反向索引: component_id → surface_id（O(1) 确定性查找）
    component_to_surface: HashMap<ComponentId, String>,
}

impl Default for ComponentForest {
    fn default() -> Self {
        Self {
            surfaces: HashMap::new(),
            component_to_surface: HashMap::new(),
        }
    }
}

/// 单个 Surface 的组件存储
#[derive(Debug, Clone)]
struct ComponentSurface {
    /// flat list → tree 的构建缓存
    tree: Option<ComponentTreeNode>,
    /// 所有组件的 flat map
    components: HashMap<ComponentId, Component>,
    /// 组件到达顺序（用于根检测的确定性兜底）
    order: Vec<ComponentId>,
    /// root 组件 ID（每次 upsert 后按 detect_root 重算）
    root: ComponentId,
}

impl ComponentForest {
    /// 创建新的空组件森林
    pub fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            component_to_surface: HashMap::new(),
        }
    }

    /// 向指定 Surface 添加或更新组件
    pub fn upsert(&mut self, surface_id: &str, component: Component) -> RenderResult<()> {
        let surface = self
            .surfaces
            .entry(surface_id.to_string())
            .or_insert_with(|| ComponentSurface {
                tree: None,
                components: HashMap::new(),
                order: Vec::new(),
                root: ComponentId::new("root").expect("'root' is a valid ComponentId"),
            });

        let comp_id = component.id().clone();

        if !surface.components.contains_key(&comp_id) {
            surface.order.push(comp_id.clone());
        }
        surface.components.insert(comp_id.clone(), component);
        // 组件集变化后重算 root（约定名优先，否则未被引用者）
        surface.root = Self::detect_root(surface);

        // 维护反向索引
        self.component_to_surface
            .insert(comp_id, surface_id.to_string());
        surface.tree = None;
        Ok(())
    }

    /// 检测 Surface 的 root 组件 id。
    ///
    /// 优先级（与 TS `store.ts` 的 rootId 逻辑对齐）：
    /// 1. 约定名 `root`，其次 `root_card`；
    /// 2. 否则取「未被任何组件作为子节点引用」的组件（按到达顺序取首个）；
    /// 3. 再兜底到到达顺序首个，最后默认 `root`。
    fn detect_root(surface: &ComponentSurface) -> ComponentId {
        for name in ["root", "root_card"] {
            if let Ok(id) = ComponentId::new(name) {
                if surface.components.contains_key(&id) {
                    return id;
                }
            }
        }

        let mut referenced: HashSet<ComponentId> = HashSet::new();
        for comp in surface.components.values() {
            collect_child_refs(comp, &mut referenced);
        }
        for id in &surface.order {
            if !referenced.contains(id) {
                return id.clone();
            }
        }

        surface
            .order
            .first()
            .cloned()
            .unwrap_or_else(|| ComponentId::new("root").expect("'root' is a valid ComponentId"))
    }

    /// 通过 component_id 查找所属的 surface_id
    pub fn surface_of(&self, component_id: &ComponentId) -> Option<&str> {
        self.component_to_surface
            .get(component_id)
            .map(|s| s.as_str())
    }

    /// 获取指定 Surface 中的所有组件
    pub fn components_of(&self, surface_id: &str) -> Vec<&Component> {
        self.surfaces
            .get(surface_id)
            .map(|s| s.components.values().collect())
            .unwrap_or_default()
    }

    /// 获取指定 Surface 的组件
    pub fn get(&self, surface_id: &str, component_id: &ComponentId) -> Option<&Component> {
        self.surfaces.get(surface_id)?.components.get(component_id)
    }

    /// 获取指定 Surface 的 root 组件
    pub fn get_root(&self, surface_id: &str) -> Option<&Component> {
        let surface = self.surfaces.get(surface_id)?;
        surface.components.get(&surface.root)
    }

    /// 移除整个 Surface
    pub fn remove_surface(&mut self, surface_id: &str) -> RenderResult<()> {
        // 清理反向索引
        if let Some(surface) = self.surfaces.get(surface_id) {
            for comp_id in surface.components.keys() {
                self.component_to_surface.remove(comp_id);
            }
        }
        self.surfaces.remove(surface_id);
        Ok(())
    }

    /// 获取指定 Surface 的组件数量
    pub fn component_count(&self, surface_id: &str) -> usize {
        self.surfaces
            .get(surface_id)
            .map(|s| s.components.len())
            .unwrap_or(0)
    }

    /// 展开 ChildList::Object 模板：从 Data Model 读取数组，为每个项实例化模板组件
    ///
    /// 返回新创建的组件 ID 列表。父组件的 `children` 属性会从
    /// `{"template": "...", "path": "..."}` 更新为 `{"children": [id0, id1, ...]}`。
    pub fn expand_templates(
        &mut self,
        surface_id: &str,
        data_binding: &DataBinding,
        resolver: &PathResolver,
        dispatcher: &FunctionDispatcher,
    ) -> RenderResult<Vec<ComponentId>> {
        // 1. 收集需要展开的模板（避免持有借用同时修改 HashMap）
        let templates_to_expand: Vec<(ComponentId, String, String)> = {
            let surface = self.surfaces.get(surface_id).ok_or_else(|| {
                crate::error::RendererError::SurfaceNotFound(surface_id.to_string())
            })?;
            let mut result = Vec::new();
            for comp in surface.components.values() {
                if let Some(obj) = comp.properties().as_object() {
                    if let Some(children_val) = obj.get("children") {
                        if let Some(children_obj) = children_val.as_object() {
                            if let (Some(template_id), Some(path)) = (
                                children_obj.get("template").and_then(|v| v.as_str()),
                                children_obj.get("path").and_then(|v| v.as_str()),
                            ) {
                                result.push((
                                    comp.id().clone(),
                                    template_id.to_string(),
                                    path.to_string(),
                                ));
                            }
                        }
                    }
                }
            }
            result
        };

        let mut all_new_ids = Vec::new();

        // 2. 逐个展开模板
        for (parent_id, template_id_str, data_path) in &templates_to_expand {
            let new_ids = self.expand_one_template(
                surface_id,
                parent_id,
                template_id_str,
                data_path,
                data_binding,
                resolver,
                dispatcher,
            )?;
            all_new_ids.extend(new_ids);
        }

        Ok(all_new_ids)
    }

    /// 展开单个 ChildList::Object 模板
    fn expand_one_template(
        &mut self,
        surface_id: &str,
        parent_id: &ComponentId,
        template_id_str: &str,
        data_path: &str,
        data_binding: &DataBinding,
        resolver: &PathResolver,
        dispatcher: &FunctionDispatcher,
    ) -> RenderResult<Vec<ComponentId>> {
        // 读取 Data Model 中的数组（使用 DataBinding::get 而非 Value::get，前者走 JSON Pointer）
        let array = data_binding
            .get(data_path)
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                crate::error::RendererError::BindingError(format!(
                    "template data path not found: {}",
                    data_path
                ))
            })?;

        // 获取模板组件（用于验证模板存在）
        let template_id = ComponentId::new(template_id_str)?;

        let mut expanded_ids = Vec::new();

        for (index, _item) in array.iter().enumerate() {
            // 创建带集合作用域的解析器（克隆已有 resolver 以复用 DataModel）
            let mut scope_resolver = resolver.clone();
            scope_resolver.enter_collection(data_path, index);

            let suffix = index.to_string();

            // 递归克隆并解析模板及其整个子树
            let new_id = self.clone_and_resolve_subtree(
                surface_id,
                &template_id,
                &suffix,
                &scope_resolver,
                dispatcher,
            )?;
            expanded_ids.push(new_id);
        }

        // 3. 更新父组件的 children 属性
        if !expanded_ids.is_empty() {
            let parent_comp = {
                let surface = self.surfaces.get(surface_id).ok_or_else(|| {
                    crate::error::RendererError::SurfaceNotFound(surface_id.to_string())
                })?;
                surface.components.get(parent_id).cloned().ok_or_else(|| {
                    crate::error::RendererError::ComponentNotFound(parent_id.clone())
                })?
            };

            // 通过 JSON 序列化/反序列化更新父组件（字段为 private）
            // 将 children 从 template 对象替换为展开后的 ID 数组
            let mut parent_json = serde_json::to_value(&parent_comp).map_err(|e| {
                crate::error::RendererError::CoreError(a2ui_core::A2uiError::Deserialization(e))
            })?;
            if let Some(obj) = parent_json.as_object_mut() {
                let ids: Vec<Value> = expanded_ids
                    .iter()
                    .map(|id| Value::String(id.as_str().to_string()))
                    .collect();
                obj.insert("children".to_string(), Value::Array(ids));
            }
            let new_parent: Component = serde_json::from_value(parent_json).map_err(|e| {
                crate::error::RendererError::CoreError(a2ui_core::A2uiError::Deserialization(e))
            })?;
            self.upsert(surface_id, new_parent)?;
        }

        Ok(expanded_ids)
    }

    /// 递归克隆组件子树，解析所有 DynamicValue 并重命名 ID
    ///
    /// 返回新创建的根组件 ID。
    /// 模板展开时，每个数组项调用一次，对整个子树做深拷贝 + 属性解析。
    fn clone_and_resolve_subtree(
        &mut self,
        surface_id: &str,
        comp_id: &ComponentId,
        suffix: &str,
        scope_resolver: &PathResolver,
        dispatcher: &FunctionDispatcher,
    ) -> RenderResult<ComponentId> {
        let mut visited = HashSet::new();
        let ctx = TemplateCloneCtx {
            suffix,
            scope_resolver,
            dispatcher,
        };
        self.clone_and_resolve_subtree_inner(surface_id, comp_id, &ctx, 0, &mut visited)
    }

    /// 克隆单条引用边（`child`/`content`/`trigger`/`tabs[].child`）指向的子树。
    ///
    /// 返回改写后的新 id 字符串；引用 id 非法时返回 `Ok(None)`（静默跳过，
    /// 与 build_tree 的边语义一致），组件缺失时向上传播错误。
    fn clone_edge_ref(
        &mut self,
        surface_id: &str,
        ref_str: &str,
        ctx: &TemplateCloneCtx<'_>,
        depth: usize,
        visited: &mut HashSet<ComponentId>,
    ) -> RenderResult<Option<String>> {
        let child_id = match ComponentId::new(ref_str) {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!("invalid child ID '{}' in template: {}", ref_str, e);
                return Ok(None);
            }
        };
        let new_id =
            self.clone_and_resolve_subtree_inner(surface_id, &child_id, ctx, depth + 1, visited)?;
        Ok(Some(new_id.as_str().to_string()))
    }

    /// clone_and_resolve_subtree 的内部递归实现
    /// 包含循环检测和深度限制
    fn clone_and_resolve_subtree_inner(
        &mut self,
        surface_id: &str,
        comp_id: &ComponentId,
        ctx: &TemplateCloneCtx<'_>,
        depth: usize,
        visited: &mut HashSet<ComponentId>,
    ) -> RenderResult<ComponentId> {
        // 深度限制
        if depth >= crate::error::MAX_TREE_DEPTH {
            return Err(RendererError::tree_too_deep(depth));
        }

        // 循环检测
        if !visited.insert(comp_id.clone()) {
            return Err(RendererError::BindingError(format!(
                "circular component reference detected: {}",
                comp_id.as_str()
            )));
        }

        // 获取原始组件
        let original = {
            let surface = self
                .surfaces
                .get(surface_id)
                .ok_or_else(|| RendererError::SurfaceNotFound(surface_id.to_string()))?;
            surface
                .components
                .get(comp_id)
                .cloned()
                .ok_or_else(|| RendererError::ComponentNotFound(comp_id.clone()))?
        };

        let new_id_str = format!("{}_{}", comp_id.as_str(), ctx.suffix);

        // 序列化为 JSON，解析动态值，设置新 ID
        let mut comp_json = serde_json::to_value(&original)
            .map_err(|e| RendererError::CoreError(a2ui_core::A2uiError::Deserialization(e)))?;

        if let Some(obj) = comp_json.as_object_mut() {
            obj.remove("id");
            obj.insert("id".to_string(), Value::String(new_id_str.clone()));

            // 递归处理 children 数组中的每个子组件引用
            if let Some(children_val) = obj.get_mut("children") {
                if let Some(children_arr) = children_val.as_array() {
                    let mut new_children = Vec::new();
                    for id_val in children_arr {
                        let id_str = match id_val.as_str() {
                            Some(s) => s,
                            None => {
                                tracing::warn!("non-string child ID in template: {:?}", id_val);
                                continue;
                            }
                        };
                        if let Some(new_child_id) =
                            self.clone_edge_ref(surface_id, id_str, ctx, depth, visited)?
                        {
                            new_children.push(Value::String(new_child_id));
                        }
                    }
                    *children_val = Value::Array(new_children);
                }
            }

            // 处理单引用边：Button/Card 的 child、Modal 的 content/trigger
            for key in ["child", "content", "trigger"] {
                let ref_str = match obj.get(key).and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                if let Some(new_ref) =
                    self.clone_edge_ref(surface_id, &ref_str, ctx, depth, visited)?
                {
                    obj.insert(key.to_string(), Value::String(new_ref));
                }
            }

            // 处理 Tabs 的 tabs[].child（数组内嵌对象，需就地改写 tab["child"]）
            if let Some(tabs) = obj.get_mut("tabs").and_then(|v| v.as_array_mut()) {
                for tab in tabs.iter_mut() {
                    let ref_str = match tab.get("child").and_then(|v| v.as_str()) {
                        Some(s) => s.to_string(),
                        None => continue,
                    };
                    if let Some(new_ref) =
                        self.clone_edge_ref(surface_id, &ref_str, ctx, depth, visited)?
                    {
                        if let Some(tab_obj) = tab.as_object_mut() {
                            tab_obj.insert("child".to_string(), Value::String(new_ref));
                        }
                    }
                }
            }

            // 转换 DynamicValue：相对路径→绝对路径，@index→字面量
            // 保留其他动态绑定（路径、函数调用）供渲染时解析
            // content/trigger 已改写为纯字符串 id，transform 对字符串是恒等，
            // 无需加入跳过名单；tabs 内的 title 等动态值仍需转换，故不整体跳过
            for (key, val) in obj.iter_mut() {
                if key == "component" || key == "id" || key == "children" || key == "child" {
                    continue;
                }
                *val =
                    Self::transform_dynamic_for_template(val, ctx.scope_resolver, ctx.dispatcher);
            }
        }

        let new_comp: Component = serde_json::from_value(comp_json)
            .map_err(|e| RendererError::CoreError(a2ui_core::A2uiError::Deserialization(e)))?;
        let new_id = ComponentId::new(&new_id_str)?;
        self.upsert(surface_id, new_comp)?;
        Ok(new_id)
    }

    /// 模板展开时转换 DynamicValue：将相对路径转为绝对路径，保留绑定结构
    ///
    /// 与 `resolve_value_json`（急切求值）不同，此函数做最小转换：
    /// - `{"path": "name"}` → `{"path": "/items/0/name"}`（保留 DataBinding）
    /// - `{"path": "/global"}` → 原样保留
    /// - `{"call": "@index"}` → 急切求值为当前索引（系统上下文变量）
    /// - `{"call": "func", "args": {...}}` → 递归转换 args 中的路径，保留 FunctionCall 结构
    /// - 嵌套对象/数组 → 递归处理
    /// - 字面量 → 原样保留
    fn transform_dynamic_for_template(
        value: &Value,
        resolver: &PathResolver,
        dispatcher: &FunctionDispatcher,
    ) -> Value {
        match value {
            Value::Object(map) => {
                // 检测 DynamicValue::Path: {"path": "..."}
                // 将相对路径转为绝对路径，保留 DataBinding 结构供渲染时解析
                if let Some(Value::String(p)) = map.get("path") {
                    let absolute = resolver.make_absolute(p);
                    return json!({"path": absolute});
                }
                // 检测 DynamicValue::FunctionCall: {"call": "...", "args": {...}}
                if let Some(Value::String(call)) = map.get("call") {
                    if call == "@index" {
                        // @index 是系统上下文变量，只在模板展开时有效，必须急切求值
                        return resolver.resolve("@index").unwrap_or(Value::Null);
                    }
                    // 其他函数调用：递归转换 args 中的路径绑定，保留 FunctionCall 结构
                    let mut result = serde_json::Map::new();
                    result.insert("call".to_string(), Value::String(call.clone()));
                    if let Some(args) = map.get("args") {
                        result.insert(
                            "args".to_string(),
                            Self::transform_dynamic_for_template(args, resolver, dispatcher),
                        );
                    }
                    return Value::Object(result);
                }
                // 普通对象：递归处理每个值
                let mut result = serde_json::Map::new();
                for (k, v) in map {
                    result.insert(
                        k.clone(),
                        Self::transform_dynamic_for_template(v, resolver, dispatcher),
                    );
                }
                Value::Object(result)
            }
            Value::Array(arr) => Value::Array(
                arr.iter()
                    .map(|v| Self::transform_dynamic_for_template(v, resolver, dispatcher))
                    .collect(),
            ),
            _ => value.clone(),
        }
    }

    /// 构建组件树（含循环检测和深度限制）
    pub fn build_tree(&self, surface_id: &str) -> RenderResult<ComponentTreeNode> {
        let surface = self
            .surfaces
            .get(surface_id)
            .ok_or_else(|| crate::error::RendererError::SurfaceNotFound(surface_id.to_string()))?;

        let root_id = &surface.root;
        let root_comp = surface
            .components
            .get(root_id)
            .ok_or_else(|| crate::error::RendererError::ComponentNotFound(root_id.clone()))?;

        let mut visited = HashSet::new();
        self.build_node_with_depth(root_comp, &surface.components, 0, &mut visited)
    }

    /// 递归构建节点，包含深度限制和循环检测
    fn build_node_with_depth(
        &self,
        component: &Component,
        all: &HashMap<ComponentId, Component>,
        depth: usize,
        visited: &mut HashSet<ComponentId>,
    ) -> RenderResult<ComponentTreeNode> {
        // 深度限制
        if depth >= crate::error::MAX_TREE_DEPTH {
            return Err(RendererError::tree_too_deep(depth));
        }

        // 循环检测
        if !visited.insert(component.id().clone()) {
            return Err(RendererError::BindingError(format!(
                "circular component reference detected: {}",
                component.id().as_str()
            )));
        }

        let mut node = ComponentTreeNode::new(component.clone());
        let props = component.properties();

        // 树边追加顺序约定（与 collect_child_refs 的引用统计对齐）：
        // children → child → content → trigger → tabs[].child。
        // 消费端应按 id 匹配而非依赖位置；顺序仅保证确定性。

        // children 属性（数组格式：["id1", "id2"]）
        if let Some(ids) = props.get("children").and_then(|v| v.as_array()) {
            for id_val in ids {
                if let Some(id_str) = id_val.as_str() {
                    self.append_child_by_id(id_str, all, depth, visited, &mut node)?;
                }
            }
        }

        // Button/Card 的 child 属性
        if let Some(child_str) = props.get("child").and_then(|v| v.as_str()) {
            self.append_child_by_id(child_str, all, depth, visited, &mut node)?;
        }

        // Modal 的 content/trigger 属性
        for key in ["content", "trigger"] {
            if let Some(child_str) = props.get(key).and_then(|v| v.as_str()) {
                self.append_child_by_id(child_str, all, depth, visited, &mut node)?;
            }
        }

        // Tabs 的 tabs[].child 属性（按数组序）
        if let Some(tabs) = props.get("tabs").and_then(|v| v.as_array()) {
            for tab in tabs {
                if let Some(child_str) = tab.get("child").and_then(|v| v.as_str()) {
                    self.append_child_by_id(child_str, all, depth, visited, &mut node)?;
                }
            }
        }

        Ok(node)
    }

    /// 按 id 字符串解析并递归构建子节点，追加进 node.children。
    /// 非法 id 或组件缺失时静默跳过（与既有 children/child 边语义一致）。
    ///
    /// 模板克隆（clone_and_resolve_subtree_inner）与此处跟随同一组边：
    /// children/child/content/trigger/tabs[].child，两侧语义保持一致。
    fn append_child_by_id(
        &self,
        id_str: &str,
        all: &HashMap<ComponentId, Component>,
        depth: usize,
        visited: &mut HashSet<ComponentId>,
        node: &mut ComponentTreeNode,
    ) -> RenderResult<()> {
        if let Ok(child_id) = ComponentId::new(id_str) {
            if let Some(child_comp) = all.get(&child_id) {
                let child_node = self.build_node_with_depth(child_comp, all, depth + 1, visited)?;
                node.children.push(child_node);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_tree_node_new() {
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let node = ComponentTreeNode::new(comp);
        assert!(node.children.is_empty());
    }

    #[test]
    fn test_component_forest_new() {
        let forest = ComponentForest::new();
        assert!(forest.surfaces.is_empty());
    }

    #[test]
    fn build_tree_detects_root_card_without_literal_root() {
        let mut forest = ComponentForest::new();
        let card: Component = serde_json::from_value(
            serde_json::json!({"component":"Card","id":"root_card","child":"form_col"}),
        )
        .unwrap();
        let col: Component = serde_json::from_value(
            serde_json::json!({"component":"Column","id":"form_col","children":["title"]}),
        )
        .unwrap();
        let title = Component::text(
            ComponentId::new("title").unwrap(),
            DynamicValue::Literal("hi".to_string()),
        );
        forest.upsert("s1", card).unwrap();
        forest.upsert("s1", col).unwrap();
        forest.upsert("s1", title).unwrap();

        let tree = forest.build_tree("s1").unwrap();
        assert_eq!(tree.component.id().as_str(), "root_card");
        assert_eq!(forest.get_root("s1").unwrap().id().as_str(), "root_card");
    }

    #[test]
    fn detects_unreferenced_root_when_no_conventional_name() {
        let mut forest = ComponentForest::new();
        let top: Component = serde_json::from_value(
            serde_json::json!({"component":"Column","id":"top","children":["leaf"]}),
        )
        .unwrap();
        let leaf = Component::text(
            ComponentId::new("leaf").unwrap(),
            DynamicValue::Literal("x".to_string()),
        );
        forest.upsert("s1", top).unwrap();
        forest.upsert("s1", leaf).unwrap();
        assert_eq!(forest.get_root("s1").unwrap().id().as_str(), "top");
    }

    #[test]
    fn literal_root_still_wins_over_other_components() {
        let mut forest = ComponentForest::new();
        let root: Component = serde_json::from_value(
            serde_json::json!({"component":"Column","id":"root","children":["c"]}),
        )
        .unwrap();
        let c = Component::text(
            ComponentId::new("c").unwrap(),
            DynamicValue::Literal("c".to_string()),
        );
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", c).unwrap();
        assert_eq!(forest.get_root("s1").unwrap().id().as_str(), "root");
    }

    #[test]
    fn test_upsert_and_get() {
        let mut forest = ComponentForest::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        forest.upsert("s1", comp).unwrap();

        let retrieved = forest.get("s1", &ComponentId::new("root").unwrap());
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_get_root() {
        let mut forest = ComponentForest::new();
        let root = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let child = Component::text(
            ComponentId::new("child").unwrap(),
            DynamicValue::Literal("World".to_string()),
        );
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", child).unwrap();

        let root_comp = forest.get_root("s1");
        assert!(root_comp.is_some());
        assert_eq!(root_comp.unwrap().id().as_str(), "root");
    }

    #[test]
    fn test_build_tree() {
        let mut forest = ComponentForest::new();
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("child1").unwrap()],
        );
        let child1 = Component::text(
            ComponentId::new("child1").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", child1).unwrap();

        let tree = forest.build_tree("s1").unwrap();
        assert_eq!(tree.component.id().as_str(), "root");
        assert_eq!(tree.children.len(), 1);
    }

    #[test]
    fn build_tree_includes_modal_content_and_trigger() {
        let mut forest = ComponentForest::new();
        let modal: Component = serde_json::from_value(
            serde_json::json!({"component":"Modal","id":"root","content":"body","trigger":"btn"}),
        )
        .unwrap();
        let body = Component::text(
            ComponentId::new("body").unwrap(),
            DynamicValue::Literal("modal body".to_string()),
        );
        let btn: Component = serde_json::from_value(
            serde_json::json!({"component":"Button","id":"btn","label":"open"}),
        )
        .unwrap();
        forest.upsert("s1", modal).unwrap();
        forest.upsert("s1", body).unwrap();
        forest.upsert("s1", btn).unwrap();

        let tree = forest.build_tree("s1").unwrap();
        // 顺序约定锁定为 [content, trigger]
        assert_eq!(tree.children.len(), 2, "content 与 trigger 都应进入组件树");
        assert_eq!(tree.children[0].component.id().as_str(), "body");
        assert_eq!(tree.children[1].component.id().as_str(), "btn");
    }

    #[test]
    fn build_tree_includes_tabs_children() {
        let mut forest = ComponentForest::new();
        let tabs: Component = serde_json::from_value(serde_json::json!({
            "component":"Tabs","id":"root",
            "tabs":[{"title":"T1","child":"a"},{"title":"T2","child":"b"}]
        }))
        .unwrap();
        let a = Component::text(
            ComponentId::new("a").unwrap(),
            DynamicValue::Literal("tab a".to_string()),
        );
        let b = Component::text(
            ComponentId::new("b").unwrap(),
            DynamicValue::Literal("tab b".to_string()),
        );
        forest.upsert("s1", tabs).unwrap();
        forest.upsert("s1", a).unwrap();
        forest.upsert("s1", b).unwrap();

        let tree = forest.build_tree("s1").unwrap();
        assert_eq!(tree.children.len(), 2, "tabs[].child 应按数组序进入组件树");
        assert_eq!(tree.children[0].component.id().as_str(), "a");
        assert_eq!(tree.children[1].component.id().as_str(), "b");
    }

    #[test]
    fn build_tree_detects_cycle_via_content() {
        let mut forest = ComponentForest::new();
        let root: Component = serde_json::from_value(
            serde_json::json!({"component":"Column","id":"root","children":["m"]}),
        )
        .unwrap();
        // content 指回祖先 root，构成环——必须报错而非静默丢边/栈溢出
        let modal: Component = serde_json::from_value(
            serde_json::json!({"component":"Modal","id":"m","content":"root"}),
        )
        .unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", modal).unwrap();

        let err = forest.build_tree("s1").expect_err("content 成环必须报错");
        assert!(
            err.to_string().contains("circular"),
            "expected circular error, got: {err}"
        );
    }

    #[test]
    fn build_tree_modal_content_respects_depth_limit() {
        let mut forest = ComponentForest::new();
        // root → m1 → m2 → ... → m50，经 content 边串 51 个节点，超过 MAX_TREE_DEPTH=50
        let root: Component = serde_json::from_value(
            serde_json::json!({"component":"Modal","id":"root","content":"m1"}),
        )
        .unwrap();
        forest.upsert("s1", root).unwrap();
        for i in 1..=50 {
            let comp: Component = serde_json::from_value(serde_json::json!({
                "component":"Modal",
                "id": format!("m{i}"),
                "content": format!("m{}", i + 1)
            }))
            .unwrap();
            forest.upsert("s1", comp).unwrap();
        }

        let err = forest
            .build_tree("s1")
            .expect_err("content 边链超深必须报 tree_too_deep");
        assert!(
            err.to_string().contains("too deep") || err.to_string().contains("depth"),
            "expected depth error, got: {err}"
        );
    }

    #[test]
    fn test_remove_surface() {
        let mut forest = ComponentForest::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        forest.upsert("s1", comp).unwrap();
        assert!(forest
            .get("s1", &ComponentId::new("root").unwrap())
            .is_some());

        forest.remove_surface("s1").unwrap();
        assert!(forest
            .get("s1", &ComponentId::new("root").unwrap())
            .is_none());
    }

    // --- P1-1: @index scope system — template expansion ---

    #[test]
    fn test_expand_templates_object_mode() {
        let mut forest = ComponentForest::new();
        let dm = DataModel::new(json!({"items": [{"name": "a"}, {"name": "b"}]}));
        let binding = DataBinding::new(dm);
        let resolver = PathResolver::new(DataModel::new(binding.as_value().clone()));

        // Template: Text with relative path "name"
        let template = Component::text(
            ComponentId::new("item_tmpl").unwrap(),
            DynamicValue::Path {
                path: "name".into(),
            },
        );

        // Parent with ChildList::Object: template + data path
        let parent: Component =
            serde_json::from_value(json!({"component": "Column", "id": "list", "children": {"template": "item_tmpl", "path": "/items"}}))
                .unwrap();

        forest.upsert("s1", parent).unwrap();
        forest.upsert("s1", template).unwrap();

        let new_ids = forest
            .expand_templates("s1", &binding, &resolver, &FunctionDispatcher::new())
            .unwrap();

        assert_eq!(new_ids.len(), 2);

        // 验证相对路径已被转换为绝对路径，保留了 DataBinding 结构（而非急切求值）
        let comp0 = forest
            .get("s1", &ComponentId::new("item_tmpl_0").unwrap())
            .unwrap();
        assert_eq!(
            comp0.properties().get("text"),
            Some(&json!({"path": "/items/0/name"}))
        );

        let comp1 = forest
            .get("s1", &ComponentId::new("item_tmpl_1").unwrap())
            .unwrap();
        assert_eq!(
            comp1.properties().get("text"),
            Some(&json!({"path": "/items/1/name"}))
        );
    }

    #[test]
    fn test_expand_templates_preserves_reactive_binding() {
        // 验证模板展开后的组件保留 DataBinding，DataModel 更新时能响应
        let mut forest = ComponentForest::new();
        let dm = DataModel::new(json!({"items": [{"name": "old_a"}, {"name": "old_b"}]}));
        let binding = DataBinding::new(dm.clone());
        let resolver = PathResolver::new(dm.clone());

        let template = Component::text(
            ComponentId::new("item_tmpl").unwrap(),
            DynamicValue::Path {
                path: "name".into(),
            },
        );
        let parent: Component =
            serde_json::from_value(json!({"component": "Column", "id": "list", "children": {"template": "item_tmpl", "path": "/items"}}))
                .unwrap();

        forest.upsert("s1", parent).unwrap();
        forest.upsert("s1", template).unwrap();

        // 展开模板
        forest
            .expand_templates("s1", &binding, &resolver, &FunctionDispatcher::new())
            .unwrap();

        // 验证展开后保留的是路径绑定
        let comp = forest
            .get("s1", &ComponentId::new("item_tmpl_0").unwrap())
            .unwrap();
        assert_eq!(
            comp.properties().get("text"),
            Some(&json!({"path": "/items/0/name"}))
        );

        // 更新 DataModel：模拟响应式传播
        let mut new_dm = dm;
        new_dm
            .apply_pointer("/items/0/name", Some(json!("updated_a")))
            .unwrap();

        // 通过新的 DataModel 解析路径，验证能正确读取更新后的值
        // （真实场景中由渲染器在渲染时完成此解析）
        let _new_binding = DataBinding::new(new_dm.clone());
        let new_resolver = PathResolver::new(new_dm);
        let resolved = new_resolver.resolve("/items/0/name");
        assert_eq!(resolved, Some(json!("updated_a")));
    }

    #[test]
    fn test_expand_templates_with_at_index() {
        let mut forest = ComponentForest::new();
        let dm = DataModel::new(json!({"items": [{"name": "x"}, {"name": "y"}, {"name": "z"}]}));
        let binding = DataBinding::new(dm);
        let resolver = PathResolver::new(DataModel::new(binding.as_value().clone()));

        // Template uses @index function call
        let template = Component::text(
            ComponentId::new("idx_tmpl").unwrap(),
            DynamicValue::FunctionCall {
                call: "@index".into(),
                args: json!({}),
            },
        );

        let parent: Component =
            serde_json::from_value(json!({"component": "Column", "id": "list", "children": {"template": "idx_tmpl", "path": "/items"}}))
                .unwrap();

        forest.upsert("s1", parent).unwrap();
        forest.upsert("s1", template).unwrap();

        let new_ids = forest
            .expand_templates("s1", &binding, &resolver, &FunctionDispatcher::new())
            .unwrap();

        assert_eq!(new_ids.len(), 3);
        assert_eq!(
            forest
                .get("s1", &ComponentId::new("idx_tmpl_0").unwrap())
                .unwrap()
                .properties()
                .get("text"),
            Some(&json!(0))
        );
        assert_eq!(
            forest
                .get("s1", &ComponentId::new("idx_tmpl_1").unwrap())
                .unwrap()
                .properties()
                .get("text"),
            Some(&json!(1))
        );
        assert_eq!(
            forest
                .get("s1", &ComponentId::new("idx_tmpl_2").unwrap())
                .unwrap()
                .properties()
                .get("text"),
            Some(&json!(2))
        );
    }

    #[test]
    fn test_expand_templates_clones_modal_content_edge() {
        // 模板是 Modal（content 指向 body）：展开后每个实例的 content
        // 必须指向带实例后缀的克隆，而非原模板子组件（否则实例间共享状态）
        let mut forest = ComponentForest::new();
        let dm = DataModel::new(json!({"items": [{"name": "a"}, {"name": "b"}]}));
        let binding = DataBinding::new(dm);
        let resolver = PathResolver::new(DataModel::new(binding.as_value().clone()));

        let body = Component::text(
            ComponentId::new("body").unwrap(),
            DynamicValue::Path {
                path: "name".into(),
            },
        );
        let tmpl: Component = serde_json::from_value(json!({
            "component": "Modal", "id": "tmpl", "content": "body"
        }))
        .unwrap();
        let parent: Component = serde_json::from_value(json!({
            "component": "Column", "id": "list",
            "children": {"template": "tmpl", "path": "/items"}
        }))
        .unwrap();

        forest.upsert("s1", parent).unwrap();
        forest.upsert("s1", tmpl).unwrap();
        forest.upsert("s1", body).unwrap();

        let new_ids = forest
            .expand_templates("s1", &binding, &resolver, &FunctionDispatcher::new())
            .unwrap();
        assert_eq!(new_ids.len(), 2);

        for i in 0..2 {
            let inst_id = ComponentId::new(format!("tmpl_{i}")).unwrap();
            let inst = forest.get("s1", &inst_id).unwrap();
            assert_eq!(
                inst.properties().get("content"),
                Some(&json!(format!("body_{i}"))),
                "实例 {i} 的 content 应指向带实例后缀的克隆"
            );
            let clone_id = ComponentId::new(format!("body_{i}")).unwrap();
            let cloned = forest
                .get("s1", &clone_id)
                .expect("content 引用的克隆必须存在于 forest");
            assert_eq!(
                cloned.properties().get("text"),
                Some(&json!({"path": format!("/items/{i}/name")})),
                "克隆的相对路径应转换为实例作用域的绝对路径"
            );
        }
    }

    #[test]
    fn test_expand_templates_clones_trigger_and_tabs_edges() {
        // trigger（单引用边）与 tabs[].child（数组内嵌对象边）同样需要克隆改写
        let mut forest = ComponentForest::new();
        let dm = DataModel::new(json!({"items": [{"name": "a"}]}));
        let binding = DataBinding::new(dm);
        let resolver = PathResolver::new(DataModel::new(binding.as_value().clone()));

        let btn: Component = serde_json::from_value(json!({
            "component": "Button", "id": "btn", "label": "open"
        }))
        .unwrap();
        let tab_a = Component::text(
            ComponentId::new("tab_a").unwrap(),
            DynamicValue::Path {
                path: "name".into(),
            },
        );
        let tabs: Component = serde_json::from_value(json!({
            "component": "Tabs", "id": "tabbar",
            "tabs": [{"title": "T1", "child": "tab_a"}]
        }))
        .unwrap();
        let tmpl: Component = serde_json::from_value(json!({
            "component": "Modal", "id": "tmpl",
            "trigger": "btn", "content": "tabbar"
        }))
        .unwrap();
        let parent: Component = serde_json::from_value(json!({
            "component": "Column", "id": "list",
            "children": {"template": "tmpl", "path": "/items"}
        }))
        .unwrap();

        forest.upsert("s1", parent).unwrap();
        forest.upsert("s1", tmpl).unwrap();
        forest.upsert("s1", btn).unwrap();
        forest.upsert("s1", tabs).unwrap();
        forest.upsert("s1", tab_a).unwrap();

        forest
            .expand_templates("s1", &binding, &resolver, &FunctionDispatcher::new())
            .unwrap();

        let inst = forest
            .get("s1", &ComponentId::new("tmpl_0").unwrap())
            .unwrap();
        assert_eq!(
            inst.properties().get("trigger"),
            Some(&json!("btn_0")),
            "trigger 引用应改写为克隆 id"
        );
        assert!(
            forest
                .get("s1", &ComponentId::new("btn_0").unwrap())
                .is_some(),
            "trigger 克隆必须存在"
        );

        let tabbar = forest
            .get("s1", &ComponentId::new("tabbar_0").unwrap())
            .expect("content 克隆必须存在");
        assert_eq!(
            tabbar.properties().get("tabs"),
            Some(&json!([{"title": "T1", "child": "tab_a_0"}])),
            "tabs[].child 引用应改写为克隆 id"
        );
        assert!(
            forest
                .get("s1", &ComponentId::new("tab_a_0").unwrap())
                .is_some(),
            "tabs[].child 克隆必须存在"
        );
    }

    #[test]
    fn test_expand_templates_empty_array() {
        let mut forest = ComponentForest::new();
        let dm = DataModel::new(json!({"items": []}));
        let binding = DataBinding::new(dm);
        let resolver = PathResolver::new(DataModel::new(binding.as_value().clone()));

        let template = Component::text(
            ComponentId::new("item_tmpl").unwrap(),
            DynamicValue::Path {
                path: "name".into(),
            },
        );
        let parent: Component =
            serde_json::from_value(json!({"component": "Column", "id": "list", "children": {"template": "item_tmpl", "path": "/items"}}))
                .unwrap();

        forest.upsert("s1", parent).unwrap();
        forest.upsert("s1", template).unwrap();

        let new_ids = forest
            .expand_templates("s1", &binding, &resolver, &FunctionDispatcher::new())
            .unwrap();

        assert!(new_ids.is_empty());
    }

    #[test]
    fn test_expand_templates_missing_data_path() {
        let mut forest = ComponentForest::new();
        let dm = DataModel::new(json!({"other": [1, 2]}));
        let binding = DataBinding::new(dm);
        let resolver = PathResolver::new(DataModel::new(binding.as_value().clone()));

        let template = Component::text(
            ComponentId::new("item_tmpl").unwrap(),
            DynamicValue::Path {
                path: "name".into(),
            },
        );
        let parent: Component =
            serde_json::from_value(json!({"component": "Column", "id": "list", "children": {"template": "item_tmpl", "path": "/missing"}}))
                .unwrap();

        forest.upsert("s1", parent).unwrap();
        forest.upsert("s1", template).unwrap();

        let result = forest.expand_templates("s1", &binding, &resolver, &FunctionDispatcher::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_surface_of_returns_correct_surface() {
        let mut forest = ComponentForest::new();
        let c1 = Component::text(
            ComponentId::new("c1").unwrap(),
            DynamicValue::Literal("S1".to_string()),
        );
        let c2 = Component::text(
            ComponentId::new("c2").unwrap(),
            DynamicValue::Literal("S2".to_string()),
        );
        forest.upsert("s1", c1).unwrap();
        forest.upsert("s2", c2).unwrap();
        assert_eq!(
            forest.surface_of(&ComponentId::new("c1").unwrap()),
            Some("s1")
        );
        assert_eq!(
            forest.surface_of(&ComponentId::new("c2").unwrap()),
            Some("s2")
        );
    }

    #[test]
    fn test_surface_of_nonexistent() {
        let forest = ComponentForest::new();
        assert_eq!(
            forest.surface_of(&ComponentId::new("noexist").unwrap()),
            None
        );
    }

    #[test]
    fn test_remove_surface_cleans_reverse_index() {
        let mut forest = ComponentForest::new();
        let c = Component::text(
            ComponentId::new("c1").unwrap(),
            DynamicValue::Literal("X".to_string()),
        );
        forest.upsert("s1", c).unwrap();
        assert!(forest
            .surface_of(&ComponentId::new("c1").unwrap())
            .is_some());
        forest.remove_surface("s1").unwrap();
        assert!(forest
            .surface_of(&ComponentId::new("c1").unwrap())
            .is_none());
    }
}
