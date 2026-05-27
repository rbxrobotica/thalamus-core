<script lang="ts">
  let { decision }: { decision: string } = $props();

  let cls = $derived(
    decision === 'Allow'
      ? 'badge badge-allow'
      : decision === 'Deny'
        ? 'badge badge-deny'
        : decision === 'AllowWithReview'
          ? 'badge badge-review'
          : 'badge',
  );
</script>

<span class={cls}>{decision}</span>
