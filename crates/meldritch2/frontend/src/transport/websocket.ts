import type { AccessLink, Command, Snapshot } from "../types/protocol";
export class PerformanceSocket {
  private socket?: WebSocket;
  private retry?: number;
  private stopped = false;
  constructor(
    private onSnapshot: (s: Snapshot) => void,
    private onStatus: (s: string) => void,
    private onError: (e: string) => void,
    private onSession: (role: string | null) => void,
    private onAccessLinks: (links: AccessLink[]) => void,
  ) {}
  async connect(token: string) {
    this.stopped = false;
    const config = await fetch("/api/browser-config").then((r) => r.json());
    this.onAccessLinks(config.access_links || []);
    if (!config.websocket) {
      this.onError("WebSocket disabled");
      return;
    }
    const protocol = location.protocol === "https:" ? "wss" : "ws";
    this.socket = new WebSocket(
      `${protocol}://${location.host}${config.websocket}`,
    );
    this.socket.onopen = () => {
      this.onStatus("connected");
      if (token)
        this.socket?.send(JSON.stringify({ type: "authenticate", token }));
    };
    this.socket.onmessage = (e) => {
      const value = JSON.parse(e.data);
      if (value.type === "state.snapshot" || value.type === "state.updated")
        this.onSnapshot(value);
      else if (value.type === "session") this.onSession(value.role);
      else if (value.type === "error") this.onError(value.error);
    };
    this.socket.onclose = () => {
      if (!this.stopped) {
        this.onStatus("reconnecting");
        this.retry = window.setTimeout(() => this.connect(token), 750);
      }
    };
    this.socket.onerror = () => this.onError("connection failed");
  }
  send(command: Command) {
    if (this.socket?.readyState === WebSocket.OPEN)
      this.socket.send(JSON.stringify(command));
    else this.onError("not connected");
  }
  close() {
    this.stopped = true;
    if (this.retry) clearTimeout(this.retry);
    this.socket?.close();
  }
}
