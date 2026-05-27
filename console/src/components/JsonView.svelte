<script lang="ts">
  let { data }: { data: unknown } = $props();
  let formatted = $derived(JSON.stringify(data, null, 2));
</script>

<pre class="result">{formatted}</pre>
