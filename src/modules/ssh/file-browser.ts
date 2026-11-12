import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { Connection } from "@/modules/connections/types";

export async function openSftpFileBrowser(connection: Connection) {
  const label = `sftp-${connection.id}`;
  const existing = await WebviewWindow.getByLabel(label);
  if (existing) {
    await existing.setFocus();
    return;
  }

  const window = new WebviewWindow(label, {
    url: `/?window=sftp&connectionId=${encodeURIComponent(connection.id)}`,
    title: `Server files — ${connection.name}`,
    width: 1000,
    height: 700,
    minWidth: 760,
    minHeight: 500,
    center: true,
  });

  await new Promise<void>((resolve, reject) => {
    void window.once("tauri://created", () => resolve());
    void window.once("tauri://error", (event) => reject(new Error(String(event.payload))));
  });
}
