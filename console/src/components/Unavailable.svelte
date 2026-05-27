<script lang="ts">
  let { title, description }: { title: string; description: string } = $props();
</script>

<div class="unavailable">
  <h3>{title}</h3>
  <p>{description}</p>
  <span class="tag">Pending implementation</span>
</div>
