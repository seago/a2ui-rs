#!/usr/bin/env bash
# serde_json 隔离白名单检查（CLAUDE.md / ARCHITECTURE.md 架构约束）：
# 只允许 a2ui-core 与 a2ui-renderer 直接依赖 serde_json。
# 设计依据：docs/refactor-step2-serde-isolation.md §3.4。
#
# 用法：scripts/check-serde-isolation.sh（工作区根目录或任意子目录均可）
set -euo pipefail

cd "$(dirname "$0")/.."

ALLOWED="a2ui-core a2ui-renderer"

# cargo tree -i serde_json 的反向依赖树中，缩进一层（"└── "/"├── " 前缀，
# 深度 1）的节点即 serde_json 的直接依赖方；更深层都是经由它们的间接边。
direct_dependents=$(cargo tree -i serde_json --prefix depth --workspace 2>/dev/null |
    sed -n 's/^1\([a-z0-9_-]*\) v.*/\1/p' | sort -u)

if [ -z "$direct_dependents" ]; then
    echo "error: 未能从 cargo tree 解析出 serde_json 的直接依赖方" >&2
    exit 2
fi

violations=""
for crate in $direct_dependents; do
    case " $ALLOWED " in
    *" $crate "*) ;;
    *) violations="$violations $crate" ;;
    esac
done

if [ -n "$violations" ]; then
    echo "error: 以下 crate 直接依赖 serde_json，违反白名单（仅允许: $ALLOWED）：" >&2
    for crate in $violations; do
        echo "  - $crate" >&2
    done
    echo "请改用 a2ui-core 的类型化访问器与 re-export（Value / json! / Component::from_value）。" >&2
    exit 1
fi

echo "ok: serde_json 直接依赖方 = [$(echo "$direct_dependents" | tr '\n' ' ' | sed 's/ $//')]，符合白名单。"
