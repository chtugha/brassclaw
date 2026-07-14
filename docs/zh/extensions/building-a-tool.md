# 构建一个 BrassClaw 工具

> **Phase 4 更新:** 本文原为“构建 WASM 工具（wasm32-wasip2 组件）”教程。BrassClaw v1 的 WebAssembly 沙箱执行通道（`brassclaw_wasm`、`brassclaw_wasm_sandbox_core`、`brassclaw_wasm_limiter`、`brassclaw_wasm_product_adapters`）以及单独的 `brassclaw_scripts` 脚本运行时已全部移除。新工具应当落地在三种原生 Reborn 通道之一：
>
> - **Hosted MCP** —— 现有的 MCP 服务器，直接由宿主托管
> - **FirstParty** —— 主进程内 Rust 实现，由 `FirstPartyCapabilityRegistry` 注册
> - **ProductAdapter / System** —— 暴露外部协议（通过 `crates/brassclaw_product_adapters`），或基于其他工具组合方式
>
> 需要子进程隔离（shell、docker、git 等）时，统一通过 `brassclaw_process_sandbox::image::validate_reference` 进行镜像校验与能力租约管理。

英文原文已重写：`../extensions/building-a-tool.md`。中文完整教程将随用户文档（`docs/quickstart.mdx`、`docs/extensions/mcp.mdx`、`docs/extensions/*`）在 Phase 6（v1 文档清理阶段）一起重新输出。
