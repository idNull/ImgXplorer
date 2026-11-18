// Atiende las llamadas de la IA que llegan desde Rust (evento `mcp-call`), una detrás de otra, y
// lleva la cuenta de si hay una IA conectada y qué ha hecho (para el aviso de la cabecera).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { McpTools, ToolError, type McpContext } from "./tools.svelte";
import { log } from "../log";

interface CallEvent {
  id: number;
  tool: string;
  args: Record<string, unknown>;
}

interface ActivityEvent {
  kind: "connected" | "disconnected" | "call";
  tool?: string;
  ok?: boolean;
  connections: number;
}

export interface Action {
  tool: string;
  ok: boolean;
  at: number;
}

const KEEP = 30;

class McpState {
  connections = $state(0);
  /** Últimas acciones, la más reciente primero. */
  actions = $state<Action[]>([]);
  /** Herramienta en curso. */
  busy = $state<string | null>(null);
}

export const mcp = new McpState();

/** Empieza a atender a la IA. Devuelve la función para dejar de hacerlo. */
export function startMcp(ctx: McpContext): () => void {
  const tools = new McpTools(ctx);
  let queue: Promise<void> = Promise.resolve();

  async function run(e: CallEvent) {
    mcp.busy = e.tool;
    try {
      const r = await tools.call(e.tool, e.args ?? {});
      await invoke("mcp_reply", { id: e.id, reply: { ok: true, text: r.text, images: r.images ?? [] } });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (!(err instanceof ToolError)) log.warn(`IA: ${e.tool} falló:`, err);
      await invoke("mcp_reply", { id: e.id, reply: { ok: false, error: msg } }).catch(() => {});
    } finally {
      mcp.busy = null;
    }
  }

  const unCall = listen<CallEvent>("mcp-call", (ev) => {
    queue = queue.then(() => run(ev.payload));
  });
  const unAct = listen<ActivityEvent>("mcp-activity", (ev) => {
    const a = ev.payload;
    mcp.connections = a.connections;
    if (a.kind === "call" && a.tool) mcp.actions = [{ tool: a.tool, ok: !!a.ok, at: Date.now() }, ...mcp.actions].slice(0, KEEP);
  });
  invoke<{ connections: number }>("mcp_host_info").then((i) => (mcp.connections = i.connections)).catch(() => {});
  return () => {
    unCall.then((f) => f());
    unAct.then((f) => f());
  };
}
