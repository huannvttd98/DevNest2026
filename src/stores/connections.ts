import { defineStore } from "pinia";
import { computed, ref, watch } from "vue";
import { connectionsApi } from "@/modules/connections/api";
import type { Connection, Group, SaveConnectionInput } from "@/modules/connections/types";

export interface GroupNode {
  group: Group | null; // null = "Ungrouped"
  connections: Connection[];
}

function loadCollapsed(): string[] {
  try {
    return JSON.parse(localStorage.getItem("devnest.collapsedGroups") ?? "[]");
  } catch {
    return [];
  }
}

export const useConnectionsStore = defineStore("connections", () => {
  const groups = ref<Group[]>([]);
  const connections = ref<Connection[]>([]);
  const loading = ref(false);
  const loaded = ref(false);
  const error = ref<string | null>(null);
  const filter = ref("");
  const collapsed = ref<Set<string>>(new Set(loadCollapsed()));

  watch(
    collapsed,
    (s) => {
      try {
        localStorage.setItem("devnest.collapsedGroups", JSON.stringify([...s]));
      } catch {
        /* ignore */
      }
    },
    { deep: true },
  );

  const byId = computed(() => new Map(connections.value.map((c) => [c.id, c])));

  const tree = computed<GroupNode[]>(() => {
    const q = filter.value.trim().toLowerCase();
    const match = (c: Connection) =>
      !q || [c.name, c.host, c.username].some((v) => v.toLowerCase().includes(q));
    const nodes: GroupNode[] = groups.value.map((g) => ({
      group: g,
      connections: connections.value.filter((c) => c.group_id === g.id && match(c)),
    }));
    const ungrouped = connections.value.filter((c) => !c.group_id && match(c));
    if (ungrouped.length || !groups.value.length) nodes.push({ group: null, connections: ungrouped });
    // While filtering, hide groups without matches.
    return q ? nodes.filter((n) => n.connections.length) : nodes;
  });

  async function load() {
    loading.value = true;
    error.value = null;
    try {
      const [g, c] = await Promise.all([connectionsApi.listGroups(), connectionsApi.list()]);
      groups.value = g;
      connections.value = c;
      loaded.value = true;
    } catch (e) {
      error.value = (e as { message?: string }).message ?? String(e);
    } finally {
      loading.value = false;
    }
  }

  function upsert(c: Connection) {
    const i = connections.value.findIndex((x) => x.id === c.id);
    if (i >= 0) connections.value[i] = c;
    else connections.value.push(c);
  }

  async function create(input: SaveConnectionInput) {
    const c = await connectionsApi.create(input);
    upsert(c);
    if (c.group_id) collapsed.value.delete(c.group_id);
    return c;
  }

  async function update(id: string, input: SaveConnectionInput) {
    const c = await connectionsApi.update(id, input);
    upsert(c);
    return c;
  }

  async function move(id: string, groupId: string | null) {
    upsert(await connectionsApi.move(id, groupId));
  }

  async function remove(id: string) {
    await connectionsApi.remove(id);
    connections.value = connections.value.filter((c) => c.id !== id);
  }

  /** Re-fetch one connection (e.g. after a prompt stored a secret). */
  async function refresh() {
    connections.value = await connectionsApi.list();
  }

  async function createGroup(name: string) {
    const g = await connectionsApi.createGroup(name);
    groups.value.push(g);
    return g;
  }

  async function renameGroup(id: string, name: string) {
    const g = await connectionsApi.renameGroup(id, name);
    const i = groups.value.findIndex((x) => x.id === id);
    if (i >= 0) groups.value[i] = g;
  }

  async function deleteGroup(id: string) {
    await connectionsApi.deleteGroup(id);
    groups.value = groups.value.filter((g) => g.id !== id);
    connections.value = connections.value.map((c) => (c.group_id === id ? { ...c, group_id: null } : c));
  }

  function toggleGroup(id: string) {
    if (collapsed.value.has(id)) collapsed.value.delete(id);
    else collapsed.value.add(id);
  }

  return {
    groups,
    connections,
    loading,
    loaded,
    error,
    filter,
    collapsed,
    byId,
    tree,
    load,
    create,
    update,
    move,
    remove,
    refresh,
    createGroup,
    renameGroup,
    deleteGroup,
    toggleGroup,
  };
});
