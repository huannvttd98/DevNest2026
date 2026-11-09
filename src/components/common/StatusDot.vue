<script setup lang="ts">
import { computed } from "vue";
import type { ConnectionStatus } from "@/modules/ssh/api";

const props = defineProps<{ status: ConnectionStatus }>();
const cls = computed(
  () =>
    ({
      connected: "bg-ok",
      connecting: "bg-warn animate-pulse",
      error: "bg-danger",
      disconnected: "bg-fg-subtle/50",
    })[props.status],
);
const label = computed(
  () => ({ connected: "Connected", connecting: "Connecting", error: "Error", disconnected: "Disconnected" })[props.status],
);
</script>

<template>
  <span class="inline-block size-2 shrink-0 rounded-full" :class="cls" :title="label" role="status" :aria-label="label" />
</template>
