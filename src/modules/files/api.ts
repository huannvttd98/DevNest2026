import { call } from "@/lib/ipc";

export interface LocalFileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number | null;
}

export interface LocalDirectory {
  path: string;
  parent: string | null;
  entries: LocalFileEntry[];
}

export const filesApi = {
  listLocalDir: (path: string) => call<LocalDirectory>("local_list_dir", { path }),
};
